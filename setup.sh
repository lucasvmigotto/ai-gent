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
#   opencode V2 also scans ~/.claude/skills, with no switch to disable it, so
#   the config edit below denies those short, colliding IDs (spec, build, ...)
#   with `skill` permission rules, leaving only the namespaced ones.
#
# opencode configuration (asked, [Y/n]; --yes applies, --no-config-edits skips):
#   ~/.config/opencode/opencode.json  OpenCode V2 `permissions` rules mirroring
#                                     the git and db guard hooks, the `plugins`
#                                     entry that loads opencode/guard, and
#                                     `skill` denies that hide the short,
#                                     colliding IDs OpenCode finds in
#                                     ~/.claude/skills. A file with comments is
#                                     left alone (see scripts/opencode-config.py).
#   Both are undone by --uninstall (only what this script added).
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
YES=0
NO_CONFIG_EDITS=0
VERBOSE=0
TARGET="all"

usage() {
  cat <<EOF
Usage: $(basename "$0") [--target claude|opencode|all] [--dry-run] [--force] [--uninstall] [--verbose]

Installs skills/* and plugins/* from $REPO_DIR for:
  claude    symlinks into $CLAUDE_DIR            (override: CLAUDE_SKILLS_DIR)
  opencode  links and generated entry files into $OPENCODE_SKILLS
            and commands into $OPENCODE_COMMANDS (override: OPENCODE_SKILLS_DIR,
            OPENCODE_COMMANDS_DIR); skipped when opencode isn't installed
  all       both (default)

  --dry-run    print what would change, change nothing
  --force      back up and replace a real directory or foreign symlink in the way
  --uninstall  remove everything this script installed for the chosen target(s)
  --yes        apply the opencode config edits without asking
  --no-config-edits
               never edit opencode.json; print what to add instead
  --verbose    one line per skill and plugin instead of a summary per status

Colors, bold and italics are used on a terminal; NO_COLOR=1 turns them off.
EOF
}

while (($#)); do
  case "$1" in
    --target) TARGET="${2:-}"; shift ;;
    --target=*) TARGET="${1#*=}" ;;
    --dry-run) DRY_RUN=1 ;;
    --force) FORCE=1 ;;
    --uninstall) UNINSTALL=1 ;;
    --yes | -y) YES=1 ;;
    --no-config-edits) NO_CONFIG_EDITS=1 ;;
    --verbose | -v) VERBOSE=1 ;;
    -h | --help) usage; exit 0 ;;
    *) echo "unknown option: $1" >&2; usage >&2; exit 2 ;;
  esac
  shift
done
case "$TARGET" in claude | opencode | all) ;; *) echo "invalid --target: $TARGET" >&2; exit 2 ;; esac

run() {
  if ((DRY_RUN)); then
    ((VERBOSE)) && echo "  [dry-run] $*"
    return 0
  else
    "$@"
  fi
}

# write <file> <content> — honors --dry-run
write() {
  if ((DRY_RUN)); then
    ((VERBOSE)) && echo "  [dry-run] write $1"
    return 0
  else
    printf '%s' "$2" >"$1"
  fi
}

# ------------------------------------------------------------------- output
#
# Each skill or plugin a section installs is an `item` with a status. On a
# terminal, the current item is shown on one line that's rewritten in place;
# at the end of the section (`section_end`) one line per status remains:
#   [ok]       [8/9]
#   [link]     [1/9]  compact
# Without a terminal (a pipe, CI), only the summary is printed. --verbose
# prints one line per item instead. `line` and `warn` print a line that stays
# (warn on stderr); skip reasons are printed after the summary.

OUT_TTY=0 ERR_TTY=0
[[ -t 1 ]] && OUT_TTY=1
[[ -t 2 ]] && ERR_TTY=1
PROGRESS=0
((OUT_TTY && !VERBOSE)) && PROGRESS=1

# styles: set when the stream is a terminal that has colors and NO_COLOR is unset
styles() { # styles <is a terminal: 1 or 0> — prints "bold dim italic reset green cyan yellow red"
  if (($1)) && [[ -z "${NO_COLOR:-}" && "${TERM:-dumb}" != dumb ]] &&
    (($(tput colors 2>/dev/null || echo 0) >= 8)); then
    printf '%s\n' $'\e[1m' $'\e[2m' $'\e[3m' $'\e[0m' $'\e[32m' $'\e[36m' $'\e[33m' $'\e[31m'
  else
    printf '%s\n' '' '' '' '' '' '' '' ''
  fi
}
{ read -r B; read -r D; read -r I; read -r R; read -r GREEN; read -r CYAN; read -r YELLOW; read -r RED; } < <(styles "$OUT_TTY")
{ read -r _; read -r _; read -r EI; read -r ER; read -r _; read -r _; read -r EYELLOW; read -r ERED; } < <(styles "$ERR_TTY")

color_of() { # color_of <status> [stderr] — the status's color for stdout or stderr
  local err="${2:-}"
  case "$1" in
    ok | keep) printf '%s' "$GREEN" ;;
    link | relink | generate | create | merge) printf '%s' "$CYAN" ;;
    backup | unlink | remove | replace | note | missing | dry-run)
      if [[ -n $err ]]; then printf '%s' "$EYELLOW"; else printf '%s' "$YELLOW"; fi ;;
    skip | error) if [[ -n $err ]]; then printf '%s' "$ERED"; else printf '%s' "$RED"; fi ;;
  esac
}

tag() { # tag <status> <label> <width> [err] — the label in the status's color, padded
  local reset="$R"
  [[ -n ${4:-} ]] && reset="$ER"
  printf '%s%s%s%*s' "$(color_of "$1" "${4:-}")" "$2" "$reset" $(($3 - ${#2})) ""
}

SECTION_ITEMS=""  # "status name" per line
SECTION_TOTAL=0   # items the section installs (removals and backups aren't counted)
SECTION_NOTES=""  # skip reasons, printed after the summary
SHOWN=0           # a progress line is on screen

clear_progress() {
  if ((SHOWN)); then printf '\r\e[K'; SHOWN=0; fi
}

section() { # section <title> <path>
  SECTION_ITEMS="" SECTION_TOTAL=0 SECTION_NOTES=""
  printf '%s== %s%s %s%s%s\n' "$B" "$1" "$R" "$I" "$2" "$R"
}

# item <status> <name> [reason] — the outcome for one skill or plugin
item() {
  local status="$1" name="$2" reason="${3:-}"
  case "$status" in unlink | remove | backup) ;; *) SECTION_TOTAL=$((SECTION_TOTAL + 1)) ;; esac
  SECTION_ITEMS+="$status $name"$'\n'
  if ((VERBOSE)); then
    if [[ $status == skip ]]; then
      printf '%s %s%s\n' "$(tag skip "$status" 8 err)" "$name" "${reason:+ ($reason)}" >&2
    else
      printf '%s %s\n' "$(tag "$status" "$status" 8)" "$name"
    fi
    return
  fi
  [[ $status == skip ]] && SECTION_NOTES+="$name: $reason"$'\n'
  if ((PROGRESS)); then
    printf '\r\e[K%s %s' "$(tag "$status" "[$status]" 11)" "$name"
    SHOWN=1
  fi
}

# names_of <status> — the section's items with that status, one per line
names_of() {
  local status name
  while read -r status name; do
    if [[ $status == "$1" ]]; then printf '%s\n' "$name"; fi
  done <<<"$SECTION_ITEMS"
}

section_end() {
  clear_progress
  ((VERBOSE)) && return
  local status names n shown count
  for status in ok link relink generate backup unlink remove skip; do
    names="$(names_of "$status")"
    [[ -n $names ]] || continue
    n="$(printf '%s\n' "$names" | wc -l | tr -d ' ')"
    case "$status" in
      unlink | remove | backup) count="[$n]" ;;
      *) count="[$n/$SECTION_TOTAL]" ;;
    esac
    shown=""
    if [[ $status != ok ]]; then
      # up to 8 names, then how many more
      shown="$(printf '%s\n' "$names" | head -n 8 | paste -sd, - | sed 's/,/, /g')"
      ((n > 8)) && shown+=" … (+$((n - 8)))"
    fi
    if [[ -n $shown ]]; then
      printf '%s %-9s %s%s%s\n' "$(tag "$status" "[$status]" 11)" "$count" "$D" "$shown" "$R"
    else
      printf '%s %s\n' "$(tag "$status" "[$status]" 11)" "$count"
    fi
  done
  if [[ -n $SECTION_NOTES ]]; then
    while IFS= read -r note; do
      if [[ -n $note ]]; then printf '%-11s %s\n' "" "$EI$note$ER" >&2; fi
    done <<<"$SECTION_NOTES"
  fi
  return 0
}

# line <status> <text> — a line that stays; warn does the same on stderr
line() {
  clear_progress
  if ((VERBOSE)); then
    printf '%s %s\n' "$(tag "$1" "$1" 8)" "$2"
  else
    printf '%s %s\n' "$(tag "$1" "[$1]" 11)" "$2"
  fi
}
warn() {
  clear_progress
  if ((VERBOSE)); then
    printf '%s %s\n' "$(tag "$1" "$1" 8 err)" "$2" >&2
  else
    printf '%s %s\n' "$(tag "$1" "[$1]" 11 err)" "$2" >&2
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
      item ok "$name"
      return
    fi
    if ! points_into_repo "$dest" && ((!FORCE)); then
      item skip "$name" "symlink to $(readlink "$dest"); use --force"
      return
    fi
    item relink "$name"
    run ln -sfn "$src" "$dest"
  elif [[ -e "$dest" ]]; then
    if ((!FORCE)); then
      item skip "$name" "real file or directory in the way; use --force to back it up"
      return
    fi
    local backup
    backup="$dest.bak-$(date +%Y%m%d%H%M%S)"
    item backup "$name -> $(basename "$backup")"
    run mv "$dest" "$backup"
    item link "$name"
    run ln -s "$src" "$dest"
  else
    item link "$name"
    run ln -s "$src" "$dest"
  fi
}

# Remove symlinks into this repo whose source is gone, or all of them on --uninstall.
prune_links() {
  local dir="$1" dest
  for dest in "$dir"/*; do
    if [[ ! -L "$dest" ]] || ! points_into_repo "$dest"; then continue; fi
    if ((UNINSTALL)) || [[ ! -e "$dest" ]]; then
      item unlink "$(basename "$dest")"
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
  section "Claude Code:" "$CLAUDE_DIR"
  run mkdir -p "$CLAUDE_DIR"
  prune_links "$CLAUDE_DIR"
  if ((UNINSTALL)); then section_end; return; fi

  local dir
  for dir in "$REPO_DIR"/skills/*/; do
    dir="${dir%/}"
    [[ -f "$dir/SKILL.md" ]] && link "$dir" "$CLAUDE_DIR/$(basename "$dir")"
  done
  for dir in "$REPO_DIR"/plugins/*/; do
    dir="${dir%/}"
    [[ -f "$dir/.claude-plugin/plugin.json" ]] && link "$dir" "$CLAUDE_DIR/$(basename "$dir")"
  done
  section_end
}

# ------------------------------------------------------------------- opencode

opencode_entry() { # opencode_entry <plugin> <skill> <source SKILL.md>
  local plugin="$1" skill="$2" src="$3"
  local name="$plugin-$skill"
  local desc
  desc="$(frontmatter "$src" description)"
  if ((${#desc} < 1 || ${#desc} > 1024)); then
    item skip "$name" "description is ${#desc} chars; opencode needs 1-1024"
    return
  fi

  local dir="$OPENCODE_SKILLS/$name"
  if [[ -e "$dir" && ! -f "$dir/$MARKER" ]]; then
    item skip "$name" "exists and wasn't generated by this script"
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
    item ok "$name"
  else
    item generate "$name"
    run mkdir -p "$dir"
    write "$dir/SKILL.md" "$content"
    write "$dir/$MARKER" ""
  fi

  # /<plugin>-<skill> command
  local summary="${desc%%. *}"
  ((${#summary} > 160)) && summary="${summary:0:157}..."
  local cmd="$OPENCODE_COMMANDS/$name.md"
  if [[ -f "$cmd" ]] && ! grep -q 'Generated by ai-gent/setup.sh' "$cmd"; then
    warn skip "/$name (command exists and wasn't generated by this script)"
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
      item remove "$name"
      run rm -rf "$dir"
      [[ -f "$OPENCODE_COMMANDS/$name.md" ]] && run rm "$OPENCODE_COMMANDS/$name.md"
    fi
  done
}

# ------------------------------------------------------ opencode config

OPENCODE_CONFIG_FILE="${OPENCODE_CONFIG:-$HOME/.config/opencode/opencode.json}"
AI_GENT_STATE="${XDG_STATE_HOME:-$HOME/.local/state}/ai-gent"
CONFIG_STATE="$AI_GENT_STATE/opencode-config.json"
CREATED_STATE="$AI_GENT_STATE/opencode-config-created"
OC_PY="$REPO_DIR/scripts/opencode-config.py"
GUARD_PLUGIN="$REPO_DIR/opencode/guard"
SKILL_NAMES=""

# The OpenCode V2 `permissions` array and the `plugins` entry are built by
# scripts/opencode-config.py from the guard rules plus the plugin skill names
# (so the short, colliding IDs OpenCode finds in ~/.claude/skills are denied).
oc() { python3 "$OC_PY" "$@"; }

# confirm <question> — [Y/n]. --yes answers yes; --no-config-edits, --dry-run
# and "no terminal to ask on" (a pipe, CI) answer no.
confirm() {
  ((DRY_RUN || NO_CONFIG_EDITS)) && return 1
  ((YES)) && return 0
  (exec </dev/tty) 2>/dev/null || return 1
  local answer
  printf '%s [Y/n] ' "$1" >/dev/tty
  read -r answer </dev/tty || return 1
  [[ -z "$answer" || "$answer" == [Yy]* ]]
}

# opencode V2 config: the guard permission rules, the guard plugin, and the
# `skill` denies that hide the short, colliding IDs OpenCode finds in
# ~/.claude/skills. scripts/opencode-config.py builds all three.

print_rules_snippet() {
  if command -v python3 >/dev/null 2>&1; then
    printf '  "permissions": %s\n' "$(oc rules --skills "$SKILL_NAMES")" >&2
  else
    echo '  "permissions": [ ... ]  (run scripts/opencode-config.py rules)' >&2
  fi
  printf '  "plugins": ["%s"]\n' "$GUARD_PLUGIN" >&2
}

opencode_permissions() {
  local file="$OPENCODE_CONFIG_FILE"
  [[ ! -e "$file" && -e "${file%.json}.jsonc" ]] && file="${file%.json}.jsonc"
  if ! command -v python3 >/dev/null 2>&1; then
    warn note "python3 not found: add the OpenCode guard rules and plugin yourself:"
    print_rules_snippet
    return
  fi
  if [[ ! -e "$file" ]]; then
    if ((DRY_RUN)); then
      line dry-run "would ask to create $file with the guard rules and plugin"
      return
    fi
    if confirm "Create $file with the OpenCode guard rules and plugin?"; then
      run mkdir -p "$(dirname "$file")"
      oc new --skills "$SKILL_NAMES" --plugin "$GUARD_PLUGIN" >"$file.tmp.$$" && mv "$file.tmp.$$" "$file"
      oc record "$CONFIG_STATE" "$(oc rules --skills "$SKILL_NAMES")" --plugin "$GUARD_PLUGIN"
      ((DRY_RUN)) || { mkdir -p "$AI_GENT_STATE"; printf '%s\n' "$file" >"$CREATED_STATE"; }
      line create "$file (guard rules and plugin)"
    else
      warn note "opencode has no guard rules or plugin; add these to $file yourself:"
      print_rules_snippet
    fi
    return
  fi
  if ! oc check "$file"; then
    warn note "$file has comments or isn't plain JSON; left as it is. Add these yourself:"
    print_rules_snippet
    return
  fi
  local missing conflicts plugin_missing=0 plugin_arg=''
  missing="$(oc missing "$file" --skills "$SKILL_NAMES")" || missing='[]'
  conflicts="$(oc conflicts "$file" --skills "$SKILL_NAMES")" || conflicts=''
  oc plugin-missing "$file" "$GUARD_PLUGIN" || plugin_missing=1
  if [[ -n "$conflicts" ]]; then
    local conflict
    while IFS= read -r conflict; do line keep "$conflict"; done <<<"$conflicts"
  fi
  if [[ "$missing" == "[]" && $plugin_missing -eq 0 ]]; then
    line ok "opencode guard rules and plugin"
    return
  fi
  line missing "opencode guard rules: $missing"
  ((plugin_missing)) && line missing "opencode guard plugin: $GUARD_PLUGIN"
  if ((DRY_RUN)); then
    line dry-run "would ask to merge them into $file"
    return
  fi
  if confirm "Merge the guard rules and plugin into $file (a backup is kept)?"; then
    local merged backup
    ((plugin_missing)) && plugin_arg="$GUARD_PLUGIN"
    merged="$(oc apply "$file" --skills "$SKILL_NAMES" --plugin "$plugin_arg")" || {
      warn error "couldn't merge into $file"
      return
    }
    backup="$file.bak-$(date +%Y%m%d%H%M%S)"
    cp -p "$file" "$backup"
    printf '%s\n' "$merged" >"$file.tmp.$$" && mv "$file.tmp.$$" "$file"
    oc record "$CONFIG_STATE" "$missing" --plugin "$plugin_arg"
    line merge "$file (backup: $(basename "$backup"))"
  else
    warn note "not merged; add these to $file yourself:"
    print_rules_snippet
  fi
}

uninstall_opencode_permissions() {
  local file="$OPENCODE_CONFIG_FILE"
  [[ ! -e "$file" && -e "${file%.json}.jsonc" ]] && file="${file%.json}.jsonc"
  [[ -f "$CONFIG_STATE" && -f "$file" ]] || return 0
  if ! oc check "$file"; then
    warn note "$file isn't plain JSON anymore; remove the ai-gent rules by hand (listed in $CONFIG_STATE)"
    return
  fi
  line remove "the guard rules and plugin this script added to $(basename "$file")"
  if ((!DRY_RUN)); then
    local cleaned
    cleaned="$(oc remove "$file" "$CONFIG_STATE")" || {
      warn note "couldn't edit $file; remove the ai-gent rules by hand (listed in $CONFIG_STATE)"
      return
    }
    printf '%s\n' "$cleaned" >"$file.tmp.$$" && mv "$file.tmp.$$" "$file"
    rm -f "$CONFIG_STATE"
    # A file this script created goes too, if nothing else was ever added to it.
    if [[ -f "$CREATED_STATE" && "$(cat "$CREATED_STATE")" == "$file" ]] && oc empty "$file"; then
      line remove "$(basename "$file") (created by this script, now empty)"
      rm -f "$file"
    fi
    rm -f "$CREATED_STATE"
  fi
}

install_opencode() {
  if ! command -v opencode >/dev/null 2>&1 && [[ ! -d "$(dirname "$OPENCODE_SKILLS")" ]]; then
    section "opencode:" "not installed, skipped"
    return
  fi
  section "opencode:" "$OPENCODE_SKILLS"

  # A symlink to the Claude skills dir would make generated entries appear
  # as Claude personal skills too; opencode needs its own real directory.
  if [[ -L "$OPENCODE_SKILLS" ]]; then
    if [[ "$(readlink -f "$OPENCODE_SKILLS")" == "$(readlink -f "$CLAUDE_DIR")" ]] || ((FORCE)); then
      line replace "$OPENCODE_SKILLS (symlink to $(readlink "$OPENCODE_SKILLS")) with a real directory"
      run rm "$OPENCODE_SKILLS"
    else
      warn skip "opencode ($OPENCODE_SKILLS is a symlink to $(readlink "$OPENCODE_SKILLS"); use --force)"
      return
    fi
  fi
  run mkdir -p "$OPENCODE_SKILLS" "$OPENCODE_COMMANDS"

  prune_links "$OPENCODE_SKILLS"
  prune_opencode_generated
  if ((UNINSTALL)); then
    if [[ -L "$OPENCODE_SKILLS/synced" ]]; then
      item unlink synced
      run rm "$OPENCODE_SKILLS/synced"
    fi
    section_end
    uninstall_opencode_permissions
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
      [[ -f "$skill/SKILL.md" ]] || continue
      SKILL_NAMES="$SKILL_NAMES $(basename "$skill")"
      opencode_entry "$(basename "$dir")" "$(basename "$skill")" "$skill/SKILL.md"
    done
  done
  section_end

  opencode_permissions
}

case "$TARGET" in
  claude) install_claude ;;
  opencode) install_opencode ;;
  all) install_claude; install_opencode ;;
esac

if ((DRY_RUN)); then
  printf '%sdry run — nothing was changed.%s\n' "$I" "$R"
elif ((!UNINSTALL)); then
  printf '%sdone%s — start a new Claude Code / opencode session to pick up a changed set of skills.\n' "$B" "$R"
fi
