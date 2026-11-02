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
#   pkg-2.28      a 2.28.x package install with the real binary (this tree, packaged as 2.28.0): both
#                 `ai-gent update --yes` and scripts/ai-gent-update.sh upgrade it to the 3.0.0
#                 candidate (the target release's installer, fetched and checked against SHA256SUMS),
#                 `previous` is 2.28.0, the links lead into `current`, `ai-gent doctor` is green
#   rollback      the same upgrade, then `ai-gent update --rollback --yes` and `sh install.sh --rollback`
#                 restore 2.28.0 with the links and `ai-gent doctor` green (G30-7); `--rollback` runs
#                 only this case
#
# pkg-2.28 and rollback need cargo (the real binary) and skip with a notice without it.
# Called by scripts/check.sh (skipped by --quick).

set -euo pipefail

ONLY=""
[ "${1:-}" = --rollback ] && ONLY=rollback

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

if [ -z "$ONLY" ]; then
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
fi # ONLY

# --- pkg-2.28 and rollback: the real binary, the current tree as 2.28.0, the 3.0.0 candidate -------
if ! command -v cargo >/dev/null 2>&1; then
  printf '      skip  pkg-2.28, rollback: cargo not on PATH\n'
else
  (cd "$REPO_DIR/rust" && cargo build --locked -q -p ai-gent --target-dir "${CARGO_TARGET_DIR:-$REPO_DIR/rust/target}")
  BIN="${CARGO_TARGET_DIR:-$REPO_DIR/rust/target}/debug/ai-gent"
  R28="$SANDBOX/rel28"
  mkdir -p "$R28"
  for v in 2.28.0 3.0.0; do
    mkdir -p "$R28/$v"
    python3 "$REPO_DIR/scripts/package.py" --version "$v" --out "$R28/$v" >/dev/null
    python3 "$REPO_DIR/scripts/package.py" --version "$v" --out "$R28/$v" --target "$AI_GENT_TARGET" --binary "$BIN" >/dev/null
  done
  cur() { readlink "$AI_GENT_HOME/current"; }
  prev() { readlink "$AI_GENT_HOME/previous"; }
  AI() { "$AI_GENT_HOME/current/bin/ai-gent" "$@"; }
  # shellcheck disable=SC2329 # run through check
  doctor_ok() { AI doctor >"$SANDBOX/doctor" 2>&1 || { cat "$SANDBOX/doctor"; return 1; }; }
  # shellcheck disable=SC2329 # run through check
  links_in_current() { test "$(readlink "$CLAUDE_SKILLS_DIR/git")" == "$AI_GENT_HOME/current/plugins/git" -a "$(readlink "$HOME/.local/bin/ai-gent")" == "$AI_GENT_HOME/current/bin/ai-gent"; }
  # a 2.28.0 package install, with 3.0.0 as the newest release
  install28() {
    reset
    printf '3.0.0\n' >"$R28/LATEST"
    AI_GENT_RELEASE_BASE="file://$R28" VERSION=2.28.0 sh "$REPO_DIR/install.sh" --skip-prompt --target claude >"$SANDBOX/out" 2>&1
    rm -f "$AI_GENT_HOME/pinned"
  }
  export AI_GENT_RELEASE_BASE="file://$R28" AI_GENT_NO_UPDATE_CHECK=1

  if [ -z "$ONLY" ]; then
    echo "      -- pkg-2.28"
    install28
    check "2.28.0 installs with the real binary" test "$(cur)" == 2.28.0
    check "...doctor green before the upgrade" doctor_ok
    AI update --check >"$SANDBOX/out" 2>&1 || true
    AI update --yes >"$SANDBOX/out" 2>&1
    check "ai-gent update: current is 3.0.0 and previous is 2.28.0" test "$(cur)/$(prev)" == 3.0.0/2.28.0
    check "...the agent links and the binary link lead into current" links_in_current
    check "...ai-gent doctor is green on 3.0.0" doctor_ok
    # tampered installer in the target release: refused, nothing changed (G28-7)
    install28
    echo 'exit 0' >>"$R28/3.0.0/install.sh"
    rc=0
    AI update --yes >"$SANDBOX/out" 2>&1 || rc=$?
    check "a tampered 3.0.0 installer is refused with exit 4 and nothing changed" test "$rc" -eq 4 -a "$(cur)" == 2.28.0
    python3 "$REPO_DIR/scripts/package.py" --version 3.0.0 --out "$R28/3.0.0" >/dev/null
    python3 "$REPO_DIR/scripts/package.py" --version 3.0.0 --out "$R28/3.0.0" --target "$AI_GENT_TARGET" --binary "$BIN" >/dev/null
    # the script a 2.28 install ships
    install28
    "$AI_GENT_HOME/current/scripts/ai-gent-update.sh" >"$SANDBOX/out" 2>&1
    check "scripts/ai-gent-update.sh: current is 3.0.0 and previous is 2.28.0" test "$(cur)/$(prev)" == 3.0.0/2.28.0
    check "...the links" links_in_current
    check "...doctor" doctor_ok
  fi

  echo "      -- rollback"
  install28
  AI update --yes >"$SANDBOX/out" 2>&1
  check "upgraded to 3.0.0 first" test "$(cur)/$(prev)" == 3.0.0/2.28.0
  AI update --rollback --yes >"$SANDBOX/out" 2>&1
  check "ai-gent update --rollback: current is 2.28.0 again" test "$(cur)" == 2.28.0
  check "...the links lead into current" links_in_current
  check "...ai-gent doctor is green" doctor_ok
  check "...the binary reports 2.28.0 via VERSION" test "$(cat "$AI_GENT_HOME/current/VERSION")" == 2.28.0
  # the installer's own rollback, from the same state
  install28
  AI update --yes >"$SANDBOX/out" 2>&1
  sh "$AI_GENT_HOME/current/install.sh" --rollback >"$SANDBOX/out" 2>&1
  check "sh install.sh --rollback: current is 2.28.0" test "$(cur)" == 2.28.0
  check "...the links" links_in_current
  check "...doctor" doctor_ok
fi

exit $((failures > 0))
