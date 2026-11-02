#!/usr/bin/env bash
# End-to-end test of `ai-gent update` (spec 034): the real install.sh and the real binary built from
# rust/, file:// releases 9.9.0 and 9.9.1 from scripts/package.py, a throwaway HOME. Covers install,
# update, rollback, a bad installer checksum, and pinned silence. Called by scripts/check.sh (not
# with --quick). Needs cargo; skips with a notice when it is missing.

set -euo pipefail
REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
command -v cargo >/dev/null 2>&1 || { echo "      skip  cargo not on PATH"; exit 0; }
SANDBOX="$(mktemp -d)"
trap 'rm -rf "$SANDBOX"' EXIT

(cd "$REPO_DIR/rust" && cargo build --locked -q -p ai-gent --target-dir "${CARGO_TARGET_DIR:-$REPO_DIR/rust/target}")
BIN="${CARGO_TARGET_DIR:-$REPO_DIR/rust/target}/debug/ai-gent"

export HOME="$SANDBOX/home"
export CLAUDE_SKILLS_DIR="$HOME/.claude/skills" OPENCODE_SKILLS_DIR="$HOME/.config/opencode/skills" AGENTS_SKILLS_DIR="$HOME/.agents/skills"
export XDG_STATE_HOME="$SANDBOX/state" AI_GENT_BIN_DIR="$HOME/bin"
export AI_GENT_HOME="$SANDBOX/ai gent" AI_GENT_TARGET=x86_64-unknown-linux-musl
export AI_GENT_RELEASE_BASE="file://$SANDBOX/rel"
unset VERSION AI_GENT_MODE AI_GENT_BRANCH AI_GENT_NO_UPDATE_CHECK
mkdir -p "$HOME"

for v in 9.9.0 9.9.1; do
  mkdir -p "$SANDBOX/rel/$v"
  python3 "$REPO_DIR/scripts/package.py" --version "$v" --out "$SANDBOX/rel/$v" >/dev/null
  python3 "$REPO_DIR/scripts/package.py" --version "$v" --out "$SANDBOX/rel/$v" --target "$AI_GENT_TARGET" --binary "$BIN" >/dev/null
done
echo 9.9.0 >"$SANDBOX/rel/LATEST"

failures=0
check() { local what="$1"; shift; if "$@"; then printf '      ok    %s\n' "$what"; else printf '      FAIL  %s\n' "$what"; failures=$((failures + 1)); fi; }
cur() { readlink "$AI_GENT_HOME/current"; }
prev() { readlink "$AI_GENT_HOME/previous"; }
AI() { "$AI_GENT_HOME/current/bin/ai-gent" "$@"; }

sh "$REPO_DIR/install.sh" --skip-prompt --target claude >/dev/null 2>&1
check "installed 9.9.0 with the real binary" test "$(cur)" = 9.9.0
check "the installed binary is the real one" bash -c '"$1" --version | grep -q "^ai-gent "' _ "$AI_GENT_HOME/current/bin/ai-gent"

echo 9.9.1 >"$SANDBOX/rel/LATEST"
check "--check is silent without a cache" test -z "$(AI update --check)"
printf '%s 9.9.1\n' "$(date +%s)" >"$XDG_STATE_HOME/ai-gent/update-check"
check "--check prints one notice from the cache" test "$(AI update --check | grep -c 'update available: 9.9.0 -> 9.9.1')" = 1
check "update without --yes and without a terminal exits 2" bash -c '"$1" update </dev/null >/dev/null 2>&1; [ $? -eq 2 ]' _ "$AI_GENT_HOME/current/bin/ai-gent"

