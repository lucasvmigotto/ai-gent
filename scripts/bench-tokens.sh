#!/bin/sh
# Times `budget --projected 400k --json` for token_report.py and the ai-gent binary (spec 035 SC-004).
#
#   scripts/bench-tokens.sh [--runs N] [--bin PATH]              the real ~/.claude* (only totals are printed)
#   scripts/bench-tokens.sh --synth SIZE [--runs N] [--bin PATH] a synthetic corpus of about SIZE (e.g. 200MB, 1GB)
#
# The synthetic corpus is made of generated assistant messages in a temporary HOME; no transcript content
# is read or written. Not part of CI. Prints the median and the slowest of N runs (default 7) per tool.
set -eu
runs=7
bin=rust/target/release/ai-gent
synth=
while [ $# -gt 0 ]; do
  case "$1" in
    --runs) runs=$2; shift 2 ;;
    --bin) bin=$2; shift 2 ;;
    --synth) synth=$2; shift 2 ;;
    *) echo "usage: $0 [--synth SIZE] [--runs N] [--bin PATH]" >&2; exit 2 ;;
  esac
done
root=$(cd "$(dirname "$0")/.." && pwd)
py="$root/plugins/agents/scripts/token_report.py"
case "$bin" in /*) ;; *) bin="$root/$bin" ;; esac
[ -x "$bin" ] || { echo "no binary at $bin (cargo build --release in rust/)" >&2; exit 1; }

work=$(mktemp -d "${TMPDIR:-/tmp}/bench-tokens.XXXXXX")
trap 'rm -rf "$work"' EXIT
if [ -n "$synth" ]; then
  HOME_DIR="$work/home"
  python3 - "$synth" "$HOME_DIR" <<'PY'
import json, os, sys, re
from datetime import datetime, timedelta, timezone
size = sys.argv[1].upper()
mult = {"KB": 10**3, "MB": 10**6, "GB": 10**9}
n = int(re.match(r"\d+", size).group()) * mult[re.search(r"[KMG]B", size).group()]
home = sys.argv[2]
now = datetime.now(timezone.utc)
files, written, i = 0, 0, 0
while written < n:
    d = os.path.join(home, ".claude", "projects", f"-synth-{i % 20}")
    os.makedirs(d, exist_ok=True)
    path = os.path.join(d, f"s{i}.jsonl")
    lines = []
    for j in range(2000):
        ts = (now - timedelta(minutes=(i * 2000 + j) % 9000)).strftime("%Y-%m-%dT%H:%M:%SZ")
        lines.append(json.dumps({"type": "assistant", "timestamp": ts, "message": {
            "id": f"m{i}-{j}", "model": "claude-sonnet-5-5", "content": [{"type": "text", "text": "x" * 400}],
            "usage": {"input_tokens": 10, "cache_creation_input_tokens": 500, "cache_read_input_tokens": 9000, "output_tokens": 300}}}))
    text = "\n".join(lines)
    with open(path, "w") as fh:
        fh.write(text)
    written += len(text)
    files += 1
    i += 1
print(f"synthetic corpus: {files} files, {written} bytes", file=sys.stderr)
PY
else
  HOME_DIR=${HOME}
  find "$HOME_DIR"/.claude*/projects -name '*.jsonl' -mtime -7 2>/dev/null | wc -l | sed 's/^/files in the last 7 days: /' >&2
fi
mkdir -p "$work/cwd"
cd "$work/cwd"

now_ns() { date +%s%N; }
bench() { # bench <label> <command...>
  label=$1; shift
  : >"$work/times"
  "$@" >/dev/null 2>&1 || true # warm the page cache
  i=0
  while [ "$i" -lt "$runs" ]; do
    s=$(now_ns); HOME="$HOME_DIR" "$@" >/dev/null 2>&1 || true; e=$(now_ns)
    echo $(( (e - s) / 1000000 )) >>"$work/times"
    i=$((i + 1))
  done
  sort -n "$work/times" | awk -v l="$label" '{a[NR]=$1} END {printf "%-8s median %d ms, slowest %d ms (%d runs)\n", l, a[int((NR+1)/2)], a[NR], NR}'
}
bench python3 python3 "$py" budget --projected 400k --json
bench rust "$bin" tokens budget --projected 400k --json
