#!/usr/bin/env bash
# Install this repo's skills and plugins for Claude Code (the default) and,
# when chosen, other coding agents, so the repo stays the single place to edit.
#
# Which agents: on a terminal, an fzf selector (a numbered prompt without fzf)
# with Claude Code and the previous choice preselected; --target <list> or
# --skip-prompt instead. The choice is kept in
# ${XDG_STATE_HOME:-~/.local/state}/ai-gent/targets for the next run.
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
# Codex, Cursor, Gemini CLI, GitHub Copilot, Amp (one shared directory that
# all five read; installing per tool would list every skill twice):
#   skills/<name>/, plugins/<p>/skills/<s>/ -> ~/.agents/skills/<name>, <p>-<s>
#                                              (generated entry files, not
#                                              symlinks, which Cursor skips)
# Kiro      the same entry files in ~/.kiro/skills
# Windsurf  the same entry files in ~/.codeium/windsurf/skills
#   These agents don't run the guard hooks (git, db, secrets): there, they are
#   instructions only.
#
# opencode configuration (asked, [Y/n]; --yes applies, --no-config-edits skips):
#   ~/.config/opencode/opencode.json  OpenCode V2 `permissions` rules mirroring
#                                     the git, db and secrets guard hooks, the `plugins`
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
AGENTS_SKILLS="${AGENTS_SKILLS_DIR:-$HOME/.agents/skills}"
KIRO_SKILLS="${KIRO_SKILLS_DIR:-$HOME/.kiro/skills}"
WINDSURF_SKILLS="${WINDSURF_SKILLS_DIR:-$HOME/.codeium/windsurf/skills}"
MARKER=".ai-gent-generated"
AI_GENT_STATE="${XDG_STATE_HOME:-$HOME/.local/state}/ai-gent"
TARGETS_STATE="$AI_GENT_STATE/targets"

DRY_RUN=0
FORCE=0
UNINSTALL=0
YES=0
NO_CONFIG_EDITS=0
VERBOSE=0
SKIP_PROMPT=0
TARGET=""

# The agents, in the selector's order: an id, a label, and the commands or
# directories that show it's installed (any one is enough).
AGENT_IDS=(claude opencode codex cursor gemini copilot amp kiro windsurf)
declare -A AGENT_LABEL=(
  [claude]="Claude Code" [opencode]="opencode" [codex]="Codex" [cursor]="Cursor"
  [gemini]="Gemini CLI" [copilot]="GitHub Copilot" [amp]="Amp" [kiro]="Kiro"
  [windsurf]="Windsurf"
)
declare -A AGENT_DETECT=(
  [claude]="claude .claude" [opencode]="opencode .config/opencode"
  [codex]="codex .codex" [cursor]="cursor cursor-agent .cursor"
  [gemini]="gemini .gemini" [copilot]="copilot .copilot"
  [amp]="amp .config/amp" [kiro]="kiro kiro-cli .kiro"
  [windsurf]="windsurf .codeium/windsurf"
)
# The agents that read the shared ~/.agents/skills directory.
SHARED_AGENTS=" codex cursor gemini copilot amp "

