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
# The package for this platform carries bin/ai-gent; setup.sh links it into
# ~/.local/bin. A 2.x release without that archive (2.26 and older) installs its
# content-only package, ai-gent-<ver>.tar.gz, with a notice; from 3.0.0 a missing
# archive is an error.
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
#                         lucasvmigotto/ai-gent). <base>/<ver>/ai-gent-<ver>-<triple>.tar.gz
#                         (.zip on Windows) and SHA256SUMS; "latest" reads <base>/LATEST.
#                         file:// works.
#   AI_GENT_TARGET        the platform triple to install, instead of the one detected from
#                         uname (tests, odd shells): x86_64-unknown-linux-musl,
#                         aarch64-unknown-linux-musl, aarch64-apple-darwin,
#                         x86_64-apple-darwin, x86_64-pc-windows-msvc (Git Bash),
#                         aarch64-pc-windows-msvc (Git Bash)
#
# Git mode was removed in 3.0.0: when AI_GENT_MODE, AI_GENT_BRANCH, AI_GENT_REPO or
# AI_GENT_DIR is set the installer stops and says what to do (contributors work in a
# checkout and run its setup.sh; scripts/dev-bin.sh provides the binary).
#
# Everything runs inside main(), called on the last line, so a partially
# downloaded script never executes.

set -eu

SLUG="lucasvmigotto/ai-gent"

say() { printf 'ai-gent: %s\n' "$*"; }
die() { printf 'ai-gent: error: %s\n' "$*" >&2; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || die "'$1' is required but not installed"; }

is_version() { case "$1" in '' | *[!0-9.]* | .* | *. | *..*) return 1 ;; esac; [ "$(printf '%s' "$1" | tr -cd . | wc -c)" -eq 2 ]; }

SUPPORTED_TARGETS="x86_64-unknown-linux-musl aarch64-unknown-linux-musl aarch64-apple-darwin x86_64-apple-darwin x86_64-pc-windows-msvc aarch64-pc-windows-msvc"

# detect_target — print this platform's triple, or stop before downloading anything
detect_target() {
  if [ -n "${AI_GENT_TARGET:-}" ]; then
    case " $SUPPORTED_TARGETS " in
      *" $AI_GENT_TARGET "*) printf '%s' "$AI_GENT_TARGET"; return ;;
    esac
    die "unsupported AI_GENT_TARGET '$AI_GENT_TARGET'; supported: $SUPPORTED_TARGETS"
  fi
  os="$(uname -s)"; arch="$(uname -m)"
  case "$os/$arch" in
    Linux/x86_64) printf 'x86_64-unknown-linux-musl' ;;
    Linux/aarch64 | Linux/arm64) printf 'aarch64-unknown-linux-musl' ;;
    Darwin/arm64) printf 'aarch64-apple-darwin' ;;
    Darwin/x86_64)
      # a shell run under Rosetta reports x86_64 on an Apple silicon Mac
      if [ "$(sysctl -n hw.optional.arm64 2>/dev/null || true)" = 1 ]; then printf 'aarch64-apple-darwin'; else printf 'x86_64-apple-darwin'; fi ;;
    MINGW*/x86_64 | MSYS*/x86_64 | CYGWIN*/x86_64) printf 'x86_64-pc-windows-msvc' ;;
    MINGW*/aarch64 | MSYS*/aarch64 | CYGWIN*/aarch64 | MINGW*/arm64 | MSYS*/arm64 | CYGWIN*/arm64) printf 'aarch64-pc-windows-msvc' ;;
    *) die "unsupported platform $os/$arch; supported: $SUPPORTED_TARGETS" ;;
  esac
}

