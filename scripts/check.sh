#!/usr/bin/env bash
# Repository checks: skill and plugin metadata, cross-references, symlinks,
# shell scripts, the git guard hook, and installer tests in a throwaway HOME. Run before every commit; CI runs the same script.
#
#   scripts/check.sh                 everything
#   scripts/check.sh --quick         skip the installer tests
#
# Environment:
#   DESC_BUDGET  soft limit for skill descriptions, in characters (default 400);
#                opencode's hard limit is 1024
#   SHELLCHECK   shellcheck command (default: shellcheck on PATH, else skipped)

set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_DIR"

DESC_BUDGET="${DESC_BUDGET:-400}"
QUICK=0
[[ "${1:-}" == "--quick" ]] && QUICK=1

failures=0
fail() { printf 'FAIL  %s\n' "$*"; failures=$((failures + 1)); }
pass() { printf 'ok    %s\n' "$*"; }
warn() { printf 'warn  %s\n' "$*"; }

NAME_RE='^[a-z0-9]+(-[a-z0-9]+)*$'

frontmatter_block() { # the lines between the first two --- lines
  awk 'NR == 1 && $0 != "---" { exit } NR > 1 && $0 == "---" { exit } NR > 1 { print }' "$1"
}

frontmatter_value() { # frontmatter_value <file> <key>
  frontmatter_block "$1" | sed -n "s/^$2: //p" | head -n1
}

# ------------------------------------------------------------------ skills

