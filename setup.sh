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
# opencode configuration (asked, [Y/n]; --yes applies, --no-config-edits skips):
#   ~/.config/opencode/opencode.json  permission rules mirroring the git and db
#                                     guard hooks, merged in (jq, else python3);
#                                     a file with comments is left alone
#   your shell's rc file              export OPENCODE_DISABLE_CLAUDE_CODE_SKILLS=1
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
  --yes        apply the opencode config edits without asking
  --no-config-edits
               never edit opencode.json or a shell profile; print what to add
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

# ------------------------------------------------------ opencode config

OPENCODE_CONFIG_FILE="${OPENCODE_CONFIG:-$HOME/.config/opencode/opencode.json}"
AI_GENT_STATE="${XDG_STATE_HOME:-$HOME/.local/state}/ai-gent"
PERMISSIONS_STATE="$AI_GENT_STATE/opencode-permissions.json"
CREATED_STATE="$AI_GENT_STATE/opencode-config-created"

# The rules mirroring the git and db guard hooks (README § Guard hooks).
# opencode evaluates them in order and the last match wins, so they are
# appended after the user's own rules.
GUARD_RULES='{
  "bash": {
    "git push*": "ask",
    "git commit*--no-verify*": "deny",
    "git branch -D*": "ask",
    "git reset --hard*": "ask",
    "gh pr create*": "ask",
    "psql*": "ask",
    "mysql*": "ask",
    "sqlcmd*": "ask",
    "sqlplus*": "ask",
    "sqlite3*": "ask",
    "pg_restore*": "deny",
    "*connections.env*": "deny"
  },
  "read": { "~/.config/ai-gent/**": "deny" },
  "edit": { "~/.config/ai-gent/**": "deny" }
}'

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

# jq when installed (AI_GENT_JSON_TOOL=python forces the python3 fallback).
use_jq() { [[ "${AI_GENT_JSON_TOOL:-}" != python ]] && command -v jq >/dev/null 2>&1; }

