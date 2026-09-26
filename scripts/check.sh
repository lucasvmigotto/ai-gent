#!/usr/bin/env bash
# Repository checks: skill and plugin metadata, cross-references, symlinks,
# shell scripts, the git and db guard hooks, dbrun, the release script, and installer tests
# in a throwaway HOME. Run before every commit; CI runs the same script.
#
#   scripts/check.sh                 everything
#   scripts/check.sh --quick         skip the installer tests
#
# Environment:
#   DESC_BUDGET  soft limit for skill descriptions, in characters (default 400);
#                a conservative ceiling of 1024 is also enforced (OpenCode V2
#                no longer caps description length, but every session loads it)
#   SHELLCHECK   the linter command (default: the pinned SHELLCHECK_VERSION run
#                through uvx or pipx, so local runs and CI use the same release;
#                then the one on PATH; else the lint is skipped)
#   SHELLCHECK_VERSION  shellcheck-py release to pin (default below)

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
    fail "$file: description is ${#desc} chars; ceiling is 1024"
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
python3 -c 'compile(open("scripts/opencode-config.py").read(), "opencode-config.py", "exec")' ||
  fail "scripts/opencode-config.py: syntax"

shellcheck_version="${SHELLCHECK_VERSION:-0.11.0.1}"
shellcheck_cmd="${SHELLCHECK:-}"
if [[ -z "$shellcheck_cmd" ]]; then
  if command -v uvx >/dev/null 2>&1; then
    shellcheck_cmd="uvx --quiet --from shellcheck-py==$shellcheck_version shellcheck"
  elif command -v pipx >/dev/null 2>&1; then
    shellcheck_cmd="pipx run --quiet --spec shellcheck-py==$shellcheck_version shellcheck"
  elif command -v shellcheck >/dev/null 2>&1; then
    shellcheck_cmd=shellcheck
    warn "using shellcheck $(shellcheck --version | sed -n 's/^version: //p') from PATH; CI pins $shellcheck_version (install uv or pipx to match)"
  fi
fi
if [[ -n "$shellcheck_cmd" ]]; then
  # shellcheck disable=SC2086 # SHELLCHECK may be a command with arguments
  $shellcheck_cmd "${scripts[@]}" install.sh "${hooks[@]}" ||
    fail "shellcheck"
else
  warn "shellcheck not found (nor uvx or pipx to fetch it); skipped"
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

# ------------------------------------------------------------------- db

if scripts/test-db-guard.sh >"${TMPDIR:-/tmp}/ai-gent-dbguard.out" 2>&1; then
  pass "db guard hook ($(grep -c ' ok ' "${TMPDIR:-/tmp}/ai-gent-dbguard.out") cases)"
else
  cat "${TMPDIR:-/tmp}/ai-gent-dbguard.out"
  fail "db guard hook"
fi
rm -f "${TMPDIR:-/tmp}/ai-gent-dbguard.out"

if python3 -m unittest plugins/db/scripts/test_dbrun.py >"${TMPDIR:-/tmp}/ai-gent-dbrun.out" 2>&1; then
  pass "dbrun ($(grep -oE 'Ran [0-9]+ tests' "${TMPDIR:-/tmp}/ai-gent-dbrun.out"))"
else
  cat "${TMPDIR:-/tmp}/ai-gent-dbrun.out"
  fail "dbrun unit tests"
fi
rm -f "${TMPDIR:-/tmp}/ai-gent-dbrun.out"

# ----------------------------------------------------------------- release

if scripts/test-release.sh >"${TMPDIR:-/tmp}/ai-gent-release.out" 2>&1; then
  pass "release script ($(grep -c ' ok ' "${TMPDIR:-/tmp}/ai-gent-release.out") cases)"
else
  cat "${TMPDIR:-/tmp}/ai-gent-release.out"
  fail "release script"
fi
rm -f "${TMPDIR:-/tmp}/ai-gent-release.out"

# ------------------------------------------------------- opencode guard

runtime="$(command -v bun || command -v node || true)"
if [[ -n "$runtime" ]]; then
  if "$runtime" opencode/guard/test/rules.test.mjs >"${TMPDIR:-/tmp}/ai-gent-ocguard.out" 2>&1; then
    pass "opencode guard rules ($(grep -oE '[0-9]+ checks' "${TMPDIR:-/tmp}/ai-gent-ocguard.out" | tail -1))"
  else
    cat "${TMPDIR:-/tmp}/ai-gent-ocguard.out"
    fail "opencode guard rules (vs plugins/*/hooks/guard.sh)"
  fi
  rm -f "${TMPDIR:-/tmp}/ai-gent-ocguard.out"
else
  warn "neither bun nor node on PATH; skipped the opencode guard rule test"
fi

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