usage() {
  cat <<EOF
Usage: $(basename "$0") [--target <agents>] [--skip-prompt] [--dry-run] [--force] [--uninstall] [--verbose]

Installs skills/* and plugins/* from $REPO_DIR for the chosen coding agents.
On a terminal, a selector (fzf, or a numbered prompt) asks which, with Claude
Code and the previous choice preselected.

  --target <agents>
               comma-separated, instead of the selector: claude, opencode,
               codex, cursor, gemini, copilot, amp, kiro, windsurf, or all
               (every agent found on this machine)
  --skip-prompt
               no selector: the previous choice, or Claude Code alone the
               first time (also what happens without a terminal)
  --dry-run    print what would change, change nothing
  --force      back up and replace a real directory or foreign symlink in the way
  --uninstall  remove everything this script installed, or only for --target
  --yes        apply the opencode config edits without asking
  --no-config-edits
               never edit opencode.json; print what to add instead
  --verbose    one line per skill and plugin instead of a summary per status

Where each agent's skills go (override with the variable in parentheses):
  claude     $CLAUDE_DIR (CLAUDE_SKILLS_DIR), symlinks
  opencode   $OPENCODE_SKILLS (OPENCODE_SKILLS_DIR) and
             $OPENCODE_COMMANDS (OPENCODE_COMMANDS_DIR);
             skipped when opencode isn't installed
  codex, cursor, gemini, copilot, amp
             $AGENTS_SKILLS (AGENTS_SKILLS_DIR), shared
  kiro       $KIRO_SKILLS (KIRO_SKILLS_DIR)
  windsurf   $WINDSURF_SKILLS (WINDSURF_SKILLS_DIR)

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
    --skip-prompt) SKIP_PROMPT=1 ;;
    --yes | -y) YES=1 ;;
    --no-config-edits) NO_CONFIG_EDITS=1 ;;
    --verbose | -v) VERBOSE=1 ;;
    -h | --help) usage; exit 0 ;;
    *) echo "unknown option: $1" >&2; usage >&2; exit 2 ;;
  esac
  shift
done
for t in ${TARGET//,/ }; do
  if [[ $t != all && -z ${AGENT_LABEL[$t]:-} ]]; then
    echo "invalid --target: $t (one of: all ${AGENT_IDS[*]})" >&2
    exit 2
  fi
done

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

# skill_entry <skills dir> <name> <source SKILL.md> [commands dir] — a
# generated <name>/SKILL.md that points at the source (opencode, the shared
# ~/.agents/skills, Kiro, Windsurf), plus a /<name> command when a commands
# dir is given (opencode).
skill_entry() {
  local skills_dir="$1" name="$2" src="$3" commands_dir="${4:-}"
  local desc
  desc="$(frontmatter "$src" description)"
  if ((${#desc} < 1 || ${#desc} > 1024)); then
    item skip "$name" "description is ${#desc} chars; agents need 1-1024"
    return
  fi

  local dir="$skills_dir/$name"
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
  this agent's equivalents.
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
  [[ -n $commands_dir ]] || return 0
  local summary="${desc%%. *}"
  ((${#summary} > 160)) && summary="${summary:0:157}..."
  local cmd="$commands_dir/$name.md"
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

# prune_generated <skills dir> [commands dir] — remove generated entries whose
# source is gone, or all of them on --uninstall.
prune_generated() {
  local skills_dir="$1" commands_dir="${2:-}" dir name
  for dir in "$skills_dir"/*/; do
    dir="${dir%/}"
    [[ -f "$dir/$MARKER" ]] || continue
    name="$(basename "$dir")"
    local src
    src="$(sed -n 's/^  source: "\(.*\)"$/\1/p' "$dir/SKILL.md" 2>/dev/null || true)"
    if ((UNINSTALL)) || [[ ! -f "$src" ]]; then
      item remove "$name"
      run rm -rf "$dir"
      if [[ -n $commands_dir && -f "$commands_dir/$name.md" ]]; then run rm "$commands_dir/$name.md"; fi
    fi
  done
}

# ------------------------------------------------------ opencode config

OPENCODE_CONFIG_FILE="${OPENCODE_CONFIG:-$HOME/.config/opencode/opencode.json}"
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
  prune_generated "$OPENCODE_SKILLS" "$OPENCODE_COMMANDS"
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
      skill_entry "$OPENCODE_SKILLS" "$(basename "$dir")-$(basename "$skill")" "$skill/SKILL.md" "$OPENCODE_COMMANDS"
    done
  done
  section_end

  opencode_permissions
}

# ------------------------------------------- Codex, Cursor, Gemini, Kiro, ...

# install_entries <title> <skills dir> — a generated entry per personal skill
# and per plugin skill (<plugin>-<skill>), for agents that read SKILL.md
# folders but have no plugin namespaces and run no Claude Code hooks.
install_entries() {
  local title="$1" skills_dir="$2"
  section "$title:" "$skills_dir"
  if ((UNINSTALL)); then
    [[ -d "$skills_dir" ]] && prune_generated "$skills_dir"
    section_end
    return
  fi
  run mkdir -p "$skills_dir"
  prune_generated "$skills_dir"

  local dir skill
  for dir in "$REPO_DIR"/skills/*/; do
    dir="${dir%/}"
    [[ -f "$dir/SKILL.md" ]] && skill_entry "$skills_dir" "$(basename "$dir")" "$dir/SKILL.md"
  done
  for dir in "$REPO_DIR"/plugins/*/; do
    dir="${dir%/}"
    [[ -f "$dir/.claude-plugin/plugin.json" ]] || continue
    for skill in "$dir"/skills/*/; do
      skill="${skill%/}"
      [[ -f "$skill/SKILL.md" ]] || continue
      skill_entry "$skills_dir" "$(basename "$dir")-$(basename "$skill")" "$skill/SKILL.md"
    done
  done
  section_end
  line note "the git, db and secrets guards are instructions only here: these agents don't run Claude Code's hooks"
}

# ------------------------------------------------------------ agent selection

detected() { # detected <id> — the agent's command is on PATH or its directory exists
  local probe
  for probe in ${AGENT_DETECT[$1]}; do
    if [[ $probe == .* ]]; then
      [[ -e "$HOME/$probe" ]] && return 0
    else
      command -v "$probe" >/dev/null 2>&1 && return 0
    fi
  done
  return 1
}

# The previous choice; the first time, Claude Code, plus opencode when an
# earlier setup.sh (which installed it by default) generated its entries.
previous_agents() {
  if [[ -s "$TARGETS_STATE" ]]; then
    tr -s ' \n' ' ' <"$TARGETS_STATE"
    return
  fi
  printf 'claude'
  if compgen -G "$OPENCODE_SKILLS/*/$MARKER" >/dev/null; then printf ' opencode'; fi
}