# json_tool check|conflicts|missing|merge|remove <file> [json] — jq, else python3.
json_tool() {
  local action="$1" file="$2"
  shift 2
  if use_jq; then
    case "$action" in
      check) jq -e 'type == "object" and ((.permission // {}) | type == "object")' "$file" >/dev/null 2>&1 ;;
      empty) jq -e 'del(."$schema") | (.permission // {}) == {} and (del(.permission) == {})' "$file" >/dev/null 2>&1 ;;
      conflicts) jq -r --argjson r "$GUARD_RULES" '
          . as $doc | $r | to_entries[] | .key as $t | ($doc.permission[$t] // {}) as $cur
          | if ($cur | type) != "object" then "\($t): is a single action (\($cur)), left as it is"
            else .value | to_entries[] as $e | select(($cur | has($e.key)) and $cur[$e.key] != $e.value)
              | "\($t) \($e.key): yours is \($cur[$e.key]), kept" end' "$file" | sort -u ;;
      missing) jq -c --argjson r "$GUARD_RULES" '
          . as $doc | $r | with_entries(.key as $t | ($doc.permission[$t] // {}) as $cur
            | .value |= (if ($cur | type) == "object" then with_entries(. as $e | select($cur | has($e.key) | not)) else {} end))
          | with_entries(select(.value != {}))' "$file" ;;
      merge) jq --indent 2 --argjson add "$1" '
          reduce ($add | to_entries[]) as $e (.; .permission[$e.key] = ((.permission[$e.key] // {}) + $e.value))' "$file" ;;
      remove) jq --indent 2 --argjson rm "$1" '
          reduce ($rm | to_entries[] | .key as $t | .value | to_entries[] | {t: $t, k: .key, v: .value}) as $e (.;
            if (.permission[$e.t] | type) == "object" and .permission[$e.t][$e.k] == $e.v
            then .permission[$e.t] |= del(.[$e.k]) else . end)
          | .permission |= with_entries(select(.value != {}))' "$file" ;;
    esac
  else
    python3 - "$action" "$file" "$GUARD_RULES" "$@" <<'PY'
import json, sys
action, path, rules = sys.argv[1], sys.argv[2], json.loads(sys.argv[3])
try:
    doc = json.load(open(path))
except (OSError, ValueError):
    sys.exit(1)
perm = doc.get("permission", {}) if isinstance(doc, dict) else None
if action == "check":
    sys.exit(0 if isinstance(doc, dict) and isinstance(perm, dict) else 1)
if action == "empty":
    rest = {k: v for k, v in doc.items() if k not in ("$schema", "permission")}
    sys.exit(0 if not rest and not perm else 1)
if action == "conflicts":
    out = set()
    for t, rs in rules.items():
        cur = perm.get(t, {})
        for k, v in rs.items():
            if not isinstance(cur, dict):
                out.add(f"{t}: is a single action ({cur}), left as it is")
            elif k in cur and cur[k] != v:
                out.add(f"{t} {k}: yours is {cur[k]}, kept")
    print("\n".join(sorted(out)))
elif action == "missing":
    miss = {t: {k: v for k, v in rs.items() if isinstance(perm.get(t, {}), dict) and k not in perm.get(t, {})}
            for t, rs in rules.items()}
    print(json.dumps({t: rs for t, rs in miss.items() if rs}))
elif action == "merge":
    doc.setdefault("permission", {})
    for t, rs in json.loads(sys.argv[4]).items():
        doc["permission"].setdefault(t, {}).update(rs)
    print(json.dumps(doc, indent=2, ensure_ascii=False))
elif action == "remove":
    for t, rs in json.loads(sys.argv[4]).items():
        cur = doc.get("permission", {}).get(t)
        if isinstance(cur, dict):
            for k, v in rs.items():
                if cur.get(k) == v:
                    del cur[k]
            if not cur:
                del doc["permission"][t]
    print(json.dumps(doc, indent=2, ensure_ascii=False))
PY
  fi
}

print_rules_snippet() {
  printf '         {\n           "permission": %s\n         }\n' "$(printf '%s' "$GUARD_RULES" | sed '2,$s/^/           /')" >&2
}

opencode_permissions() {
  local file="$OPENCODE_CONFIG_FILE"
  [[ ! -e "$file" && -e "${file%.json}.jsonc" ]] && file="${file%.json}.jsonc"
  if ! use_jq && ! command -v python3 >/dev/null 2>&1; then
    echo "note     neither jq nor python3 found: add these rules to $file yourself:" >&2
    print_rules_snippet
    return
  fi
  if [[ ! -e "$file" ]]; then
    if ((DRY_RUN)); then
      echo "  [dry-run] would ask to create $file with the guard permission rules"
      return
    fi
    if confirm "Create $file with permission rules mirroring the git and db guards?"; then
      run mkdir -p "$(dirname "$file")"
      write "$file" "$(printf '{\n  "$schema": "https://opencode.ai/config.json",\n  "permission": %s\n}\n' "$GUARD_RULES")
"
      record_permissions "$GUARD_RULES"
      ((DRY_RUN)) || { mkdir -p "$AI_GENT_STATE"; printf '%s\n' "$file" >"$CREATED_STATE"; }
      echo "create   $file (guard permission rules)"
    else
      echo "note     opencode has no permission rules for the guards; to add them, create $file with:" >&2
      print_rules_snippet
    fi
    return
  fi
  if ! json_tool check "$file"; then
    echo "note     $file has comments or isn't plain JSON (or its permission is a single action); left as it is. Add these rules yourself:" >&2
    print_rules_snippet
    return
  fi
  local missing conflicts
  if ! missing="$(json_tool missing "$file")" || ! conflicts="$(json_tool conflicts "$file")"; then
    echo "note     couldn't read the permission rules in $file; left as it is. Add these rules yourself:" >&2
    print_rules_snippet
    return
  fi
  [[ -n "$conflicts" ]] && printf 'keep     %s\n' "${conflicts//$'\n'/$'\n'keep     }"
  if [[ -z "$missing" || "$missing" == "{}" ]]; then
    echo "ok       opencode permission rules"
    return
  fi
  echo "missing  opencode permission rules: $missing"
  if ((DRY_RUN)); then
    echo "  [dry-run] would ask to merge them into $file"
    return
  fi
  if confirm "Merge the missing permission rules into $file (a backup is kept)?"; then
    local merged backup
    merged="$(json_tool merge "$file" "$missing")" || { echo "error    couldn't merge into $file" >&2; return; }
    backup="$file.bak-$(date +%Y%m%d%H%M%S)"
    cp -p "$file" "$backup"
    printf '%s\n' "$merged" >"$file.tmp.$$" && mv "$file.tmp.$$" "$file"
    record_permissions "$missing"
    echo "merge    $file (backup: $(basename "$backup"))"
  else
    echo "note     not merged; the rules to add are:" >&2
    print_rules_snippet
  fi
}

# record_permissions <json> — remember what was added, for --uninstall.
record_permissions() {
  ((DRY_RUN)) && return
  mkdir -p "$AI_GENT_STATE"
  local previous='{}'
  [[ -f "$PERMISSIONS_STATE" ]] && previous="$(cat "$PERMISSIONS_STATE")"
  if use_jq; then
    jq -n --argjson a "$previous" --argjson b "$1" 'reduce ($b | to_entries[]) as $e ($a; .[$e.key] = ((.[$e.key] // {}) + $e.value))' >"$PERMISSIONS_STATE"
  else
    python3 -c 'import json,sys; a=json.loads(sys.argv[1]); b=json.loads(sys.argv[2]); [a.setdefault(t, {}).update(r) for t, r in b.items()]; print(json.dumps(a, indent=2))' "$previous" "$1" >"$PERMISSIONS_STATE"
  fi
}

uninstall_opencode_permissions() {
  local file="$OPENCODE_CONFIG_FILE"
  [[ ! -e "$file" && -e "${file%.json}.jsonc" ]] && file="${file%.json}.jsonc"
  [[ -f "$PERMISSIONS_STATE" && -f "$file" ]] || return 0
  if ! json_tool check "$file"; then
    echo "note     $file isn't plain JSON anymore; remove the ai-gent permission rules by hand (listed in $PERMISSIONS_STATE)" >&2
    return
  fi
  echo "remove   the permission rules this script added to $(basename "$file")"
  if ((!DRY_RUN)); then
    local cleaned
    cleaned="$(json_tool remove "$file" "$(cat "$PERMISSIONS_STATE")")" || {
      echo "note     couldn't edit $file; remove the ai-gent permission rules by hand (listed in $PERMISSIONS_STATE)" >&2
      return
    }
    printf '%s\n' "$cleaned" >"$file.tmp.$$" && mv "$file.tmp.$$" "$file"
    rm -f "$PERMISSIONS_STATE"
    # A file this script created goes too, if nothing else was ever added to it.
    if [[ -f "$CREATED_STATE" && "$(cat "$CREATED_STATE")" == "$file" ]] && json_tool empty "$file"; then
      echo "remove   $(basename "$file") (created by this script, now empty)"
      rm -f "$file"
    fi
    rm -f "$CREATED_STATE"
  fi
}

# ------------------------------------------------------- shell profile

PROFILE_BEGIN="# >>> ai-gent >>>"
PROFILE_END="# <<< ai-gent <<<"

shell_profile() { # prints the rc file for the user's shell
  case "$(basename "${SHELL:-sh}")" in
    zsh) echo "${ZDOTDIR:-$HOME}/.zshrc" ;;
    bash) [[ "$(uname -s)" == Darwin ]] && echo "$HOME/.bash_profile" || echo "$HOME/.bashrc" ;;
    fish) echo "${XDG_CONFIG_HOME:-$HOME/.config}/fish/conf.d/ai-gent.fish" ;;
    *) echo "$HOME/.profile" ;;
  esac
}

