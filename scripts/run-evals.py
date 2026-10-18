#!/usr/bin/env python3
"""Runs the plugins' eval suites and records the scores.

  scripts/run-evals.py                     trigger cases of every plugin with evals
  scripts/run-evals.py db git              only those plugins
  scripts/run-evals.py --behavior          the behavior cases (tag "behavior": a
                                           scaffold script and Bash in the sandbox)
  scripts/run-evals.py --runs 3 --max-cost-usd 5

Each plugin runs through `claude plugin eval`, one run per case unless --runs
says otherwise. A table is printed and one row per plugin is appended to
docs/product/evals.md, which project:pitch reads. Evals cost real tokens
(about $0.06-0.13 per case-run), so this is run on demand or monthly, never
in CI. Exit 1 when any case scores under the threshold (default 1.0).
"""
import argparse
import datetime
import json
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
HISTORY = ROOT / "docs" / "product" / "evals.md"
HEADER = """# Eval history

One row per plugin per run of `scripts/run-evals.py`: the cases that passed,
the overall score, and what the run cost. Trigger cases check that a request
reaches the right skill; behavior cases check what the skill does once loaded.

| Date | Claude Code | Mode | Plugin | Cases | Score | Cost (USD) |
|---|---|---|---|---|---|---|
"""


def plugins_with_evals(names: list[str]) -> list[str]:
    found = sorted(p.parent.name for p in (ROOT / "plugins").glob("*/evals") if p.is_dir())
    if not names:
        return found
    unknown = [n for n in names if n not in found]
    if unknown:
        sys.exit(f"error: no evals for: {', '.join(unknown)} (have: {', '.join(found)})")
    return names


def run_one(plugin: str, behavior: bool, runs: int, extra: list[str]) -> dict | None:
    out = Path(tempfile.mkdtemp()) / "result.json"
    cmd = ["claude", "plugin", "eval", f"plugins/{plugin}", "--runs", str(runs), "--ablation", "none",
           "--trust-plugin", "--no-publish", "--json", str(out)]
    if behavior:
        cmd += ["--tag", "behavior", "--scaffold", "--allow-tools", "Bash"]
    cmd += extra
    proc = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True, stdin=subprocess.DEVNULL)
    if not out.is_file():
        print(f"  {plugin}: no result ({(proc.stderr or proc.stdout).strip().splitlines()[-1:] or 'no output'})")
        return None
    data = json.loads(out.read_text())
    data["_exit"] = proc.returncode
    return data


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("plugins", nargs="*", help="plugin names (default: every plugin with evals)")
    ap.add_argument("--behavior", action="store_true", help="run the behavior cases instead of the trigger cases")
    ap.add_argument("--runs", type=int, default=1, help="runs per case (default 1)")
    ap.add_argument("--max-cost-usd", help="abort a plugin's suite past this cost")
    ap.add_argument("--no-record", action="store_true", help="print the table, leave docs/product/evals.md alone")
    args = ap.parse_args()

    extra = ["--max-cost-usd", args.max_cost_usd] if args.max_cost_usd else []
    mode = "behavior" if args.behavior else "trigger"
    rows, failed, total_cost = [], False, 0.0
    version = subprocess.run(["claude", "--version"], capture_output=True, text=True).stdout.split()[0]
    today = datetime.date.today().isoformat()

    for plugin in plugins_with_evals(args.plugins):
        data = run_one(plugin, args.behavior, args.runs, extra)
        if data is None:
            if args.behavior:
                continue  # a plugin without behavior cases isn't a failure
            failed = True
            continue
        agg = data.get("aggregates", {})
        if not agg.get("casesTotal"):
            continue
        cost = float(data.get("costUsd") or 0)
        total_cost += cost
        failed = failed or data["_exit"] != 0
        rows.append((today, version, mode, plugin, f"{agg['casesPassed']}/{agg['casesTotal']}",
                     f"{agg['overallScore']:.2f}", f"{cost:.2f}"))

    if not rows:
        print("no cases ran")
        return 1 if failed else 0
    print(f"{'plugin':<14}{'cases':>8}{'score':>8}{'cost':>9}")
    for r in rows:
        print(f"{r[3]:<14}{r[4]:>8}{r[5]:>8}{r[6]:>9}")
    print(f"{'total':<14}{'':>8}{'':>8}{total_cost:>9.2f}")

    if not args.no_record:
        HISTORY.parent.mkdir(parents=True, exist_ok=True)
        if not HISTORY.exists():
            HISTORY.write_text(HEADER)
        with HISTORY.open("a") as f:
            for r in rows:
                f.write("| " + " | ".join(r) + " |\n")
        print(f"recorded in {HISTORY.relative_to(ROOT)}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
