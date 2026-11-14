#!/usr/bin/env bash
# Tests plugins/git/hooks/guard.sh against the shared fixtures
# (shared/guard-fixtures/git.json, spec 033) through scripts/guard-fixtures.py, which
# prints one `ok`/`FAIL` line per case. Called by scripts/check.sh.

set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
python3 "$REPO_DIR/scripts/guard-fixtures.py" run --guard git --impl sh

# The guard log the shell fallback writes (generated `guard words` block, shared/guard-words.sh):
# only a regular file, never through a symlink, never blocking on a FIFO; modes 0700/0600 (a looser
# one we own is tightened); cut to the newest rows from the file's tail, under flock, and a held
# lock skips the cut without losing a row.
log="$(mktemp -d "${TMPDIR:-/tmp}/ai-gent-glog.XXXXXX")"
trap 'rm -rf "$log"' EXIT
fails=0
check() { if "${@:2}"; then printf '      ok    log: %s\n' "$1"; else printf '      FAIL  log: %s\n' "$1"; fails=$((fails + 1)); fi; }
deny() { printf '{"session_id":"%s","tool_name":"Bash","tool_input":{"command":"git commit --no-verify -m x"},"cwd":"/tmp"}' "$1" |
  env -u CLAUDE_PLUGIN_OPTION_guardLog -u CLAUDE_PLUGIN_OPTION_GUARDLOG AI_GENT_GUARD_LOG_DIR="$log" timeout 10 sh "$REPO_DIR/plugins/git/hooks/guard.sh"; }
lines() { wc -l <"$1" | tr -d ' '; }
deny s1 >/dev/null
check "a row is written" test "$(lines "$log/s1.jsonl")" = 1
echo x >"$log/target"; ln -s "$log/target" "$log/s2.jsonl"
out="$(deny s2)"
check "a symlinked log is refused, the decision still printed" test "$(cat "$log/target")" = x -a -n "$out"
mkfifo "$log/s3.jsonl"
out="$(deny s3)"
check "a FIFO is refused without blocking, the decision still printed" test -p "$log/s3.jsonl" -a -n "$out"
: >"$log/s4.jsonl"; chmod 644 "$log/s4.jsonl"; chmod 755 "$log"
deny s4 >/dev/null
check "a looser mode is tightened" test "$(stat -c %a "$log" "$log/s4.jsonl" 2>/dev/null || stat -f %Lp "$log" "$log/s4.jsonl")" = "700
600"
for _ in $(seq 400); do echo '{"at":1}'; done >"$log/s5.jsonl"
deny s5 >/dev/null
check "cut to the newest 200, no temporary file left" test "$(lines "$log/s5.jsonl")" = 200 -a -z "$(find "$log" -name '.s5.jsonl.*')"
head -c 600000 /dev/zero | tr '\0' 'x' >"$log/s6.jsonl"; for _ in $(seq 10); do echo '{"at":2}'; done >>"$log/s6.jsonl"
deny s6 >/dev/null
# the first line read from the middle (the x run with the first row) is dropped as partial
check "a large file is cut from its tail" test "$(lines "$log/s6.jsonl")" = 10 -a "$(wc -c <"$log/s6.jsonl" | tr -d ' ')" -lt 1000
if command -v flock >/dev/null 2>&1; then
  for _ in $(seq 400); do echo '{"at":1}'; done >"$log/s7.jsonl"
  (
    flock 9
    deny s7 >/dev/null
  ) 9>>"$log/s7.jsonl.lock"
  check "a held lock skips the cut and loses no row" test "$(lines "$log/s7.jsonl")" = 401
fi
((fails == 0)) || exit 1

# the launcher head of a package install turns a missing, failing or slow binary into the ask line (spec 033 T051)
exec python3 "$REPO_DIR/scripts/guard-fixtures.py" failures --guard git
