#!/usr/bin/env python3
"""Latency of the guards (spec 033, FR-012): the shell guards or the ai-gent binary.

    bench-guard.py [--impl sh|bin] [--runs 100] [--json] [--check] [--bin PATH]

Spawns each guard N times per case with the same stdin, discards the first 5
runs, and reports p50/p95/p99 in ms (process start included). `--check` exits
1 when a p95 exceeds the absolute budget: Linux and macOS 15 ms, Windows 60 ms
(FR-012; the baseline + 5 ms rule is read off two runs, `--impl sh` and `--impl bin`).
Stdlib only.
"""
import argparse
import json
import os
import platform
import statistics
import subprocess
import sys
import tempfile
import time

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
GUARDS = {
    "git": "plugins/git/hooks/guard.sh",
    "db": "plugins/db/hooks/guard.sh",
    "secrets": "plugins/secrets/hooks/guard.sh",
    "repo": "plugins/repo/hooks/guard.sh",
    "containers": "plugins/devcontainer/hooks/guard.sh",
}
HEREDOC = "cat > /tmp/notes.txt <<'EOF'\n" + "line of text that is not a command\n" * 200 + "EOF"
# (case name, guard, tool, command or tool_input)
CASES = [
    ("clean-ls", "git", "Bash", "ls -la"),
    ("git-push-ask", "git", "Bash", "git push origin feat/x"),
    ("git-commit-branch", "git", "Bash", 'git commit -m "fix: x"'),
    ("long-heredoc", "git", "Bash", HEREDOC),
    ("db-read-path", "db", "Read", {"file_path": "/home/me/project/readme.md"}),
]
BUDGET_MS = 60.0 if os.name == "nt" else 15.0


def find_bin(arg):
    exe = "ai-gent.exe" if os.name == "nt" else "ai-gent"
    for p in [arg, os.environ.get("AI_GENT_BIN")] + [os.path.join(REPO, "rust", "target", d, exe) for d in ("release", "debug")]:
        if p and os.path.isfile(p):
            return p
    sys.exit("bench-guard: no ai-gent binary (cargo build --release in rust/, --bin or AI_GENT_BIN)")


def pct(xs, p):
    xs = sorted(xs)
    return xs[min(len(xs) - 1, int(round(p / 100 * (len(xs) - 1))))]


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--impl", default="sh", choices=["sh", "bin"])
    ap.add_argument("--runs", type=int, default=100)
    ap.add_argument("--json", action="store_true")
    ap.add_argument("--check", action="store_true")
    ap.add_argument("--bin")
    a = ap.parse_args()
    sandbox = tempfile.mkdtemp(prefix="ai-gent-bench-")
    subprocess.run(["git", "init", "-q", "-b", "feat/x", sandbox], check=True)
    binpath = find_bin(a.bin) if a.impl == "bin" else None
    rows = []
    for name, guard, tool, payload in CASES:
        ti = {"command": payload} if isinstance(payload, str) else payload
        ev = json.dumps({"hook_event_name": "PreToolUse", "tool_name": tool, "cwd": sandbox, "tool_input": ti})
        argv = ["sh", os.path.join(REPO, GUARDS[guard])] if a.impl == "sh" else [binpath, "guard", "--guard", guard]
        ts = []
        for i in range(a.runs + 5):
            t0 = time.perf_counter_ns()
            subprocess.run(argv, input=ev, capture_output=True, text=True)
            dt = (time.perf_counter_ns() - t0) / 1e6
            if i >= 5:
                ts.append(dt)
        rows.append({"os": platform.system().lower(), "impl": a.impl, "case": name, "n": len(ts),
                     "p50_ms": round(pct(ts, 50), 2), "p95_ms": round(pct(ts, 95), 2), "p99_ms": round(pct(ts, 99), 2),
                     "mean_ms": round(statistics.mean(ts), 2)})
    if a.json:
        print(json.dumps(rows, indent=2))
    else:
        print("%-18s %-4s %6s %6s %6s %6s" % ("case", "impl", "p50", "p95", "p99", "mean"))
        for r in rows:
            print("%-18s %-4s %6.2f %6.2f %6.2f %6.2f" % (r["case"], r["impl"], r["p50_ms"], r["p95_ms"], r["p99_ms"], r["mean_ms"]))
    if a.check:
        bad = [r for r in rows if r["p95_ms"] > BUDGET_MS]
        for r in bad:
            print("bench-guard: %s p95 %.2f ms exceeds the %.0f ms budget" % (r["case"], r["p95_ms"], BUDGET_MS), file=sys.stderr)
        return 1 if bad else 0
    return 0


if __name__ == "__main__":
    sys.exit(main())
