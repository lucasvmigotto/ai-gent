#!/usr/bin/env python3
"""Parity gate for `ai-gent spec` (spec 037 Group B, decision D10).

The cases in rust/fixtures/spec/*.json are the tests of plugins/repo/scripts/test_specsync.py turned into
data (a file tree, the commands to run, what specsync.py printed and wrote), plus a live-corpus case: this
repository's own specs/ at the commit it was blessed against. This script is the oracle runner;
`cargo test -p ai-gent --test parity_spec` is the Rust runner over the same files.

  scripts/test-parity-spec.py              run every case through specsync.py (the oracle)
  scripts/test-parity-spec.py --bin PATH   run every case through the ai-gent binary instead
  scripts/test-parity-spec.py --bless      regenerate the fixtures from the Python tool, then review the diff
  scripts/test-parity-spec.py --bless-corpus   also re-snapshot specs/ into the live-corpus case

--bless keeps the live-corpus case as it is (its tree is the pinned snapshot) unless --bless-corpus is given,
and refuses to run once specsync.py is gone (deleted at the 3.0.0 cutover; the fixtures then stay).
Output is normalised before comparing: the temp root becomes <ROOT> and `Last sync: YYYY-MM-DD` becomes
`Last sync: <DATE>`.
"""
import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PY_DIR = ROOT / "plugins" / "repo" / "scripts"
PY_TOOL = PY_DIR / "specsync.py"
FIXTURES = ROOT / "rust" / "fixtures" / "spec"
CORPUS = "live-corpus"

sys.path.insert(0, str(PY_DIR))


def scenario_texts():
    import test_specsync as t
    return t.README, t.SPEC, t.TASKS


class Scn:
    def __init__(self, readme_fe="In progress", readme_be="In progress", spec=None, tasks=None):
        readme, spec0, tasks0 = scenario_texts()
        self.files = {
            "work/specs/README.md": readme.format(fe=readme_fe, be=readme_be),
            "work/specs/012-export/spec.md": spec0 if spec is None else spec,
            "work/specs/013-legacy/.keep": "",
        }
        if tasks is not False:
            self.files["work/specs/012-export/tasks.md"] = tasks0 if tasks is None else tasks
        self.runs = []

    def put(self, rel, text):
        self.files[rel] = text
        return self

    def run(self, *argv):
        self.runs.append({"argv": list(argv), "stdin": None})
        return self


def tracker_for(tasks_text, extra=None):
    """The tracker.md specsync would write after a full sync of the stock scenario."""
    import specsync
    old = os.getcwd()
    with tempfile.TemporaryDirectory() as d:
        try:
            os.chdir(d)
            s = Scn(tasks=tasks_text)
            for rel, text in s.files.items():
                p = Path(d) / rel.removeprefix("work/")
                p.parent.mkdir(parents=True, exist_ok=True)
                p.write_text(text)
            plan = specsync.build_plan(specsync.feature_dir("012"))
            items = {i["key"]: {"ref": f"#{n}", "id": str(9000 + n), "state": i["state"], "hash": i["hash"],
                                "url": f"https://x/{n}"} for n, i in enumerate(plan["items"], 101)}
            if extra:
                items.update(extra)
            specsync.write_tracker(Path("specs/012-export"), "github", "o/r", items)
            return Path("specs/012-export/tracker.md").read_text()
        finally:
            os.chdir(old)


