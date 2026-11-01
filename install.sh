#!/bin/sh
# ai-gent installer. Default: download a release package, verify its checksum,
# unpack it under a versioned directory and run its setup.sh.
#
#   curl -fsSL https://github.com/lucasvmigotto/ai-gent/releases/latest/download/install.sh | sh
#   ... | sh -s -- --target claude,codex          (arguments go to setup.sh)
#   VERSION=2.26.0 sh install.sh                  (pin a release)
#   sh install.sh --list | --rollback | --prune   (manage installed versions)
#   sh install.sh --verify-attestation            (also run `gh attestation verify`)
#
# Layout: $AI_GENT_HOME/<version>/ for each release, `current` -> the active
# one, `previous` -> the one before (for --rollback). No sudo, nothing outside
# $AI_GENT_HOME is written except what setup.sh links into the agents' dirs.
# Nothing is written under $AI_GENT_HOME until the download passed its checksum.
#
# Environment:
#   VERSION               latest (default) or x.y.z (x.y.z pins: no update notices)
#   AI_GENT_HOME          install root (default: ${XDG_DATA_HOME:-$HOME/.local/share}/ai-gent)
#   AI_GENT_RELEASE_BASE  where releases live (default: the GitHub releases URL of
#                         lucasvmigotto/ai-gent). <base>/<ver>/ai-gent-<ver>.tar.gz and
#                         SHA256SUMS; "latest" reads <base>/LATEST. file:// works.
#   AI_GENT_MODE=git      DEV MODE, going away in v3: git clone instead of a package
#                         (also AI_GENT_DIR, AI_GENT_REPO, AI_GENT_BRANCH; setting
#                         AI_GENT_BRANCH implies it).
#
# Everything runs inside main(), called on the last line, so a partially
# downloaded script never executes.

set -eu

SLUG="lucasvmigotto/ai-gent"

say() { printf 'ai-gent: %s\n' "$*"; }
die() { printf 'ai-gent: error: %s\n' "$*" >&2; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || die "'$1' is required but not installed"; }

is_version() { case "$1" in '' | *[!0-9.]* | .* | *. | *..*) return 1 ;; esac; [ "$(printf '%s' "$1" | tr -cd . | wc -c)" -eq 2 ]; }

# fetch <url> <file> — curl handles https and file://
fetch() { curl -fsSL --proto '=https,file' --max-time 300 -o "$2" "$1" || die "could not download $1"; }

# sha256 <file> — the hash, with whichever tool exists
sha256() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | cut -d' ' -f1
  elif command -v shasum >/dev/null 2>&1; then shasum -a 256 "$1" | cut -d' ' -f1
  else die "need sha256sum or shasum to verify the download"; fi
}

# flip <target> <link> — point <link> at <target> atomically (rename over it)
flip() {
  tmp_link="$2.new.$$"
  rm -f "$tmp_link"
  ln -s "$1" "$tmp_link"
  if mv -fT "$tmp_link" "$2" 2>/dev/null || mv -fh "$tmp_link" "$2" 2>/dev/null; then return 0; fi
  ln -sfn "$1" "$2" && rm -f "$tmp_link"   # no atomic rename available: still correct, just not atomic
}

link_target() { [ -L "$1" ] && readlink "$1" || true; }

installed_versions() { # one per line, oldest first
  for d in "$home"/*; do
    [ -d "$d" ] && [ ! -L "$d" ] || continue
    v="${d##*/}"; is_version "$v" && printf '%s\n' "$v"
  done | sort -t. -k1,1n -k2,2n -k3,3n
}

run_setup() { # run_setup <args...> — the package's own setup.sh, through `current`
  need bash
  [ -f "$home/current/setup.sh" ] || die "$home/current/setup.sh not found"
  bash "$home/current/setup.sh" "$@"
}

do_list() {
  cur="$(link_target "$home/current")"; prev="$(link_target "$home/previous")"
  [ -d "$home" ] || { say "nothing installed in $home"; return 0; }
  installed_versions | while read -r v; do
    mark=""; [ "$v" = "$cur" ] && mark=" (current)"; [ "$v" = "$prev" ] && mark=" (previous)"
    printf '%s%s\n' "$v" "$mark"
  done
}