# --- hook wiring (spec 034 FR-012): plugins/git/hooks/hooks.json SessionStart, binary first ---
HOOK_CMD="$(python3 -c "import json,sys; print(json.load(open(sys.argv[1]))['hooks']['SessionStart'][0]['hooks'][0]['command'])" "$REPO_DIR/plugins/git/hooks/hooks.json")"
# the notice is capped at once a day, so every run starts from a fresh cache line
fresh() { printf '%s 9.9.1\n' "$(date +%s)" >"$XDG_STATE_HOME/ai-gent/update-check"; }
hook() { env -u AI_GENT_UPDATE_ROOT CLAUDE_PLUGIN_ROOT="$1" sh -c "$HOOK_CMD"; }
mkdir -p "$SANDBOX/skills"
ln -s "$AI_GENT_HOME/current/plugins/git" "$SANDBOX/skills/git"
fresh
check "package with the binary: the hook prints the notice" \
  test "$(hook "$AI_GENT_HOME/current/plugins/git" | grep -c 'update available: 9.9.0 -> 9.9.1')" = 1
check "package with the binary: the second session of the day is silent" \
  test -z "$(hook "$AI_GENT_HOME/current/plugins/git")"
fresh
check "package with the binary: works through the skills symlink" \
  test "$(hook "$SANDBOX/skills/git" | grep -c 'update available: 9.9.0 -> 9.9.1')" = 1
fresh
hook "$AI_GENT_HOME/current/plugins/git" >/dev/null && rc=0 || rc=$?
check "package: the hook exits 0" test "$rc" = 0
# the same package without the binary falls back to the script
cp -R "$AI_GENT_HOME/9.9.0" "$SANDBOX/nobin"
rm -f "$SANDBOX/nobin/bin/ai-gent"
fresh
OUT="$(hook "$SANDBOX/nobin/plugins/git")"
check "package without the binary: the script prints the notice" \
  test "$(printf '%s' "$OUT" | grep -c 'update available: 9.9.0')" = 1
check "a clone (no bin/): the hook exits 0, silent when opted out" \
  test -z "$(AI_GENT_NO_UPDATE_CHECK=1 hook "$REPO_DIR/plugins/git")"
check "a plugin outside any ai-gent tree: the hook is a silent no-op that exits 0" \
  bash -c 'out="$(env CLAUDE_PLUGIN_ROOT="$1/nowhere/plugins/git" sh -c "$2")"; [ $? -eq 0 ] && [ -z "$out" ]' _ "$SANDBOX" "$HOOK_CMD"

AI update --yes >/dev/null 2>&1
check "update flips current to 9.9.1 and previous to 9.9.0" test "$(cur)/$(prev)" = 9.9.1/9.9.0
check "the new version runs" bash -c '"$1" --version >/dev/null' _ "$AI_GENT_HOME/current/bin/ai-gent"
AI update --rollback --yes >/dev/null 2>&1
check "rollback restores 9.9.0" test "$(cur)/$(prev)" = 9.9.0/9.9.1

# bad checksum: the installer is changed after SHA256SUMS was written
echo 'exit 0' >>"$SANDBOX/rel/9.9.1/install.sh"
code=0; AI update --yes >/dev/null 2>&1 || code=$?
check "a tampered installer is refused with exit 4" test "$code" = 4
check "and nothing changed" test "$(cur)" = 9.9.0
cp "$REPO_DIR/install.sh" "$SANDBOX/rel/9.9.1/install.sh"

# pin: --to pins, --check goes silent, --to latest clears
AI update --to 9.9.1 --yes >/dev/null 2>&1
check "--to writes the pin" test "$(cat "$AI_GENT_HOME/pinned")" = 9.9.1
printf '%s 9.9.9\n' "$(date +%s)" >"$XDG_STATE_HOME/ai-gent/update-check"
check "a pinned install stays silent" test -z "$(AI update --check)"
AI update --to latest --yes >/dev/null || echo "update --to latest exit $?" >&2
check "--to latest clears the pin" test ! -e "$AI_GENT_HOME/pinned"

((failures == 0)) || { printf '%d check(s) failed\n' "$failures" >&2; exit 1; }
