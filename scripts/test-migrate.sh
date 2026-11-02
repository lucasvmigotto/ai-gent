#!/usr/bin/env bash
# Tests the migration paths to the release package (spec 039, data-model.md "Fixture
# homes"), in a throwaway HOME with no network: a candidate release built from this
# working tree is served over file:// (AI_GENT_RELEASE_BASE) with a stub binary.
#
#   env-git-mode  AI_GENT_MODE/BRANCH/REPO/DIR set: the installer stops with the removal message
#   clone         the install root is a git clone: refused, pointing to the guide; after moving it
#                 aside, install + `setup.sh --migrate` repoints the links; the clone is untouched
#   mixed         links into a clone and into a package: --migrate repoints only the clone's
#   pinned        VERSION=x.y.z pins; a later unpinned install clears it
#   pkg-2.26      a real 2.26.0 install (git archive of the tag): its updater against a release that
#                 has no content-only asset fails before writing anything; the new installer works
#
# Not covered yet (they need the 2.28 updater and `ai-gent doctor`): pkg-2.28 and --rollback.
# Called by scripts/check.sh (skipped by --quick).

set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SANDBOX="$(mktemp -d)"
trap 'rm -rf "$SANDBOX"' EXIT

export HOME="$SANDBOX/home"
mkdir -p "$HOME"
export CLAUDE_SKILLS_DIR="$HOME/.claude/skills"
export XDG_STATE_HOME="$HOME/.local/state"
export OPENCODE_SKILLS_DIR="$HOME/.config/opencode/skills" AGENTS_SKILLS_DIR="$HOME/.agents/skills"
export AI_GENT_BIN_DIR="$HOME/.local/bin"
unset XDG_DATA_HOME AI_GENT_DIR AI_GENT_REPO AI_GENT_BRANCH AI_GENT_MODE VERSION AI_GENT_HOME AI_GENT_TARGET
export AI_GENT_HOME="$HOME/.local/share/ai-gent"
export AI_GENT_TARGET=x86_64-unknown-linux-musl

failures=0
check() {
  local what="$1"
  shift
  if "$@"; then printf '      ok    %s\n' "$what"; else printf '      FAIL  %s\n' "$what"; failures=$((failures + 1)); fi
}
inst() { sh "$REPO_DIR/install.sh" --skip-prompt --target claude "$@" >"$SANDBOX/out" 2>&1; }
reset() { rm -rf "$AI_GENT_HOME" "$CLAUDE_SKILLS_DIR" "$HOME/.local/bin" "$XDG_STATE_HOME"; }
# snapshot <dir...> — every file's hash and every link's target, to compare before and after
snapshot() { for d in "$@"; do [ -e "$d" ] || continue; find "$d" -type f -exec sha256sum {} + 2>/dev/null; find "$d" -type l -printf '%p -> %l\n'; done | sort; }

STUB="$SANDBOX/stub"
mkdir -p "$STUB"
printf '#!/bin/sh\necho "ai-gent stub"\n' >"$STUB/ai-gent"
chmod 755 "$STUB/ai-gent"
# the candidate release: per-target archive only (no content-only asset, B9)
REL="$SANDBOX/release"
mkdir -p "$REL/9.9.0"
python3 "$REPO_DIR/scripts/package.py" --version 9.9.0 --out "$REL/9.9.0" --target x86_64-unknown-linux-musl --binary "$STUB/ai-gent" >/dev/null
printf '9.9.0\n' >"$REL/LATEST"
export AI_GENT_RELEASE_BASE="file://$REL"

echo "      -- env-git-mode"
reset
for var in AI_GENT_MODE AI_GENT_BRANCH AI_GENT_REPO AI_GENT_DIR; do
  rc=0
  env "$var=git" sh "$REPO_DIR/install.sh" --skip-prompt --target claude >"$SANDBOX/out" 2>&1 || rc=$?
  check "$var: stops with the removal message and the steps" bash -c 'test "$1" -ne 0 && grep -q "git mode was removed in 3.0.0" "$2" && grep -q "mv ~/.local/share/ai-gent" "$2" && grep -q "setup.sh --migrate" "$2"' _ "$rc" "$SANDBOX/out"
done
check "...nothing was installed, no fallthrough to the package" test ! -e "$AI_GENT_HOME" -a ! -e "$CLAUDE_SKILLS_DIR"