def cases():
    readme, spec0, tasks0 = scenario_texts()
    out = {}

    def add(name, derived, s):
        out[name] = (derived, s)

    s = Scn().run("plan", "012", "--json").run("plan", "012")
    add("plan-items-and-story-assignment", "SpecsyncTest::test_items_and_story_assignment", s)
    s = Scn().run("plan", "012", "--json")
    add("plan-states-in-progress", "SpecsyncTest::test_states", s)
    s = Scn("Implemented", "In progress").run("plan", "012", "--json")
    add("plan-states-least-advanced-side-wins", "SpecsyncTest::test_states", s)
    s = Scn("Verified", "Verified").run("plan", "012", "--json").run("plan", "012")
    add("plan-states-verified-closes-every-story", "SpecsyncTest::test_states", s)
    s = Scn("Implemented (pending a real host)", "Implemented").run("plan", "012", "--json")
    add("plan-status-with-notes-after-it", "SpecsyncTest::test_status_with_notes_after_it", s)
    s = Scn(tasks=tasks0.replace("- [ ] T003", "- [x] T003")).run("plan", "012", "--json")
    add("plan-all-tasks-checked-is-review", "SpecsyncTest::test_all_tasks_checked_is_review", s)

    plan_records = None  # filled below from the Python tool

    import specsync
    old = os.getcwd()
    with tempfile.TemporaryDirectory() as d:
        os.chdir(d)
        try:
            for rel, text in Scn().files.items():
                p = Path(d) / rel.removeprefix("work/")
                p.parent.mkdir(parents=True, exist_ok=True)
                p.write_text(text)
            plan_records = [{"key": i["key"], "ref": f"#{n}", "id": 9000 + n, "url": f"https://x/{n}",
                             "state": i["state"], "hash": i["hash"]}
                            for n, i in enumerate(specsync.build_plan(specsync.feature_dir("012"))["items"], 101)]
        finally:
            os.chdir(old)
    s = Scn().put("work/batch-out.json", json.dumps({"ok": True, "records": plan_records}))
    s.run("plan", "012", "--json").run("record", "012", "--platform", "github", "--where", "o/r", "--from", "batch-out.json")
    s.run("plan", "012")
    add("record-creates-the-tracker", "SpecsyncTest::test_tracker_round_trip_and_actions", s)
    s = Scn(tasks=tasks0.replace("- [ ] T003", "- [x] T003")).put("work/specs/012-export/tracker.md", tracker_for(None))
    s.run("plan", "012", "--json").run("plan", "012")
    add("plan-update-and-state-after-a-checked-task", "SpecsyncTest::test_tracker_round_trip_and_actions", s)
    s = Scn().put("work/specs/012-export/tracker.md", tracker_for(None))
    s.run("plan", "specs/012-export").run("plan", "./specs/012-export/")
    add("plan-in-sync-and-path-forms", "SpecsyncTest::test_tracker_round_trip_and_actions", s)
    s = Scn().put("work/specs/012-export/tracker.md",
                  "- Platform: github\n\n| Key | Item | Id | State | Hash | URL |\n|---|---|---|---|---|---|\n"
                  "| US9 | #150 | 1 | todo | x | u |\n")
    s.run("plan", "012").run("plan", "012", "--json")
    add("plan-orphans-reported", "SpecsyncTest::test_orphans_reported", s)
    s = Scn().put("work/specs/README.md", "| # | Feature | Frontend | Backend | Notes |\n|---|---|---|---|---|\n"
                  "| 013 | legacy | — | — | **Retired by BCR-0001** |\n")
    s.run("plan", "013", "--json").run("plan", "013").run("features")
    add("plan-retired-from-notes", "SpecsyncTest::test_retired_from_notes", s)
    s = Scn(spec="# 012 — Export\n\n## Stories\n\n1. **CSV download**: a manager downloads the month.\n"
                 "   - Accept: a CSV with the contract's columns.\n2. **Scheduled export** — every Monday.\n\n## Planned\n\n- later\n",
            tasks=False)
    s.run("plan", "012", "--json").run("plan", "012")
    add("plan-introspec-story-list", "SpecsyncTest::test_introspec_story_list", s)
    s = Scn().run("features").run("features", "--json")
    add("features-list", "specsync.cmd_features", s)
    s = Scn().run("plan", "999").run("plan", "01").run("record", "012", "--platform", "github", "--from", "nope.json")
    add("errors-exit-codes", "specsync.main (Failed -> 1)", s)
    s = Scn(spec="# Feature Specification: 012 - Export\n\n### User Story 1 - A (priority: p3)\n\nBody `x` & <b>.\n\n"
                 "### Not a story\nignored\n\n### User Story 2: Colon form  \n\n- item\n  more\n1. num\n",
            tasks="# T\n\n## Phase 1: User Story 1\n\n- [X] T001a [TUI] [P] Upper X and suffix\n- [ ] T002b9 not a task\n"
                  "- [ ]T003 not a task either\n\n### Phase 2\n\n- [ ] T004 [CLI] after phase\n\n## QA\n\n- [ ] T005 [US2] qa task\n")
    s.run("plan", "012", "--json")
    add("plan-parser-edge-cases", "SpecsyncTest (parser edge cases beyond the Python tests)", s)
    return out


def live_corpus():
    files = {"work/specs/README.md": (ROOT / "specs" / "README.md").read_text(encoding="utf-8")}
    for d in sorted((ROOT / "specs").glob("[0-9][0-9][0-9]-*")):
        if not d.is_dir():
            continue
        for name in ("spec.md", "tasks.md", "tracker.md"):
            p = d / name
            if p.exists():
                files[f"work/specs/{d.name}/{name}"] = p.read_text(encoding="utf-8")
        files.setdefault(f"work/specs/{d.name}/.keep", "")
    runs = [{"argv": ["features"], "stdin": None}, {"argv": ["features", "--json"], "stdin": None}]
    nums = sorted({k.split("/")[2][:3] for k in files if k.count("/") == 3})
    for n in nums:
        runs.append({"argv": ["plan", n], "stdin": None})
    for n in nums[::7]:
        runs.append({"argv": ["plan", n, "--json"], "stdin": None})
    return files, runs


# ----------------------------------------------------------------- materialise

