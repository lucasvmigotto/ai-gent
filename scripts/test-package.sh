#!/usr/bin/env bash
# Tests scripts/package.py and the package install path of install.sh in a
# throwaway HOME, with no network: a package built from the working tree is
# served over file:// (AI_GENT_RELEASE_BASE). Covers build determinism, a bad
# checksum or truncated tarball refusing (and extracting nothing), install ->
# upgrade -> rollback flipping `current`, --prune, a path with spaces, a package
# with a dangling symlink refusing to build, no dev-only files in the installed
# tree, --migrate, and --verify-attestation. Called by scripts/check.sh.

set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SANDBOX="$(mktemp -d)"
trap 'rm -rf "$SANDBOX"' EXIT

export HOME="$SANDBOX/home dir" # spaces on purpose
mkdir -p "$HOME"
export CLAUDE_SKILLS_DIR="$HOME/.claude/skills"
export XDG_STATE_HOME="$HOME/.local/state"
export OPENCODE_SKILLS_DIR="$HOME/.config/opencode/skills" AGENTS_SKILLS_DIR="$HOME/.agents/skills"
unset XDG_DATA_HOME AI_GENT_DIR AI_GENT_REPO AI_GENT_BRANCH AI_GENT_MODE VERSION AI_GENT_HOME
export AI_GENT_HOME="$HOME/share/ai gent"
REL="$SANDBOX/release"
export AI_GENT_RELEASE_BASE="file://$REL"

failures=0
check() {
  local what="$1"
  shift
  if "$@"; then printf '      ok    %s\n' "$what"; else printf '      FAIL  %s\n' "$what"; failures=$((failures + 1)); fi
}
inst() { sh "$REPO_DIR/install.sh" --skip-prompt --target claude "$@" >"$SANDBOX/out" 2>&1; }
cur() { readlink "$AI_GENT_HOME/current"; }
prev() { readlink "$AI_GENT_HOME/previous"; }

echo "      -- build"
mkdir -p "$SANDBOX/b1" "$SANDBOX/b2"
python3 "$REPO_DIR/scripts/package.py" --version 9.9.0 --out "$SANDBOX/b1" >/dev/null
python3 "$REPO_DIR/scripts/package.py" --version 9.9.0 --out "$SANDBOX/b2" >/dev/null
check "two builds are byte-identical" cmp -s "$SANDBOX/b1/ai-gent-9.9.0.tar.gz" "$SANDBOX/b2/ai-gent-9.9.0.tar.gz"
check "SHA256SUMS also lists install.sh" grep -q "  install.sh$" "$SANDBOX/b1/SHA256SUMS"
check "SHA256SUMS matches the tarball and install.sh" bash -c 'cd "$1" && (sha256sum -c SHA256SUMS >/dev/null 2>&1 || shasum -a 256 -c SHA256SUMS >/dev/null 2>&1)' _ "$SANDBOX/b1"
check "--check passes" bash -c 'python3 "$1/scripts/package.py" --check >/dev/null' _ "$REPO_DIR"
check "a bad version is refused" bash -c '! python3 "$1/scripts/package.py" --version v1 --out "$2" >/dev/null 2>&1' _ "$REPO_DIR" "$SANDBOX/b3"

# A tree with a dangling or escaping symlink must not build.
bad="$SANDBOX/bad"
mkdir -p "$bad/scripts" "$bad/opencode/guard" "$bad/plugins/p/references" "$bad/shared"
cp "$REPO_DIR/scripts/package.py" "$bad/scripts/"
touch "$bad/setup.sh" "$bad/install.sh" "$bad/LICENSE" "$bad/README.md" "$bad/scripts/update-check.sh" \
  "$bad/scripts/ai-gent-update.sh" "$bad/scripts/opencode-config.py" "$bad/opencode/guard/rules.mjs"
ln -s ../../../shared/missing.md "$bad/plugins/p/references/x.md"
check "a dangling symlink refuses to build" bash -c '! python3 "$1/scripts/package.py" --version 1.0.0 --out "$1/dist" >/dev/null 2>"$1/err" && grep -q dangles "$1/err"' _ "$bad"
rm "$bad/plugins/p/references/x.md"
ln -s ../../../../etc/passwd "$bad/plugins/p/references/x.md"
check "a symlink leaving the package refuses to build" bash -c '! python3 "$1/scripts/package.py" --version 1.0.0 --out "$1/dist" >/dev/null 2>&1' _ "$bad"
rm "$bad/plugins/p/references/x.md"
ln -s /etc/passwd "$bad/plugins/p/references/x.md"
check "an absolute symlink refuses to build" bash -c '! python3 "$1/scripts/package.py" --version 1.0.0 --out "$1/dist" >/dev/null 2>&1' _ "$bad"