echo "      -- clone"
reset
clone="$SANDBOX/clone-backup"
mkdir -p "$AI_GENT_HOME"
git -C "$AI_GENT_HOME" init -q
mkdir -p "$AI_GENT_HOME/plugins/git" "$AI_GENT_HOME/skills" "$CLAUDE_SKILLS_DIR"
touch "$AI_GENT_HOME/setup.sh"
ln -s "$AI_GENT_HOME/plugins/git" "$CLAUDE_SKILLS_DIR/git"
rc=0
inst || rc=$?
check "the installer refuses and points to the migration guide" bash -c 'test "$1" -ne 0 && grep -q "is a git clone" "$2" && grep -q "Guide: https://github.com/lucasvmigotto/ai-gent/.*README.md" "$2"' _ "$rc" "$SANDBOX/out"
check "...and wrote nothing" test "$(find "$AI_GENT_HOME" -mindepth 1 -maxdepth 1 | wc -l)" -eq 4
mv "$AI_GENT_HOME" "$clone"
ln -sfn "$clone/plugins/git" "$CLAUDE_SKILLS_DIR/git"
before="$(snapshot "$clone")"
inst --migrate
check "after moving it aside, the install works with the package" test "$(readlink "$AI_GENT_HOME/current")" == 9.9.0
check "--migrate repoints the link into current" test "$(readlink "$CLAUDE_SKILLS_DIR/git")" == "$AI_GENT_HOME/current/plugins/git"
check "the old clone is untouched" test "$(snapshot "$clone")" == "$before"
check "the binary is linked into the bin dir" test "$(readlink "$HOME/.local/bin/ai-gent")" == "$AI_GENT_HOME/current/bin/ai-gent"

echo "      -- mixed"
reset
clone="$SANDBOX/clone-mixed"
mkdir -p "$clone/plugins/git" "$clone/plugins/db" "$clone/skills" "$CLAUDE_SKILLS_DIR"
touch "$clone/setup.sh"
inst # a package install first
ln -sfn "$clone/plugins/git" "$CLAUDE_SKILLS_DIR/git" # one skill still in the clone
dbbefore="$(readlink "$CLAUDE_SKILLS_DIR/db")"
inst --migrate
check "--migrate reports the clone link" grep -q -e "--migrate: 1 link(s)" "$SANDBOX/out"
check "...repoints it to current" test "$(readlink "$CLAUDE_SKILLS_DIR/git")" == "$AI_GENT_HOME/current/plugins/git"
check "...and leaves the package links as they were" test "$(readlink "$CLAUDE_SKILLS_DIR/db")" == "$dbbefore"
check "...and the clone is untouched" test -f "$clone/setup.sh"
AI_GENT_MODE=git inst --migrate || true
check "--migrate run through the installer with a removed variable set is refused" grep -q "git mode was removed" "$SANDBOX/out"
AI_GENT_BRANCH=x "$AI_GENT_HOME/current/setup.sh" --target claude --skip-prompt --migrate >"$SANDBOX/out" 2>&1
check "setup.sh --migrate reports a removed variable in the environment" grep -q "AI_GENT_BRANCH is set in this environment" "$SANDBOX/out"

echo "      -- pinned"
reset
VERSION=9.9.0 inst
check "an explicit VERSION writes the pin" test "$(cat "$AI_GENT_HOME/pinned")" == 9.9.0
inst
check "an install without VERSION clears it" test ! -e "$AI_GENT_HOME/pinned"

echo "      -- pkg-2.26"
reset
if ! git -C "$REPO_DIR" rev-parse --quiet --verify 'refs/tags/2.26.0^{commit}' >/dev/null; then
  printf '      skip  no 2.26.0 tag in this clone (git fetch --tags)\n'
else
  old="$SANDBOX/old"
  mkdir -p "$old/tree" "$old/rel/2.26.0"
  git -C "$REPO_DIR" archive 2.26.0 | tar -x -C "$old/tree"
  python3 "$old/tree/scripts/package.py" --version 2.26.0 --out "$old/rel/2.26.0" >/dev/null
  printf '2.26.0\n' >"$old/rel/LATEST"
  AI_GENT_RELEASE_BASE="file://$old/rel" VERSION=2.26.0 sh "$old/tree/install.sh" --skip-prompt --target claude >"$SANDBOX/out" 2>&1
  check "2.26.0 installs with its own installer" test "$(readlink "$AI_GENT_HOME/current")" == 2.26.0
  before="$(snapshot "$AI_GENT_HOME" "$CLAUDE_SKILLS_DIR" "$HOME/.local")"
  rc=0
  "$AI_GENT_HOME/current/scripts/ai-gent-update.sh" >"$SANDBOX/out" 2>&1 || rc=$?
  check "its updater against the candidate release fails" test "$rc" -ne 0
  check "...with a download or checksum error, naming the missing asset" grep -q "ai-gent-9.9.0.tar.gz" "$SANDBOX/out"
  check "...before writing anything (tree and links byte-identical)" test "$(snapshot "$AI_GENT_HOME" "$CLAUDE_SKILLS_DIR" "$HOME/.local")" == "$before"
  inst
  check "the candidate installer upgrades the home" test "$(readlink "$AI_GENT_HOME/current")" == 9.9.0 -a "$(readlink "$AI_GENT_HOME/previous")" == 2.26.0
  check "...and the links lead into current" test "$(readlink "$CLAUDE_SKILLS_DIR/git")" == "$AI_GENT_HOME/current/plugins/git"
fi

exit $((failures > 0))
