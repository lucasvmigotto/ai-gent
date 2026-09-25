#!/usr/bin/env bash
# Install this repo's skills and plugins for Claude Code and opencode, so the
# repo stays the single place to edit.
#
# Claude Code (symlinks, read live):
#   skills/<name>/   -> ~/.claude/skills/<name>   (personal skill, /<name>)
#   plugins/<name>/  -> ~/.claude/skills/<name>   (<name>@skills-dir, /<name>:<skill>)
#
# opencode (~/.config/opencode/skills must be a real directory):
#   skills/<name>/                -> skills/<name>          (symlink)
#   ~/.claude/skills/synced       -> skills/synced          (symlink, claude.ai-synced skills)
#   plugins/<p>/skills/<s>/       -> skills/<p>-<s>/SKILL.md (generated entry file pointing at the source)
#                                    commands/<p>-<s>.md     (generated /<p>-<s> command)
#   Set OPENCODE_DISABLE_CLAUDE_CODE_SKILLS=1 so opencode reads only these,
#   instead of also scanning ~/.claude/skills (whose nested plugin skills
#   collide by short name: spec, build, ...).
#
# Idempotent: re-run after adding, renaming or removing a skill, or after
# changing a plugin skill's description. Edits to skill bodies need no re-run.

set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CLAUDE_DIR="${CLAUDE_SKILLS_DIR:-$HOME/.claude/skills}"
OPENCODE_SKILLS="${OPENCODE_SKILLS_DIR:-$HOME/.config/opencode/skills}"
OPENCODE_COMMANDS="${OPENCODE_COMMANDS_DIR:-$HOME/.config/opencode/commands}"
MARKER=".ai-gent-generated"

DRY_RUN=0
FORCE=0
UNINSTALL=0
TARGET="all"