# sums_entry <SHA256SUMS> <file name> — the checksum listed for it, if any
sums_entry() { awk -v f="$2" '$2 == f || $2 == "*" f { print $1; exit }' "$1"; }

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
  target="$(detect_target)" # before any download: an unsupported platform stops here
  if [ "$ver" = latest ]; then ver="$(resolve_latest)"; fi
  is_version "$ver" || die "not a release version: '$ver' (want x.y.z or latest)"
  say "installing ai-gent $ver ($target)"
  say "  into  $home/$ver  (current -> $ver)"

  if [ -d "$home/$ver" ] && [ ! -L "$home/$ver" ]; then
    say "$ver is already unpacked"
  else
    fetch "$base/$ver/SHA256SUMS" "$work/SHA256SUMS"
    case "$target" in *-windows-msvc) ext=zip; exe=ai-gent.exe ;; *) ext=tar.gz; exe=ai-gent ;; esac
    name="ai-gent-$ver-$target.$ext"
    with_bin=1
    if [ -z "$(sums_entry "$work/SHA256SUMS" "$name")" ]; then
      # 2.x keeps publishing the content-only package; from 3.0.0 a missing archive is an error
      if [ "${ver%%.*}" -lt 3 ] && [ -n "$(sums_entry "$work/SHA256SUMS" "ai-gent-$ver.tar.gz")" ]; then
        say "no archive for $target in release $ver; installing its content-only package (skills and plugins, no ai-gent binary)"
        name="ai-gent-$ver.tar.gz"; ext=tar.gz; with_bin=0
      else
        die "no archive for $target in release $ver (SHA256SUMS has no entry for $name); nothing was installed"
      fi
    fi
    say "  from  $base/$ver/$name"
    fetch "$base/$ver/$name" "$work/$name"
    want="$(sums_entry "$work/SHA256SUMS" "$name")"
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
    if [ "$ext" = zip ]; then
      need unzip
      unzip -Z1 "$work/$name" >"$work/list" 2>/dev/null || die "$name is not a readable zip; nothing was installed"
    else
      tar -tzf "$work/$name" >"$work/list" 2>/dev/null || die "$name is not a readable gzip tarball; nothing was installed"
    fi
    if grep -Eq '(^/|(^|/)\.\.(/|$))' "$work/list"; then die "$name has unsafe paths; nothing was installed"; fi

    mkdir -p "$home"
    stage="$(mktemp -d "$home/.stage.XXXXXX")"
    if [ "$ext" = zip ]; then
      mkdir "$work/unzip"
      unzip -q "$work/$name" -d "$work/unzip" || die "extraction failed; nothing was installed"
      mv "$work"/unzip/*/* "$work"/unzip/*/.[!.]* "$stage"/ 2>/dev/null || true # the archive's top directory
    else
      tar -xzf "$work/$name" -C "$stage" --strip-components=1 || die "extraction failed; nothing was installed"
    fi
    [ "$(cat "$stage/VERSION" 2>/dev/null)" = "$ver" ] || die "the package says it is not $ver; nothing was installed"
    [ -f "$stage/setup.sh" ] || die "the package has no setup.sh; nothing was installed"
    if [ "$with_bin" = 1 ]; then
      [ -f "$stage/bin/$exe" ] || die "the package has no bin/$exe; nothing was installed"
      chmod +x "$stage/bin/$exe" # a zip may not keep the mode
      [ -x "$stage/bin/$exe" ] || die "bin/$exe is not executable; nothing was installed"
    fi
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

MIGRATION_GUIDE="https://github.com/$SLUG/blob/main/README.md#install--update" # "Moving from the git clone"

# removed_git_mode <variable> — the dev mode is gone; stop with the steps, never fall through to a package install
removed_git_mode() {
  {
    printf 'ai-gent: error: git mode was removed in 3.0.0 (%s is set).\n' "$1"
    printf '  To install the release package: unset AI_GENT_MODE AI_GENT_BRANCH AI_GENT_REPO AI_GENT_DIR and run this installer again.\n'
    printf '  If ~/.local/share/ai-gent is your old clone: mv ~/.local/share/ai-gent ~/.local/share/ai-gent.clone, install, then run ~/.local/share/ai-gent/current/setup.sh --migrate.\n'
    printf '  To work on ai-gent itself: clone the repository, run ./setup.sh from it and scripts/dev-bin.sh for the binary.\n'
    printf '  Guide: %s\n' "$MIGRATION_GUIDE"
  } >&2
  exit 1
}

main() {
  verify_att=0; action=install
  for v in AI_GENT_MODE AI_GENT_BRANCH AI_GENT_REPO AI_GENT_DIR; do
    eval "val=\${$v:-}"
    [ -z "$val" ] || removed_git_mode "$v"
  done
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
    die "$home is a git clone (the old install). Either move it aside and re-run (mv \"$home\" \"$home.clone\"; then ./current/setup.sh --migrate relinks the agents), or set AI_GENT_HOME to another directory. Guide: $MIGRATION_GUIDE"
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