# Release directory: 9.9.0, 9.9.1, 9.9.2 (the version is only in VERSION, so one tree serves all).
for v in 9.9.0 9.9.1 9.9.2; do
  mkdir -p "$REL/$v"
  python3 "$REPO_DIR/scripts/package.py" --version "$v" --out "$REL/$v" >/dev/null
done
printf '9.9.1\n' >"$REL/LATEST"

echo "      -- refusals write nothing"
mkdir -p "$SANDBOX/rel-bad/9.9.0"
cp "$REL/9.9.0/"* "$SANDBOX/rel-bad/9.9.0/"
printf '%064d  ai-gent-9.9.0.tar.gz\n' 0 >"$SANDBOX/rel-bad/9.9.0/SHA256SUMS"
check "a bad checksum refuses" bash -c '! AI_GENT_RELEASE_BASE="file://$1" VERSION=9.9.0 sh "$2/install.sh" --skip-prompt --target claude >"$3" 2>&1 && grep -q "checksum mismatch" "$3"' _ "$SANDBOX/rel-bad" "$REPO_DIR" "$SANDBOX/out"
check "...and extracts nothing" test ! -e "$AI_GENT_HOME" -a ! -e "$CLAUDE_SKILLS_DIR"
head -c 20000 "$REL/9.9.0/ai-gent-9.9.0.tar.gz" >"$SANDBOX/rel-bad/9.9.0/ai-gent-9.9.0.tar.gz"
check "a truncated tarball (old sums) refuses" bash -c '! AI_GENT_RELEASE_BASE="file://$1" VERSION=9.9.0 sh "$2/install.sh" --skip-prompt --target claude >/dev/null 2>&1' _ "$SANDBOX/rel-bad" "$REPO_DIR"
(cd "$SANDBOX/rel-bad/9.9.0" && { sha256sum ai-gent-9.9.0.tar.gz 2>/dev/null || shasum -a 256 ai-gent-9.9.0.tar.gz; } >SHA256SUMS)
check "a truncated tarball (matching sums) refuses" bash -c '! AI_GENT_RELEASE_BASE="file://$1" VERSION=9.9.0 sh "$2/install.sh" --skip-prompt --target claude >/dev/null 2>&1' _ "$SANDBOX/rel-bad" "$REPO_DIR"
check "...and extracts nothing" test ! -e "$AI_GENT_HOME" -a ! -e "$CLAUDE_SKILLS_DIR"
check "a non-version is refused" bash -c '! VERSION="../x" sh "$1/install.sh" >/dev/null 2>&1' _ "$REPO_DIR"
mkdir -p "$SANDBOX/clonehome/.git"
check "a git clone as the install root is refused" bash -c '! AI_GENT_HOME="$1" sh "$2/install.sh" >"$3" 2>&1 && grep -q "git clone" "$3"' _ "$SANDBOX/clonehome" "$REPO_DIR" "$SANDBOX/out"

echo "      -- install, upgrade, rollback, prune (home with spaces)"
VERSION=9.9.0 inst
check "installs into <home>/<version>" test -f "$AI_GENT_HOME/9.9.0/setup.sh"
check "current points at it" test "$(cur)" == 9.9.0
check "the plan names the source and target" grep -q "into  $AI_GENT_HOME/9.9.0" "$SANDBOX/out"
check "skills link through current" test "$(readlink "$CLAUDE_SKILLS_DIR/git")" == "$AI_GENT_HOME/current/plugins/git"
check "a linked skill resolves (shared symlinks survive)" test -f "$CLAUDE_SKILLS_DIR/db/references/secrets.md"
check "no stage or temp leftovers" bash -c '! compgen -G "$1/.stage.*" >/dev/null && ! compgen -G "$1/*.new.*" >/dev/null' _ "$AI_GENT_HOME"
VERSION=9.9.0 inst
check "re-install is idempotent" test "$(cur)" == 9.9.0 -a ! -e "$AI_GENT_HOME/previous"
check "an explicit VERSION pins" test "$(cat "$AI_GENT_HOME/pinned")" == 9.9.0
inst # latest -> 9.9.1 through LATEST
check "latest clears the pin" test ! -e "$AI_GENT_HOME/pinned"
VERSION=9.9.1 inst
check "the pin is recorded again" test "$(cat "$AI_GENT_HOME/pinned")" == 9.9.1
inst
check "latest resolves and upgrades" test "$(cur)" == 9.9.1
check "previous keeps the old version" test "$(prev)" == 9.9.0
check "the old version stays unpacked" test -f "$AI_GENT_HOME/9.9.0/setup.sh"
sh "$REPO_DIR/install.sh" --rollback >"$SANDBOX/out" 2>&1
check "--rollback flips current back" test "$(cur)" == 9.9.0 -a "$(prev)" == 9.9.1
check "after rollback the skills still resolve" test -f "$CLAUDE_SKILLS_DIR/git/.claude-plugin/plugin.json"
sh "$REPO_DIR/install.sh" --list >"$SANDBOX/out" 2>&1
check "--list shows current and previous" bash -c 'grep -q "^9.9.0 (current)$" "$1" && grep -q "^9.9.1 (previous)$" "$1"' _ "$SANDBOX/out"
VERSION=9.9.2 inst
check "a third version: previous follows" test "$(cur)" == 9.9.2 -a "$(prev)" == 9.9.0
sh "$REPO_DIR/install.sh" --prune >"$SANDBOX/out" 2>&1
check "--prune keeps current and previous only" bash -c 'test -d "$1/9.9.2" && test -d "$1/9.9.0" && test ! -e "$1/9.9.1"' _ "$AI_GENT_HOME"
check "--rollback again swaps back and forth" bash -c 'sh "$1/install.sh" --rollback >/dev/null 2>&1' _ "$REPO_DIR"