usage() {
  cat <<EOF
Usage: $(basename "$0") [--target claude|opencode|all] [--dry-run] [--force] [--uninstall]

Installs skills/* and plugins/* from $REPO_DIR for:
  claude    symlinks into $CLAUDE_DIR            (override: CLAUDE_SKILLS_DIR)
  opencode  links and generated entry files into $OPENCODE_SKILLS
            and commands into $OPENCODE_COMMANDS (override: OPENCODE_SKILLS_DIR,
            OPENCODE_COMMANDS_DIR); skipped when opencode isn't installed
  all       both (default)

  --dry-run    print what would change, change nothing
  --force      back up and replace a real directory or foreign symlink in the way
  --uninstall  remove everything this script installed for the chosen target(s)
EOF
}

while (($#)); do
  case "$1" in
    --target) TARGET="${2:-}"; shift ;;
    --target=*) TARGET="${1#*=}" ;;
    --dry-run) DRY_RUN=1 ;;
    --force) FORCE=1 ;;
    --uninstall) UNINSTALL=1 ;;
    -h | --help) usage; exit 0 ;;
    *) echo "unknown option: $1" >&2; usage >&2; exit 2 ;;
  esac
  shift
done
case "$TARGET" in claude | opencode | all) ;; *) echo "invalid --target: $TARGET" >&2; exit 2 ;; esac

run() {
  if ((DRY_RUN)); then
    echo "  [dry-run] $*"
  else
    "$@"
  fi
}

# write <file> <content> — honors --dry-run
write() {
  if ((DRY_RUN)); then
    echo "  [dry-run] write $1"
  else
    printf '%s' "$2" >"$1"
  fi
}

points_into_repo() {
  [[ "$(readlink "$1")" == "$REPO_DIR"/* ]]
}

# link <src> <dest>
link() {
  local src="$1" dest="$2" name
  name="$(basename "$dest")"

  if [[ -L "$dest" ]]; then
    if [[ "$(readlink "$dest")" == "$src" ]]; then
      echo "ok       $name"
      return
    fi
    if ! points_into_repo "$dest" && ((!FORCE)); then
      echo "skip     $name (symlink to $(readlink "$dest"); use --force)" >&2
      return
    fi
    echo "relink   $name"
    run ln -sfn "$src" "$dest"
  elif [[ -e "$dest" ]]; then
    if ((!FORCE)); then
      echo "skip     $name (real file or directory in the way; use --force to back it up)" >&2
      return
    fi
    local backup
    backup="$dest.bak-$(date +%Y%m%d%H%M%S)"
    echo "backup   $name -> $(basename "$backup")"
    run mv "$dest" "$backup"
    echo "link     $name"
    run ln -s "$src" "$dest"
  else
    echo "link     $name"
    run ln -s "$src" "$dest"
  fi
}

# Remove symlinks into this repo whose source is gone, or all of them on --uninstall.
prune_links() {
  local dir="$1" dest
  for dest in "$dir"/*; do
    if [[ ! -L "$dest" ]] || ! points_into_repo "$dest"; then continue; fi
    if ((UNINSTALL)) || [[ ! -e "$dest" ]]; then
      echo "unlink   $(basename "$dest")"
      run rm "$dest"
    fi
  done
}

frontmatter() { # frontmatter <file> <key> — single-line values only
  sed -n "/^---$/,/^---$/{s/^$2: //p}" "$1" | head -n1
}

yaml_quote() {
  local s="${1//\\/\\\\}"
  printf '"%s"' "${s//\"/\\\"}"
}

# ---------------------------------------------------------------- Claude Code

install_claude() {
  echo "== Claude Code: $CLAUDE_DIR"
  run mkdir -p "$CLAUDE_DIR"
  prune_links "$CLAUDE_DIR"
  ((UNINSTALL)) && return

  local dir
  for dir in "$REPO_DIR"/skills/*/; do
    dir="${dir%/}"
    [[ -f "$dir/SKILL.md" ]] && link "$dir" "$CLAUDE_DIR/$(basename "$dir")"
  done
  for dir in "$REPO_DIR"/plugins/*/; do
    dir="${dir%/}"
    [[ -f "$dir/.claude-plugin/plugin.json" ]] && link "$dir" "$CLAUDE_DIR/$(basename "$dir")"
  done
}

# ------------------------------------------------------------------- opencode

opencode_entry() { # opencode_entry <plugin> <skill> <source SKILL.md>
  local plugin="$1" skill="$2" src="$3"
  local name="$plugin-$skill"
  local desc
  desc="$(frontmatter "$src" description)"
  if ((${#desc} < 1 || ${#desc} > 1024)); then
    echo "skip     $name (description is ${#desc} chars; opencode needs 1-1024)" >&2
    return
  fi

  local dir="$OPENCODE_SKILLS/$name"
  if [[ -e "$dir" && ! -f "$dir/$MARKER" ]]; then
    echo "skip     $name (exists and wasn't generated by this script)" >&2
    return
  fi

  local content
  content="---
name: $name
description: $(yaml_quote "$desc")
metadata:
  source: $(yaml_quote "$src")
---

<!-- Generated by ai-gent/setup.sh. Edit the source file, not this one. -->

The instructions for this skill live in the ai-gent repository. Read
\`$src\` in full and follow it as if it were this skill.

While following it:

- Its base directory is \`$(dirname "$src")\`; resolve relative paths such
  as \`../../references/pipeline.md\` from there.
- Skills written as \`plugin:skill\` (for example \`devcontainer:setup\`,
  \`frontend:spec\`) are named \`plugin-skill\` here (\`devcontainer-setup\`,
  \`frontend-spec\`); load them with the skill tool.
- Where it mentions Claude Code tools (Read, Edit, Bash, Skill), use
  opencode's equivalents.
"
  local current=""
  [[ -f "$dir/SKILL.md" ]] && current="$(cat "$dir/SKILL.md")"
  if [[ "$current" == "${content%$'\n'}" ]]; then
    echo "ok       $name"
  else
    echo "generate $name"
    run mkdir -p "$dir"
    write "$dir/SKILL.md" "$content"
    write "$dir/$MARKER" ""
  fi

  # /<plugin>-<skill> command
  local summary="${desc%%. *}"
  ((${#summary} > 160)) && summary="${summary:0:157}..."
  local cmd="$OPENCODE_COMMANDS/$name.md"
  if [[ -f "$cmd" ]] && ! grep -q 'Generated by ai-gent/setup.sh' "$cmd"; then
    echo "skip     /$name (command exists and wasn't generated by this script)" >&2
    return
  fi
  local command_text
  command_text="---
description: $(yaml_quote "$summary")
---

<!-- Generated by ai-gent/setup.sh. -->

Load the \`$name\` skill with the skill tool and follow it.

\$ARGUMENTS
"
  if [[ -f "$cmd" && "$(cat "$cmd")" == "${command_text%$'\n'}" ]]; then
    return
  fi
  write "$cmd" "$command_text"
}

prune_opencode_generated() {
  local dir name plugin skill
  for dir in "$OPENCODE_SKILLS"/*/; do
    dir="${dir%/}"
    [[ -f "$dir/$MARKER" ]] || continue
    name="$(basename "$dir")"
    local src
    src="$(sed -n 's/^  source: "\(.*\)"$/\1/p' "$dir/SKILL.md")"
    if ((UNINSTALL)) || [[ ! -f "$src" ]]; then
      echo "remove   $name"
      run rm -rf "$dir"
      [[ -f "$OPENCODE_COMMANDS/$name.md" ]] && run rm "$OPENCODE_COMMANDS/$name.md"
    fi
  done
}

