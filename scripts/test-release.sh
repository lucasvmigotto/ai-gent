#!/usr/bin/env bash
# Tests scripts/release.py in throwaway repositories: which commits release
# and at which level, per-plugin bumps, CHANGELOG promotion or generation,
# ignored commits, --dry-run and --notes. Called by scripts/check.sh.

set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RELEASE="$REPO_DIR/scripts/release.py"
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

# A base repository tagged 1.0.0: plugins a and b, a sharing shared/pipeline.md.
base="$SANDBOX/base"
mkdir -p "$base"/plugins/{a,b}/.claude-plugin "$base/plugins/a/references" "$base/shared"
for p in a b; do
  printf '{\n  "name": "%s",\n  "version": "1.0.0",\n  "description": "plugin %s"\n}\n' "$p" "$p" \
    >"$base/plugins/$p/.claude-plugin/plugin.json"
  echo "# $p" >"$base/plugins/$p/README.md"
done
echo "# contract" >"$base/shared/pipeline.md"
ln -s ../../../shared/pipeline.md "$base/plugins/a/references/pipeline.md"
printf '# Changelog\n\nIntro.\n\n## 1.0.0 — 2026-01-01\n\nFirst.\n' >"$base/CHANGELOG.md"
git -C "$base" init -q -b main
git -C "$base" add -A
git -C "$base" commit -q -m "chore: initial"
git -C "$base" tag -a 1.0.0 -m 1.0.0