echo "      -- the installed tree"
v="$AI_GENT_HOME/current"
check "VERSION matches" test "$(cat "$v/VERSION")" == 9.9.0
check "MANIFEST verifies" bash -c 'cd "$1" && (sha256sum -c MANIFEST >/dev/null 2>&1 || shasum -a 256 -c MANIFEST >/dev/null 2>&1)' _ "$v"
check "no dev-only files" bash -c '
  cd "$1" && ! find . \( -name evals -o -name results -o -name tests -o -name "*.test.ts" -o -name "*.test.tsx" -o -name "test_*.py" -o -name __pycache__ \) | grep -q . &&
  test ! -e docs -a ! -e specs -a ! -e .github -a ! -e scripts/release.py -a ! -e scripts/consistency.py -a ! -e scripts/check.sh -a ! -e scripts/run-evals.py -a ! -e CHANGELOG.md' _ "$v"
check "every symlink resolves inside the tree" bash -c '
  cd "$1" && find . -type l | while read -r l; do test -f "$l" || exit 1; done' _ "$v"
check "the update script and hook target exist" test -f "$v/scripts/update-check.sh" -a -f "$v/scripts/ai-gent-update.sh" -a -f "$v/plugins/git/../../scripts/update-check.sh"
check "setup.sh runs from the package (--dry-run)" bash -c 'bash "$1/setup.sh" --dry-run --skip-prompt --target claude >/dev/null 2>&1' _ "$v"

echo "      -- migrate from a git clone"
old="$SANDBOX/old-clone"
mkdir -p "$old/plugins/git" "$old/skills"
touch "$old/setup.sh"
rm -rf "$CLAUDE_SKILLS_DIR"
mkdir -p "$CLAUDE_SKILLS_DIR"
ln -s "$old/plugins/git" "$CLAUDE_SKILLS_DIR/git"
VERSION=9.9.0 inst
check "without --migrate the old link is kept" test "$(readlink "$CLAUDE_SKILLS_DIR/git")" == "$old/plugins/git"
check "...and the note says how to migrate" grep -q -e "--migrate" "$SANDBOX/out"
VERSION=9.9.0 inst --dry-run --migrate
check "--migrate --dry-run changes nothing but says what it would" bash -c 'test "$(readlink "$1/git")" == "$2/plugins/git" && grep -q "will be repointed" "$3"' _ "$CLAUDE_SKILLS_DIR" "$old" "$SANDBOX/out"
VERSION=9.9.0 inst --migrate
check "--migrate repoints it to current" test "$(readlink "$CLAUDE_SKILLS_DIR/git")" == "$AI_GENT_HOME/current/plugins/git"
check "the old tree is untouched" test -f "$old/setup.sh"

echo "      -- attestation (a fake gh)"
fb="$SANDBOX/fakebin"
mkdir -p "$fb"
printf '#!/bin/sh\necho "$@" >>"%s/gh.log"\nexit "${FAKE_GH_RC:-0}"\n' "$SANDBOX" >"$fb/gh"
chmod +x "$fb/gh"
VERSION=9.9.1 PATH="$fb:$PATH" inst --verify-attestation
check "gh attestation verify runs against the repo" grep -q "attestation verify .*ai-gent-9.9.1.tar.gz -R lucasvmigotto/ai-gent" "$SANDBOX/gh.log"
rm -rf "$AI_GENT_HOME/9.9.2" "$AI_GENT_HOME/9.9.1"
check "a failed attestation refuses" bash -c '! FAKE_GH_RC=1 VERSION=9.9.2 PATH="$1:$PATH" sh "$2/install.sh" --skip-prompt --target claude --verify-attestation >/dev/null 2>&1 && test ! -e "$3/9.9.2"' _ "$fb" "$REPO_DIR" "$AI_GENT_HOME"

exit $((failures > 0))
