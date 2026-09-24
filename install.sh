#!/bin/sh
# ai-gent installer: clone (or update) the repo, then run setup.sh.
#
#   curl -fsSL https://raw.githubusercontent.com/lucasvmigotto/ai-gent/HEAD/install.sh | sh
#   curl -fsSL https://raw.githubusercontent.com/lucasvmigotto/ai-gent/HEAD/install.sh | sh -s -- --target opencode
#
# Environment:
#   AI_GENT_DIR     where to clone      (default: ${XDG_DATA_HOME:-$HOME/.local/share}/ai-gent)
#   AI_GENT_REPO    what to clone       (default: https://github.com/lucasvmigotto/ai-gent.git)
#   AI_GENT_BRANCH  branch to check out (default: the repository's default branch)
#
# Arguments are passed to setup.sh (--target, --dry-run, --force, --uninstall).
#
# Everything runs inside main(), called on the last line, so a partially
# downloaded script never executes.

set -eu

say() { printf 'ai-gent: %s\n' "$*"; }
die() { printf 'ai-gent: error: %s\n' "$*" >&2; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || die "'$1' is required but not installed"; }

main() {
  need git
  need bash

  dir="${AI_GENT_DIR:-${XDG_DATA_HOME:-$HOME/.local/share}/ai-gent}"
  repo="${AI_GENT_REPO:-https://github.com/lucasvmigotto/ai-gent.git}"
  branch="${AI_GENT_BRANCH:-}"

  if [ -d "$dir/.git" ]; then
    say "updating $dir"
    if [ -n "$(git -C "$dir" status --porcelain)" ]; then
      say "local changes in $dir — not updating it, linking what's there"
    elif ! git -C "$dir" pull --ff-only --quiet; then
      say "couldn't fast-forward $dir — not updating it, linking what's there"
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

main "$@"