# A terminal to ask on: stdout (checked at startup, since resolve_agents runs
# in a command substitution) and /dev/tty, which install.sh's `curl | sh` keeps.
has_tty() { ((OUT_TTY)) && (exec </dev/tty) 2>/dev/null; }

# choose_agents <preselected ids> — asks which agents to install for; prints
# their ids. fzf when installed, a numbered prompt otherwise.
choose_agents() {
  local pre=" $1 " id n=0 found
  if command -v fzf >/dev/null 2>&1; then
    local lines="" bind="start:"
    for id in "${AGENT_IDS[@]}"; do
      found="not found"
      detected "$id" && found="found"
      lines+="$id"$'\t'"${AGENT_LABEL[$id]}"$'\t'"($found)"$'\n'
      n=$((n + 1))
      [[ $pre == *" $id "* ]] && bind+="pos($n)+select+"
    done
    printf '%s' "$lines" | fzf --multi --delimiter=$'\t' --with-nth=2.. --height=~16 \
      --layout=reverse --border --prompt='agents> ' --bind "${bind}first" \
      --header=$'Tab selects, Enter installs. Claude Code is the default.\nCodex, Cursor, Gemini CLI, Copilot and Amp share ~/.agents/skills.' |
      cut -f1
    return
  fi
  local defaults=""
  {
    echo "Install ai-gent for which agents?"
    for id in "${AGENT_IDS[@]}"; do
      n=$((n + 1))
      found=""
      detected "$id" && found=" (found)"
      if [[ $pre == *" $id "* ]]; then
        printf '  %d) [x] %s%s\n' "$n" "${AGENT_LABEL[$id]}" "$found"
        defaults+="$n "
      else
        printf '  %d) [ ] %s%s\n' "$n" "${AGENT_LABEL[$id]}" "$found"
      fi
    done
    printf 'Numbers separated by spaces [%s]: ' "${defaults% }"
  } >/dev/tty
  local answer
  read -r answer </dev/tty || return 0
  [[ -n $answer ]] || answer="$defaults"
  for n in ${answer//,/ }; do
    if [[ $n =~ ^[0-9]+$ ]] && ((n >= 1 && n <= ${#AGENT_IDS[@]})); then
      echo "${AGENT_IDS[n - 1]}"
    else
      echo "ignored: $n" >/dev/tty
    fi
  done
}

# The agents to act on, in AGENT_IDS order.
resolve_agents() {
  local chosen id
  if ((UNINSTALL)) && [[ -z $TARGET ]]; then
    chosen="${AGENT_IDS[*]}"
  elif [[ -n $TARGET ]]; then
    chosen="${TARGET//,/ }"
    if [[ " $chosen " == *" all "* ]]; then
      chosen="claude"
      for id in "${AGENT_IDS[@]}"; do detected "$id" && chosen+=" $id"; done
    fi
  elif ((SKIP_PROMPT)) || ! has_tty; then
    chosen="$(previous_agents)"
  else
    chosen="$(choose_agents "$(previous_agents)" | tr '\n' ' ')"
    if [[ -z ${chosen// /} ]]; then
      echo "no agent chosen — nothing installed" >&2
      exit 1
    fi
  fi
  for id in "${AGENT_IDS[@]}"; do
    if [[ " $chosen " == *" $id "* ]]; then printf '%s ' "$id"; fi
  done
}

# remember <ids> — the choice for the next run (--skip-prompt, install.sh updates)
remember() {
  ((DRY_RUN)) && return 0
  local keep="$1" id
  if ((UNINSTALL)); then
    keep=""
    for id in $(previous_agents); do
      if [[ " $1 " != *" $id "* ]]; then keep+="$id "; fi
    done
  fi
  mkdir -p "$AI_GENT_STATE"
  if [[ -n ${keep// /} ]]; then
    printf '%s\n' "${keep% }" >"$TARGETS_STATE"
  else
    rm -f "$TARGETS_STATE"
  fi
}

AGENTS="$(resolve_agents)"
SHARED_DONE=0
labels=""
for id in $AGENTS; do
  labels+="${labels:+, }${AGENT_LABEL[$id]}"
  case "$id" in
    claude) install_claude ;;
    opencode) install_opencode ;;
    kiro) install_entries "Kiro" "$KIRO_SKILLS" ;;
    windsurf) install_entries "Windsurf" "$WINDSURF_SKILLS" ;;
    *)
      # One pass for every chosen agent that reads ~/.agents/skills.
      ((SHARED_DONE)) && continue
      SHARED_DONE=1
      shared=""
      for s in $AGENTS; do
        if [[ $SHARED_AGENTS == *" $s "* ]]; then shared+="${shared:+, }${AGENT_LABEL[$s]}"; fi
      done
      install_entries "$shared" "$AGENTS_SKILLS"
      ;;
  esac
done
remember "$AGENTS"

if ((DRY_RUN)); then
  printf '%sdry run — nothing was changed.%s\n' "$I" "$R"
elif ((!UNINSTALL)); then
  printf '%sdone%s — start a new %s session to pick up a changed set of skills.\n' "$B" "$R" "$labels"
fi