install_opencode() {
  if ! command -v opencode >/dev/null 2>&1 && [[ ! -d "$(dirname "$OPENCODE_SKILLS")" ]]; then
    echo "== opencode: not installed, skipped"
    return
  fi
  echo "== opencode: $OPENCODE_SKILLS"

  # A symlink to the Claude skills dir would make generated entries appear
  # as Claude personal skills too; opencode needs its own real directory.
  if [[ -L "$OPENCODE_SKILLS" ]]; then
    if [[ "$(readlink -f "$OPENCODE_SKILLS")" == "$(readlink -f "$CLAUDE_DIR")" ]] || ((FORCE)); then
      echo "replace  $OPENCODE_SKILLS (symlink to $(readlink "$OPENCODE_SKILLS")) with a real directory"
      run rm "$OPENCODE_SKILLS"
    else
      echo "skip     opencode ($OPENCODE_SKILLS is a symlink to $(readlink "$OPENCODE_SKILLS"); use --force)" >&2
      return
    fi
  fi
  run mkdir -p "$OPENCODE_SKILLS" "$OPENCODE_COMMANDS"

  prune_links "$OPENCODE_SKILLS"
  prune_opencode_generated
  if ((UNINSTALL)); then
    if [[ -L "$OPENCODE_SKILLS/synced" ]]; then
      echo "unlink   synced"
      run rm "$OPENCODE_SKILLS/synced"
    fi
    return
  fi

  local dir skill
  for dir in "$REPO_DIR"/skills/*/; do
    dir="${dir%/}"
    [[ -f "$dir/SKILL.md" ]] && link "$dir" "$OPENCODE_SKILLS/$(basename "$dir")"
  done
  [[ -d "$CLAUDE_DIR/synced" ]] && link "$CLAUDE_DIR/synced" "$OPENCODE_SKILLS/synced"

  for dir in "$REPO_DIR"/plugins/*/; do
    dir="${dir%/}"
    [[ -f "$dir/.claude-plugin/plugin.json" ]] || continue
    for skill in "$dir"/skills/*/; do
      skill="${skill%/}"
      [[ -f "$skill/SKILL.md" ]] && opencode_entry "$(basename "$dir")" "$(basename "$skill")" "$skill/SKILL.md"
    done
  done

  if [[ "${OPENCODE_DISABLE_CLAUDE_CODE_SKILLS:-}" != "1" ]]; then
    cat >&2 <<'EOF'
note     OPENCODE_DISABLE_CLAUDE_CODE_SKILLS is not set: opencode also scans
         ~/.claude/skills and loads plugin skills under short, colliding names
         (spec, build, ...). Add to your shell profile:
           export OPENCODE_DISABLE_CLAUDE_CODE_SKILLS=1
EOF
  fi
}

case "$TARGET" in
  claude) install_claude ;;
  opencode) install_opencode ;;
  all) install_claude; install_opencode ;;
esac

((UNINSTALL)) || echo "done — start a new Claude Code / opencode session to pick up a changed set of skills."
