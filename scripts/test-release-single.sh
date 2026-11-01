#!/usr/bin/env bash
# Tests shared/release-single.py (the single-version release template) in
# throwaway repositories: bump rules, version sources, CHANGELOG promotion
# and generation, ignored commits, --dry-run, --notes, --initial, exit codes.
# Called by scripts/check.sh.

set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RELEASE="$REPO_DIR/shared/release-single.py"
SANDBOX="$(mktemp -d)"
trap 'rm -rf "$SANDBOX"' EXIT

export GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1
export GIT_AUTHOR_NAME=test GIT_AUTHOR_EMAIL=test@example.test
export GIT_COMMITTER_NAME=test GIT_COMMITTER_EMAIL=test@example.test

failures=0
check() { # check <description> <command...>
  local what="$1"
  shift
  if "$@"; then
    printf '      ok    %s\n' "$what"
  else
    printf '      FAIL  %s\n' "$what"
    failures=$((failures + 1))
  fi
}

# repo <name> <version-file> <content>: a repository tagged 1.0.0.
repo() {
  local dir="$SANDBOX/$1"
  mkdir -p "$dir/$(dirname "$2")"
  git init -q -b main "$dir"
  printf '%s\n' "$3" >"$dir/$2"
  printf '# Changelog\n\n## 1.0.0 — 2026-01-01\n\n- first\n' >"$dir/CHANGELOG.md"
  git -C "$dir" add -A
  git -C "$dir" commit -q -m "feat: first"
  git -C "$dir" tag -a 1.0.0 -m 1.0.0
  echo "$dir"
}
run() { (cd "$dir" && python3 "$RELEASE" "$@"); }
rc() { local code=0; run "$@" >/dev/null 2>&1 || code=$?; echo "$code"; }
commit() { git -C "$dir" commit -q --allow-empty -m "$1"; }

dir="$(repo py pyproject.toml $'[project]\nname = "x"\nversion = "1.0.0"')"
commit "docs: words"
check "docs only: exit 3" test "$(rc --dry-run)" = 3
commit "fix(api): a bug"
check "fix: patch" test "$(run 2>/dev/null)" = 1.0.1
check "pyproject bumped" grep -q '^version = "1.0.1"' "$dir/pyproject.toml"
check "changelog generated" grep -q '^## 1.0.1 — ' "$dir/CHANGELOG.md"
check "fix line in changelog" grep -q -- '- \*\*api:\*\* a bug' "$dir/CHANGELOG.md"

dir="$(repo js package.json $'{\n  "name": "x",\n  "version": "1.0.0"\n}')"
commit "feat: new thing"
commit "fix: small"
check "feat wins: minor" test "$(run 2>/dev/null)" = 1.1.0
check "package.json bumped" grep -q '"version": "1.1.0"' "$dir/package.json"

dir="$(repo major VERSION 1.0.0)"
commit "refactor!: rework"
check "bang: major" test "$(run 2>/dev/null)" = 2.0.0
check "VERSION bumped" test "$(cat "$dir/VERSION")" = 2.0.0
dir="$(repo footer VERSION 1.0.0)"
git -C "$dir" commit -q --allow-empty -m "feat: x" -m "BREAKING CHANGE: gone"
check "BREAKING CHANGE footer: major" test "$(run 2>/dev/null)" = 2.0.0

dir="$(repo src src/app/__init__.py '__version__ = "1.0.0"')"
printf '{"version_file": "src/app/__init__.py"}\n' >"$dir/release.json"
commit "perf: faster"
check "release.json version_file" test "$(run 2>/dev/null)" = 1.0.1
check "__version__ bumped" grep -q '__version__ = "1.0.1"' "$dir/src/app/__init__.py"

dir="$(repo notes VERSION 1.0.0)"
printf '# Changelog\n\n## Unreleased\n\n- hand written\n\n## 1.0.0 — 2026-01-01\n\n- first\n' >"$dir/CHANGELOG.md"
commit "feat: y"
run --date 2026-02-02 >/dev/null 2>&1
check "Unreleased promoted" grep -q '^## 1.1.0 — 2026-02-02' "$dir/CHANGELOG.md"
check "hand-written notes kept" grep -q '^- hand written' "$dir/CHANGELOG.md"
check "--notes prints the section" test "$(run --notes 1.1.0)" = "- hand written"

dir="$(repo skip VERSION 1.0.0)"
commit "chore(release): 1.0.0"
git -C "$dir" checkout -q -b side
commit "fix: only on side"
git -C "$dir" checkout -q main
git -C "$dir" merge -q --no-ff side -m "fix: merge subject must not count twice"
check "merge subject ignored, merged fix counted: patch" test "$(run --dry-run 2>/dev/null | head -n1)" = "release 1.0.1 (patch, since 1.0.0)"

dir="$(repo dry VERSION 1.0.0)"
commit "feat: z"
run --dry-run >/dev/null 2>&1
check "--dry-run writes nothing" test -z "$(git -C "$dir" status --porcelain)"
echo 1.0.5 >"$dir/VERSION"
check "version differs from tag: exit 1" test "$(rc)" = 1

dir="$(repo json VERSION 1.0.0)"
commit "docs: words"
check "--json, nothing releasable: exit 3, empty stdout" bash -c 'out="$(cd "$1" && python3 "$2" --dry-run --json 2>/dev/null)"; [ -z "$out" ]' _ "$dir" "$RELEASE"
check "--json, nothing releasable: exit 3" test "$(rc --dry-run --json)" = 3
commit "feat: j"
check "--json without --dry-run: exit 2" test "$(rc --json)" = 2
check "--json without --dry-run: nothing written" test -z "$(git -C "$dir" status --porcelain)"
check "--dry-run --json: one object with the spec keys" python3 -c '
import json, sys
d = json.loads(sys.argv[1])
assert sorted(d) == ["bump", "changelog", "since", "targets", "version"], d
assert d["version"] == "1.1.0" and d["bump"] == "minor" and d["since"] == "1.0.0", d
assert d["targets"] == [{"kind": "source", "label": "VERSION", "from": "1.0.0", "to": "1.1.0", "bump": "minor"}], d
assert isinstance(d["changelog"], str) and d["changelog"], d
' "$(run --dry-run --json 2>/dev/null)"
check "--dry-run --json writes nothing" test -z "$(git -C "$dir" status --porcelain)"
check "human --dry-run output unchanged" test "$(run --dry-run 2>/dev/null | head -n2 | tail -n1)" = "VERSION: 1.0.0 -> 1.1.0"

dir="$SANDBOX/fresh"
mkdir -p "$dir"
git init -q -b main "$dir"
echo 0.0.0 >"$dir/VERSION"
git -C "$dir" add -A
git -C "$dir" commit -q -m "feat: start"
check "no tag: --initial used" test "$(run --initial 0.3.0 2>/dev/null)" = 0.3.0

((failures == 0))