def normalise(text, tmp):
    text = text.replace(str(tmp), "<ROOT>")
    return re.sub(r"Last sync: \d{4}-\d{2}-\d{2}", "Last sync: <DATE>", text)


def materialise(case, tmp):
    for rel, text in case["files"].items():
        p = tmp / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(text.replace("<ROOT>", str(tmp)), encoding="utf-8", newline="")
    (tmp / "work").mkdir(exist_ok=True)


def tree(tmp):
    return {str(p.relative_to(tmp)): p.read_bytes() for p in sorted(tmp.rglob("*")) if p.is_file()}


def execute(case, tool_cmd, prefix):
    tmp = Path(tempfile.mkdtemp(prefix="parity-spec-"))
    try:
        materialise(case, tmp)
        before = tree(tmp)
        env = {k: v for k, v in os.environ.items() if k not in ("NO_COLOR",)}
        env.update(HOME=str(tmp / "home"), NO_COLOR="1", TZ="UTC")
        runs = []
        for run in case["runs"]:
            argv = [a.replace("<ROOT>", str(tmp)) for a in run["argv"]]
            proc = subprocess.run([*tool_cmd, *prefix, *argv], cwd=tmp / "work", env=env, input=run.get("stdin"),
                                  capture_output=True, text=True, timeout=120)
            runs.append({"argv": run["argv"], "stdin": run.get("stdin"), "exit": proc.returncode,
                         "stdout": normalise(proc.stdout, tmp)})
        files = {}
        for rel, data in tree(tmp).items():
            if before.get(rel) != data:
                files[rel] = normalise(data.decode("utf-8"), tmp)
        return runs, files
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


def compare(name, case, runs, files):
    problems = []
    for i, (want, got) in enumerate(zip(case["runs"], runs)):
        label = " ".join(want["argv"][:3])
        if want.get("exit") != got["exit"]:
            problems.append(f"{name} run {i} ({label}): exit {got['exit']}, expected {want.get('exit')}")
        if want.get("stdout") != got["stdout"]:
            problems.append(f"{name} run {i} ({label}): stdout differs\n--- expected\n{want.get('stdout')}\n--- got\n{got['stdout']}")
    for rel in sorted(set(case["expect_files"]) | set(files)):
        if case["expect_files"].get(rel) != files.get(rel):
            problems.append(f"{name}: file {rel} differs\n--- expected\n{case['expect_files'].get(rel)}\n--- got\n{files.get(rel)}")
    return problems


def write_case(name, derived, files, runs, tool, prefix):
    case = {"name": name, "derived_from": derived, "files": files, "runs": runs}
    got, changed = execute(case, tool, prefix)
    case["runs"] = [{"argv": r["argv"], "stdin": r["stdin"], "exit": r["exit"], "stdout": r["stdout"]} for r in got]
    case["expect_files"] = changed
    (FIXTURES / f"{name}.json").write_text(json.dumps(case, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--bin")
    ap.add_argument("--bless", action="store_true")
    ap.add_argument("--bless-corpus", action="store_true")
    ap.add_argument("--only")
    a = ap.parse_args()
    if a.bin:
        tool, prefix = [str(Path(a.bin).resolve())], ["spec"]
    else:
        tool, prefix = [sys.executable, str(PY_TOOL)], []
    if a.bless or a.bless_corpus:
        if not PY_TOOL.exists():
            print("refusing to bless: plugins/repo/scripts/specsync.py is gone (the fixtures are the oracle now)")
            return 1
        FIXTURES.mkdir(parents=True, exist_ok=True)
        if a.bless:
            for old in FIXTURES.glob("*.json"):
                if old.stem != CORPUS:
                    old.unlink()
            for name, (derived, scn) in cases().items():
                write_case(name, derived, scn.files, scn.runs, tool, prefix)
        if a.bless_corpus or not (FIXTURES / f"{CORPUS}.json").exists():
            files, runs = live_corpus()
            write_case(CORPUS, f"live corpus: this repository's specs/ at {subprocess.run(['git', 'rev-parse', '--short', 'HEAD'], cwd=ROOT, capture_output=True, text=True).stdout.strip()}",
                       files, runs, tool, prefix)
        print(f"blessed {len(list(FIXTURES.glob('*.json')))} cases into {FIXTURES.relative_to(ROOT)}")
        return 0
    problems, n = [], 0
    for path in sorted(FIXTURES.glob("*.json")):
        if a.only and a.only not in path.stem:
            continue
        case = json.loads(path.read_text(encoding="utf-8"))
        runs, files = execute(case, tool, prefix)
        problems += compare(case["name"], case, runs, files)
        n += 1
    for p in problems:
        print("FAIL ", p)
    who = "binary" if a.bin else "python oracle"
    print(f"{'FAIL' if problems else 'ok'}    {n} parity cases ({who}), {len(problems)} difference(s)")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