do_prune() {
  cur="$(link_target "$home/current")"; prev="$(link_target "$home/previous")"
  [ -n "$cur" ] || die "no current version in $home; nothing is pruned"
  installed_versions | while read -r v; do
    [ "$v" = "$cur" ] || [ "$v" = "$prev" ] && continue
    say "removing $home/$v"
    rm -rf "${home:?}/$v"
  done
  rm -rf "$home"/.stage.* 2>/dev/null || true
}

do_rollback() {
  prev="$(link_target "$home/previous")"; cur="$(link_target "$home/current")"
  [ -n "$prev" ] && [ -d "$home/$prev" ] || die "no previous version to roll back to in $home"
  is_version "$prev" || die "unexpected previous link: $prev"
  say "rolling back: current $cur -> $prev (previous becomes $cur)"
  flip "$prev" "$home/current"
  [ -n "$cur" ] && flip "$cur" "$home/previous"
  run_setup --skip-prompt "$@"
  say "now on $prev"
}

# resolve_latest — print the newest release version
resolve_latest() {
  if [ -n "${AI_GENT_RELEASE_BASE:-}" ]; then
    fetch "$base/LATEST" "$work/LATEST"
    tr -d ' \r\n' <"$work/LATEST"
    return
  fi
  url="$(curl -fsSL --proto '=https' --max-time 30 -o /dev/null -w '%{url_effective}' "https://github.com/$SLUG/releases/latest")" ||
    die "could not resolve the latest release; set VERSION=x.y.z"
  printf '%s' "${url##*/}"
}

