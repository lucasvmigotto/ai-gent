#!/usr/bin/env bash
# Symlink every skill and plugin in this repo into Claude Code's skills dir,
# so the repo stays the single place to edit.
#
#   skills/<name>/   -> ~/.claude/skills/<name>   (loads as a personal skill)
#   plugins/<name>/  -> ~/.claude/skills/<name>   (loads as <name>@skills-dir,
#                                                  skills invoked as /<name>:<skill>)
#
# Idempotent: re-run after adding, renaming or removing anything here.

set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TARGET_DIR="${CLAUDE_SKILLS_DIR:-$HOME/.claude/skills}"

DRY_RUN=0
FORCE=0
UNINSTALL=0

usage() {
  cat <<EOF
Usage: $(basename "$0") [--dry-run] [--force] [--uninstall]

Links skills/* and plugins/* from $REPO_DIR into $TARGET_DIR
(override the target with CLAUDE_SKILLS_DIR).

  --dry-run    print what would change, change nothing
  --force      back up and replace a real directory or foreign symlink
               that is in the way (default: skip it with a warning)
  --uninstall  remove every symlink in the target that points into this repo
EOF
}

for arg in "$@"; do
  case "$arg" in
    --dry-run) DRY_RUN=1 ;;
    --force) FORCE=1 ;;
    --uninstall) UNINSTALL=1 ;;
    -h | --help) usage; exit 0 ;;
    *) echo "unknown option: $arg" >&2; usage >&2; exit 2 ;;
  esac
done

run() {
  if ((DRY_RUN)); then
    echo "  [dry-run] $*"
  else
    "$@"
  fi
}

points_into_repo() {
  local link_target
  link_target="$(readlink "$1")"
  [[ "$link_target" == "$REPO_DIR"/* ]]
}

link() {
  local src="$1" dest="$TARGET_DIR/$(basename "$1")"

  if [[ -L "$dest" ]]; then
    if [[ "$(readlink "$dest")" == "$src" ]]; then
      echo "ok       $(basename "$dest")"
      return
    fi
    if ! points_into_repo "$dest" && ((!FORCE)); then
      echo "skip     $(basename "$dest") (symlink to $(readlink "$dest"); use --force)" >&2
      return
    fi
    echo "relink   $(basename "$dest")"
    run ln -sfn "$src" "$dest"
  elif [[ -e "$dest" ]]; then
    if ((!FORCE)); then
      echo "skip     $(basename "$dest") (real directory in the way; use --force to back it up)" >&2
      return
    fi
    local backup="$dest.bak-$(date +%Y%m%d%H%M%S)"
    echo "backup   $(basename "$dest") -> $(basename "$backup")"
    run mv "$dest" "$backup"
    echo "link     $(basename "$dest")"
    run ln -s "$src" "$dest"
  else
    echo "link     $(basename "$dest")"
    run ln -s "$src" "$dest"
  fi
}

# Remove symlinks into this repo whose source no longer exists (renamed or
# deleted skills), or all of them on --uninstall.
prune() {
  local dest
  for dest in "$TARGET_DIR"/*; do
    [[ -L "$dest" ]] && points_into_repo "$dest" || continue
    if ((UNINSTALL)) || [[ ! -e "$dest" ]]; then
      echo "unlink   $(basename "$dest")"
      run rm "$dest"
    fi
  done
}

run mkdir -p "$TARGET_DIR"
prune
((UNINSTALL)) && exit 0

for dir in "$REPO_DIR"/skills/*/; do
  dir="${dir%/}"
  [[ -f "$dir/SKILL.md" ]] && link "$dir"
done

for dir in "$REPO_DIR"/plugins/*/; do
  dir="${dir%/}"
  [[ -f "$dir/.claude-plugin/plugin.json" ]] && link "$dir"
done

echo "done — restart Claude Code (or start a new session) to pick up changes to the set of skills."