check_skill() { # check_skill <SKILL.md> <expected name> <opencode name>
  local file="$1" expected="$2" oc_name="$3" name desc
  if [[ "$(head -n1 "$file")" != "---" ]]; then
    fail "$file: no frontmatter"
    return
  fi
  name="$(frontmatter_value "$file" name)"
  desc="$(frontmatter_value "$file" description)"

  [[ "$name" == "$expected" ]] || fail "$file: name '$name' should be '$expected' (the directory name)"
  [[ "$oc_name" =~ $NAME_RE ]] || fail "$file: '$oc_name' isn't a valid opencode skill name"

  if [[ -z "$desc" ]]; then
    fail "$file: description missing or not on a single line (setup.sh reads single-line values)"
  elif ((${#desc} > 1024)); then
    fail "$file: description is ${#desc} chars; opencode's limit is 1024"
  elif ((${#desc} > DESC_BUDGET)); then
    fail "$file: description is ${#desc} chars; budget is $DESC_BUDGET (every session loads it)"
  fi
  if [[ "$desc" == *": "* || "$desc" == *" #"* ]]; then
    fail "$file: description contains ': ' or ' #', which break an unquoted YAML value"
  fi
  if frontmatter_block "$file" | grep -q '^description: *[>|]'; then
    fail "$file: description must be a single-line value, not a YAML block"
  fi
}

skill_count=0
for file in skills/*/SKILL.md; do
  [[ -e "$file" ]] || continue
  dir="$(basename "$(dirname "$file")")"
  check_skill "$file" "$dir" "$dir"
  skill_count=$((skill_count + 1))
done
for file in plugins/*/skills/*/SKILL.md; do
  skill="$(basename "$(dirname "$file")")"
  plugin="$(basename "$(dirname "$(dirname "$(dirname "$file")")")")"
  check_skill "$file" "$skill" "$plugin-$skill"
  skill_count=$((skill_count + 1))
done
((failures)) || pass "$skill_count skills: names, descriptions (budget $DESC_BUDGET chars)"

# ----------------------------------------------------------------- plugins

before=$failures
for manifest in plugins/*/.claude-plugin/plugin.json; do
  dir="$(basename "$(dirname "$(dirname "$manifest")")")"
  python3 - "$manifest" "$dir" <<'EOF' || fail "$manifest"
import json, re, sys
path, expected = sys.argv[1], sys.argv[2]
with open(path) as f:
    m = json.load(f)
errors = []
if m.get("name") != expected:
    errors.append(f"name {m.get('name')!r} should be {expected!r}")
if not re.fullmatch(r"\d+\.\d+\.\d+", str(m.get("version", ""))):
    errors.append(f"version {m.get('version')!r} isn't MAJOR.MINOR.PATCH")
for key in ("description", "license", "author"):
    if not m.get(key):
        errors.append(f"missing {key}")
for e in errors:
    print(f"      {path}: {e}")
sys.exit(1 if errors else 0)
EOF
  compgen -G "plugins/$dir/skills/*/SKILL.md" >/dev/null || fail "plugins/$dir: no skills"
done
((failures == before)) && pass "plugin manifests"

if command -v claude >/dev/null 2>&1; then
  before=$failures
  for dir in plugins/*/; do
    out="$(claude plugin validate "$dir" 2>&1)" || { fail "claude plugin validate $dir"; printf '%s\n' "$out"; }
  done
  ((failures == before)) && pass "claude plugin validate"
else
  warn "claude not on PATH; skipped 'claude plugin validate'"
fi

# -------------------------------------------------------- cross-references

before=$failures
plugins_re="$(find plugins -mindepth 1 -maxdepth 1 -type d -printf '%f\n' | paste -sd'|')"
while IFS=: read -r file line ref; do
  plugin="${ref%%:*}"
  skill="${ref#*:}"
  [[ -f "plugins/$plugin/skills/$skill/SKILL.md" ]] || fail "$file:$line: '$ref' doesn't exist"
done < <(grep -rnoE "\b($plugins_re):[a-z][a-z0-9-]*" --include="*.md" plugins shared README.md AGENTS.md 2>/dev/null |
  grep -v '/evals/')

# Backticked paths that start with ../ or references/ resolve from the file's directory.
while IFS=: read -r file line match; do
  path="${match//\`/}"
  [[ -e "$(dirname "$file")/$path" ]] || fail "$file:$line: $path doesn't exist"
done < <(grep -rnoE '`(\.\./|references/)[^` ]*`' --include='*.md' plugins shared)

while IFS= read -r link; do
  fail "broken symlink: $link -> $(readlink "$link")"
done < <(find . -path ./.git -prune -o -type l ! -exec test -e {} \; -print)
((failures == before)) && pass "skill references, reference paths, symlinks"

# ---------------------------------------------------------- shell scripts

before=$failures
scripts=(setup.sh scripts/*.sh)
hooks=()
for script in plugins/*/hooks/*.sh; do [[ -e "$script" ]] && hooks+=("$script"); done
for script in "${scripts[@]}"; do
  bash -n "$script" || fail "bash -n $script"
done
sh -n install.sh || fail "sh -n install.sh"
for script in "${hooks[@]}"; do
  sh -n "$script" || fail "sh -n $script"
done

shellcheck_cmd="${SHELLCHECK:-}"
if [[ -z "$shellcheck_cmd" ]] && command -v shellcheck >/dev/null 2>&1; then
  shellcheck_cmd=shellcheck
fi
if [[ -n "$shellcheck_cmd" ]]; then
  # shellcheck disable=SC2086 # SHELLCHECK may be a command with arguments
  $shellcheck_cmd "${scripts[@]}" install.sh "${hooks[@]}" ||
    fail "shellcheck"
else
  warn "shellcheck not found; skipped (SHELLCHECK='uvx --from shellcheck-py shellcheck' works too)"
fi
((failures == before)) && pass "shell syntax$([[ -n "$shellcheck_cmd" ]] && echo ' and shellcheck')"

# --------------------------------------------------------------- git guard

if scripts/test-guard.sh >"${TMPDIR:-/tmp}/ai-gent-guard.out" 2>&1; then
  pass "git guard hook ($(grep -c ' ok ' "${TMPDIR:-/tmp}/ai-gent-guard.out") cases)"
else
  cat "${TMPDIR:-/tmp}/ai-gent-guard.out"
  fail "git guard hook"
fi
rm -f "${TMPDIR:-/tmp}/ai-gent-guard.out"

# ---------------------------------------------------------- installer tests

if ((QUICK)); then
  warn "--quick: skipped installer tests"
elif scripts/test-install.sh; then
  pass "installer tests"
else
  fail "installer tests"
fi

echo
if ((failures)); then
  echo "$failures check(s) failed"
  exit 1
fi
echo "all checks passed"