do_install() {
  need curl; need tar; need gzip
  work="$(mktemp -d "${TMPDIR:-/tmp}/ai-gent-install.XXXXXX")"
  trap 'rm -rf "$work"; [ -z "${stage:-}" ] || rm -rf "$stage"; rm -f "${tmp_link:-}"' EXIT INT TERM

  ver="${VERSION:-latest}"
  if [ "$ver" = latest ]; then ver="$(resolve_latest)"; fi
  is_version "$ver" || die "not a release version: '$ver' (want x.y.z or latest)"
  name="ai-gent-$ver.tar.gz"
  say "installing ai-gent $ver"
  say "  from  $base/$ver/$name"
  say "  into  $home/$ver  (current -> $ver)"

  if [ -d "$home/$ver" ] && [ ! -L "$home/$ver" ]; then
    say "$ver is already unpacked"
  else
    fetch "$base/$ver/$name" "$work/$name"
    fetch "$base/$ver/SHA256SUMS" "$work/SHA256SUMS"
    want="$(awk -v f="$name" '$2 == f || $2 == "*" f { print $1; exit }' "$work/SHA256SUMS")"
    [ -n "$want" ] || die "SHA256SUMS has no entry for $name"
    got="$(sha256 "$work/$name")"
    [ "$want" = "$got" ] || die "checksum mismatch for $name (expected $want, got $got); nothing was installed"
    say "checksum ok ($got)"
    if [ "$verify_att" = 1 ]; then
      if command -v gh >/dev/null 2>&1; then
        gh attestation verify "$work/$name" -R "$SLUG" >/dev/null || die "attestation verification failed; nothing was installed"
        say "attestation ok (gh attestation verify, $SLUG)"
      else
        say "gh is not installed: attestation NOT verified (checksum only)"
      fi
    fi

    # Look inside before writing anything under $home.
    tar -tzf "$work/$name" >"$work/list" 2>/dev/null || die "$name is not a readable gzip tarball; nothing was installed"
    if grep -Eq '(^/|(^|/)\.\.(/|$))' "$work/list"; then die "$name has unsafe paths; nothing was installed"; fi

    mkdir -p "$home"
    stage="$(mktemp -d "$home/.stage.XXXXXX")"
    tar -xzf "$work/$name" -C "$stage" --strip-components=1 || die "extraction failed; nothing was installed"
    [ "$(cat "$stage/VERSION" 2>/dev/null)" = "$ver" ] || die "the package says it is not $ver; nothing was installed"
    [ -f "$stage/setup.sh" ] || die "the package has no setup.sh; nothing was installed"
    if find "$stage" -type l | while read -r l; do t="$(readlink "$l")"; case "$t" in /*) echo bad ;; esac; done | grep -q bad; then
      die "the package has an absolute symlink; nothing was installed"
    fi
    mv "$stage" "$home/$ver"   # same filesystem: atomic
    stage=""
  fi

  cur="$(link_target "$home/current")"
  if [ "$cur" != "$ver" ]; then
    [ -z "$cur" ] || flip "$cur" "$home/previous"
    flip "$ver" "$home/current"
  fi
  # An explicit VERSION=x.y.z pins (update-check.sh stays silent); latest or no VERSION clears it.
  if [ -n "${VERSION:-}" ] && [ "$VERSION" != latest ]; then printf '%s\n' "$ver" >"$home/pinned"; else rm -f "$home/pinned"; fi
  run_setup "$@"
  say "installed $ver in $home; to update run this installer again, to undo: sh install.sh --rollback"
}

# update_to <dir> <branch or tag> — move an existing clone to that ref.
update_to() {
  if ! git -C "$1" fetch --quiet --tags origin; then
    say "couldn't fetch — not updating $1, linking what's there"
  elif git -C "$1" rev-parse --quiet --verify "refs/tags/$2" >/dev/null; then
    git -C "$1" checkout --quiet --detach "refs/tags/$2"
    say "pinned $1 to $2"
  elif ! { git -C "$1" checkout --quiet "$2" && git -C "$1" pull --ff-only --quiet origin "$2"; }; then
    say "couldn't move $1 to $2 — linking what's there"
  fi
}

git_mode() {
  need git
  need bash

  dir="${AI_GENT_DIR:-${XDG_DATA_HOME:-$HOME/.local/share}/ai-gent}"
  repo="${AI_GENT_REPO:-https://github.com/lucasvmigotto/ai-gent.git}"
  branch="${AI_GENT_BRANCH:-}"

  if [ -d "$dir/.git" ]; then
    say "updating $dir"
    if [ -n "$(git -C "$dir" status --porcelain)" ]; then
      say "local changes in $dir — not updating it, linking what's there"
    elif [ -n "$branch" ]; then
      update_to "$dir" "$branch"
    elif ! git -C "$dir" symbolic-ref --quiet HEAD >/dev/null; then
      say "$dir is pinned to $(git -C "$dir" describe --tags --always) — set AI_GENT_BRANCH to move it"
    else
      before="$(git -C "$dir" describe --tags --always)"
      if ! git -C "$dir" pull --ff-only --quiet; then
        say "couldn't fast-forward $dir — not updating it, linking what's there"
      else
        after="$(git -C "$dir" describe --tags --always)"
        if [ "$before" = "$after" ]; then
          say "already at $after"
        else
          say "updated $before → $after"
        fi
      fi
    fi
  elif [ -e "$dir" ]; then
    die "$dir exists and isn't a git clone; set AI_GENT_DIR to another path"
  else
    say "cloning $repo into $dir"
    mkdir -p "$(dirname "$dir")"
    if [ -n "$branch" ]; then
      git clone --quiet --branch "$branch" "$repo" "$dir"
    else
      git clone --quiet "$repo" "$dir"
    fi
  fi

  [ -f "$dir/setup.sh" ] || die "$dir/setup.sh not found — is AI_GENT_REPO right?"
  bash "$dir/setup.sh" "$@"

  say "installed from $dir — re-run this installer (or $dir/setup.sh) to update"
}

main() {
  mode=package; verify_att=0; action=install
  [ "${AI_GENT_MODE:-}" = git ] || [ -n "${AI_GENT_BRANCH:-}" ] && mode=git
  if [ "$mode" = git ]; then
    say "git mode is a DEV mode and goes away in v3; the package install is the default"
    git_mode "$@"
    return
  fi
  # our own flags come first; the rest goes to setup.sh
  for a in "$@"; do
    shift
    case "$a" in
      --list) action=list ;; --prune) action=prune ;; --rollback) action=rollback ;;
      --verify-attestation) verify_att=1 ;;
      *) set -- "$@" "$a" ;;
    esac
  done

  home="${AI_GENT_HOME:-${XDG_DATA_HOME:-$HOME/.local/share}/ai-gent}"
  base="${AI_GENT_RELEASE_BASE:-https://github.com/$SLUG/releases/download}"
  if [ -e "$home/.git" ]; then
    die "$home is a git clone (the old install). Either move it aside and re-run (mv \"$home\" \"$home.clone\"; then ./current/setup.sh --migrate relinks the agents), or set AI_GENT_HOME to another directory, or AI_GENT_MODE=git to keep using it"
  fi
  [ ! -e "$home" ] || [ -d "$home" ] || die "$home exists and is not a directory"

  case "$action" in
    list) do_list ;;
    prune) do_prune ;;
    rollback) do_rollback "$@" ;;
    install) do_install "$@" ;;
  esac
}

main "$@"