opencode_env() {
  local rc line
  rc="$(shell_profile)"
  if [[ -f "$rc" ]] && grep -q 'OPENCODE_DISABLE_CLAUDE_CODE_SKILLS' "$rc"; then
    echo "ok       OPENCODE_DISABLE_CLAUDE_CODE_SKILLS in $rc"
    return
  fi
  [[ "$rc" == *.fish ]] && line="set -gx OPENCODE_DISABLE_CLAUDE_CODE_SKILLS 1" || line="export OPENCODE_DISABLE_CLAUDE_CODE_SKILLS=1"
  if [[ "${OPENCODE_DISABLE_CLAUDE_CODE_SKILLS:-}" == "1" ]]; then
    echo "ok       OPENCODE_DISABLE_CLAUDE_CODE_SKILLS is set in this environment"
    return
  fi
  if ((DRY_RUN)); then
    echo "  [dry-run] would ask to add '$line' to $rc"
    return
  fi
  if confirm "Add '$line' to $rc, so opencode doesn't also load ~/.claude/skills under colliding names?"; then
    run mkdir -p "$(dirname "$rc")"
    if ((!DRY_RUN)); then
      printf '\n%s\n%s\n%s\n' "$PROFILE_BEGIN" "$line" "$PROFILE_END" >>"$rc"
    fi
    echo "profile  $rc (open a new terminal to pick it up)"
  else
    cat >&2 <<EOF
note     OPENCODE_DISABLE_CLAUDE_CODE_SKILLS is not set: opencode also scans
         ~/.claude/skills and loads plugin skills under short, colliding names
         (spec, build, ...). Add to $rc:
           $line
EOF
  fi
}

uninstall_opencode_env() {
  local rc
  rc="$(shell_profile)"
  [[ -f "$rc" ]] && grep -qF "$PROFILE_BEGIN" "$rc" || return 0
  echo "remove   the ai-gent block from $rc"
  ((DRY_RUN)) && return
  if [[ "$rc" == */conf.d/ai-gent.fish ]]; then
    rm -f "$rc"
  else
    # Drop the block and the blank line this script put before it.
    awk -v b="$PROFILE_BEGIN" -v e="$PROFILE_END" '
      skip { if ($0 == e) skip = 0; next }
      $0 == b { skip = 1; pending = 0; next }
      pending { print ""; pending = 0 }
      $0 == "" { pending = 1; next }
      { print }
      END { if (pending) print "" }' "$rc" >"$rc.tmp.$$" &&
      cat "$rc.tmp.$$" >"$rc" && rm -f "$rc.tmp.$$"
  fi
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
    uninstall_opencode_permissions
    uninstall_opencode_env
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

  opencode_permissions
  opencode_env
}

case "$TARGET" in
  claude) install_claude ;;
  opencode) install_opencode ;;
  all) install_claude; install_opencode ;;
esac

((UNINSTALL)) || echo "done — start a new Claude Code / opencode session to pick up a changed set of skills."