repo="" out=""
fresh() { repo="$SANDBOX/r$RANDOM$RANDOM"; git clone -q "$base" "$repo"; }
edit() { # edit <file> <commit subject> [<body>]
  echo "change $RANDOM" >>"$repo/$1"
  git -C "$repo" add -A
  if (($# > 2)); then git -C "$repo" commit -q -m "$2" -m "$3"; else git -C "$repo" commit -q -m "$2"; fi
}
run() { # run <args...>: sets out and rc
  set +e
  out="$(cd "$repo" && python3 "$RELEASE" --date 2026-02-02 "$@" 2>/dev/null)"
  rc=$?
  set -e
}
version_of() { python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["version"])' "$repo/plugins/$1/.claude-plugin/plugin.json"; }
# shellcheck disable=SC2329 # called through check()
clean() { [[ -z "$(git -C "$repo" status --porcelain)" ]]; }

echo "      -- no release"
fresh
edit plugins/a/README.md "docs(a): explain"
edit README.md "chore: tidy"
run
check "docs and chore only → exit 3" test "$rc" -eq 3
check "nothing written" clean
edit README.md "chore(release): 1.0.1"
run
check "chore(release) ignored" test "$rc" -eq 3
git -C "$repo" switch -q -c topic
edit README.md "docs: topic notes"
git -C "$repo" switch -q main
git -C "$repo" merge -q --no-ff -m "feat: merge subject that must not count" topic
run
check "merge subject ignored" test "$rc" -eq 3

echo "      -- levels"
fresh
edit plugins/a/README.md "fix(a): handle empty input"
run
check "fix → patch 1.0.1" test "$rc" -eq 0 -a "$out" == "1.0.1"
check "  touched plugin a bumped to 1.0.1" test "$(version_of a)" == "1.0.1"
check "  untouched plugin b stays 1.0.0" test "$(version_of b)" == "1.0.0"
check "  generated Fixed section" grep -q '^- \*\*a:\*\* handle empty input$' "$repo/CHANGELOG.md"
check "  new section above 1.0.0" bash -c 'grep -n "^## " "$1" | head -1 | grep -q "1.0.1 — 2026-02-02"' _ "$repo/CHANGELOG.md"
run --notes
check "  --notes prints the new section" bash -c '[[ "$1" == *"### Fixed"* && "$1" != *"First."* ]]' _ "$out"

fresh
edit plugins/b/README.md "feat(b): add export"
edit plugins/a/README.md "perf(a): cache lookups"
run
check "feat + perf → minor 1.1.0" test "$rc" -eq 0 -a "$out" == "1.1.0"
check "  plugin b minor, plugin a patch" test "$(version_of b)/$(version_of a)" == "1.1.0/1.0.1"
check "  Added and Changed sections" bash -c 'grep -q "^### Added" "$1" && grep -q "^### Changed" "$1"' _ "$repo/CHANGELOG.md"

fresh
edit plugins/a/README.md "refactor(a)!: rename the entry point"
run
check "type! → major 2.0.0" test "$rc" -eq 0 -a "$out" == "2.0.0"
check "  Breaking section" grep -q '^### Breaking' "$repo/CHANGELOG.md"

fresh
edit README.md "fix: parse dates" "BREAKING CHANGE: dates are now ISO 8601 only"
run
check "BREAKING CHANGE footer → major 2.0.0" test "$rc" -eq 0 -a "$out" == "2.0.0"
check "  footer text in Breaking" grep -q '^- dates are now ISO 8601 only$' "$repo/CHANGELOG.md"

echo "      -- plugins"
fresh
edit shared/pipeline.md "fix: correct the contract"
run
check "shared file bumps the plugin that links it (a)" test "$(version_of a)" == "1.0.1"
check "  and not the one that doesn't (b)" test "$(version_of b)" == "1.0.0"

fresh
sed -i 's/"version": "1.0.0"/"version": "1.3.0"/' "$repo/plugins/a/.claude-plugin/plugin.json"
edit plugins/a/README.md "feat(a): bump by hand"
run
check "hand-bumped plugin version left alone" test "$(version_of a)" == "1.3.0"

fresh
mkdir -p "$repo/plugins/c/.claude-plugin"
printf '{\n  "name": "c",\n  "version": "0.4.0"\n}\n' >"$repo/plugins/c/.claude-plugin/plugin.json"
git -C "$repo" add -A && git -C "$repo" commit -q -m "feat(c): add plugin c"
run
check "new plugin keeps its initial version" test "$(version_of c)" == "0.4.0"

echo "      -- changelog"
fresh
python3 - "$repo/CHANGELOG.md" <<'EOF'
import sys
p = sys.argv[1]
s = open(p).read().replace("## 1.0.0", "## Unreleased\n\nHand-written notes.\n\n## 1.0.0", 1)
open(p, "w").write(s)
EOF
git -C "$repo" commit -q -am "docs: notes for the next release"
edit README.md "fix: something"
run
check "## Unreleased renamed to the release" bash -c 'grep -q "^## 1.0.1 — 2026-02-02$" "$1" && ! grep -q "^## Unreleased" "$1"' _ "$repo/CHANGELOG.md"
check "  hand-written notes kept, nothing generated" bash -c 'grep -q "^Hand-written notes.$" "$1" && ! grep -q "^- something$" "$1"' _ "$repo/CHANGELOG.md"

echo "      -- dry run"
fresh
edit plugins/a/README.md "fix(a): x"
run --dry-run
check "prints the plan" bash -c '[[ "$1" == "release 1.0.1 (patch, since 1.0.0)"* && "$1" == *"plugin a: 1.0.0 -> 1.0.1 (patch)"* ]]' _ "$out"
check "writes nothing" clean

echo "      -- no tag yet"
repo="$SANDBOX/untagged"
git init -q -b main "$repo"
echo x >"$repo/file"
git -C "$repo" add -A && git -C "$repo" commit -q -m "feat: first feature"
run
check "first release defaults to 0.1.0" test "$rc" -eq 0 -a "$out" == "0.1.0"
check "  CHANGELOG created" test -f "$repo/CHANGELOG.md"

echo "      -- errors"
run --initial 1.0
check "bad --initial → exit 2" test "$rc" -eq 2
repo="$SANDBOX/not-a-repo"
mkdir -p "$repo"
run
check "outside a repository → exit 1" test "$rc" -eq 1

exit $((failures > 0))
