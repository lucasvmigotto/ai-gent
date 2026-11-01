#!/usr/bin/env python3
"""Parity gate for `ai-gent wt` (spec 036, decision D10).

The cases in rust/fixtures/wt/*.json are the 16 tests of plugins/agents/scripts/test_wt.py turned into
data (plus a few extra branches): a throwaway repository, a fake container engine, and a list of steps
(git, file writes, engine state, `wt` runs). Each `wt` step records the exit code, stdout and stderr
`wt.py` produced; `state` steps record the fake engine's containers, volumes, images and call log.
This script is the oracle runner; `cargo test -p ai-gent --test parity_wt` is the Rust runner over the
same files.

  scripts/test-parity-wt.py              run every case through wt.py (the oracle) and compare
  scripts/test-parity-wt.py --bless      regenerate the recorded results from wt.py, then review the diff
  scripts/test-parity-wt.py --self-test  the harness itself, with one trivial case

--bless refuses to run once plugins/agents/scripts/wt.py is gone (it is deleted at the 3.0.0 cutover;
the fixtures then stay and only the Rust runner remains). The temp root is written as `<ROOT>`.
"""
import argparse
import contextlib
import io
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SCRIPTS = ROOT / "plugins" / "agents" / "scripts"
FIXTURES = ROOT / "rust" / "fixtures" / "wt"
sys.path.insert(0, str(SCRIPTS))

README = """# Features

| # | Feature | Priority | Depends on | Frontend | Backend |
|---|---|---|---|---|---|
| 001 | auth | P0 | — | Implemented | Verified |
| 002 | orgs | P0 | 001 | Planned | In progress |
| 003 | billing | P1 | 002 | Planned | Planned |
| 004 | search | P2 | 001 | N/A | Planned |
| 005 | reports | P2 | — | Verified | Verified |
"""
README_004_DONE = README.replace("| 004 | search | P2 | 001 | N/A | Planned |", "| 004 | search | P2 | 001 | N/A | Implemented |")
README_DEPENDS = """| # | Feature | Depends | QA | Status |
|---|---|---|---|---|
| 001 | one | — | Planned | **Implemented** |
| 002 | two | 001, 009 | Partial | Retired |
| 3 | bad | — | Planned | Planned |
"""
P, W = "me.lucasvmigotto.ai-gent.project", "me.lucasvmigotto.ai-gent.worktree"
CP = "com.docker.compose.project"


def wt(*argv, cwd="."):
    return {"wt": list(argv), "cwd": cwd}


def git(*argv, cwd="."):
    return {"git": list(argv), "cwd": cwd}


def commit_in(cwd, name, text="x = 1\n"):
    return [{"write": {f"{cwd}/{name}": text}}, git("add", ".", cwd=cwd), git("commit", "-q", "-m", f"feat: {name}", cwd=cwd)]


STATE_STEP = {"state": True}
SCOPED = {"assert_scoped": True}

CASES = []


def case(name, derived_from, steps, **kw):
    CASES.append({"name": name, "derived_from": derived_from, **kw, "steps": steps})


case("plan-json", "test_plan", [wt("plan", "--json"), wt("plan")])
case("plan-header-variants", "readme parser (depends header, bad rows, retired)",
     [wt("plan", "--json")], readme=README_DEPENDS)
case("plan-no-readme", "Failed: no specs/README.md", [{"rm": "specs/README.md"}, wt("plan")])
case("plan-reads-the-worktree-it-runs-in", "test_plan_reads_the_worktree_it_runs_in", [
    wt("add", "feat/specs"),
    {"write": {".worktrees/feat-specs/specs/README.md": README_004_DONE}},
    wt("plan", "--json", cwd=".worktrees/feat-specs"),
])
case("add-list-remove", "test_add_list_remove", [
    wt("add", "feat/002-orgs"),
    {"gitout": ["status", "--porcelain"]},
    {"cat": ".git/info/exclude", "tail": 3},
    {"cat": ".worktrees/.ports.json"},
    wt("add", "feat/004-search"),
    {"write": {".worktrees/feat-002-orgs/orgs.py": "x = 1\n"}},
    wt("list"),
    wt("remove", "feat/002-orgs"),
    git("add", ".", cwd=".worktrees/feat-002-orgs"),
    git("commit", "-q", "-m", "feat: orgs", cwd=".worktrees/feat-002-orgs"),
    wt("list", "--json"),
    wt("remove", "feat/002-orgs"),
    git("switch", "-q", "dev"),
    git("merge", "-q", "--ff-only", "feat/002-orgs"),
    wt("remove", "feat/002-orgs"),
    {"exists": ".worktrees/feat-002-orgs"},
    {"gitout": ["branch"]},
    {"cat": ".worktrees/.ports.json"},
    wt("remove", "feat/nope"),
])
case("add-refusals", "test_refusals", [
    git("branch", "develop"),
    wt("add", "feat/x"),
    wt("add", "feat/x", "--base", "dev"),
    wt("add", "feat/x", "--base", "dev"),
    wt("add", "Feat/X.", "--base", "dev"),
])
case("add-no-dev", "chain_dev missing", [git("branch", "-D", "dev"), wt("add", "feat/x"), wt("list")])
case("add-offsets-never-reuse-a-live-one", "test_offsets_never_reuse_a_live_one", [
    wt("add", "feat/a"), wt("add", "feat/b"), git("switch", "-q", "dev"), wt("remove", "feat/a"), wt("add", "feat/c"),
    {"cat": ".worktrees/.ports.json"},
])
case("remove-after-a-rebase-merge", "test_remove_after_a_rebase_merge", [
    wt("add", "feat/x"), *commit_in(".worktrees/feat-x", "x.py"),
    git("switch", "-q", "dev"), git("cherry-pick", "feat/x"),
    wt("remove", "feat/x"), {"gitout": ["branch"]},
])
case("list-from-inside-a-worktree", "test_from_inside_a_worktree", [
    wt("add", "feat/002-orgs"), wt("list", "--json", cwd=".worktrees/feat-002-orgs"),
    wt("add", "feat/other", cwd=".worktrees/feat-002-orgs"),
])
case("list-detached-and-empty", "edge: detached HEAD, no worktrees", [
    wt("list"), git("worktree", "add", "-q", "--detach", ".worktrees/det", "dev"), wt("list"), wt("list", "--json"),
    wt("remove", "(detached)"),
])
case("list-keys", "test_list_json_keeps_the_keys_the_worktree_board_reads", [
    wt("add", "feat/a"), {"write": {".worktrees/feat-a/a.py": "a = 1\n"}}, wt("list", "--json"),
    git("branch", "-D", "dev"), wt("list", "--json"), wt("list"),
])
case("add-disk-labels", "test_add_prints_labels_and_checks_disk", [
    wt("add", "feat/a"), {"usage": [500, 65]}, wt("add", "feat/b"), {"exists": ".worktrees/feat-b"},
    wt("add", "feat/b", "--min-free-gb", "0"), SCOPED, STATE_STEP,
])
case("add-disk-settings", "test_add_reads_settings_and_caps_volumes", [
    {"write": {".ai-gent/parallel.yaml": "max_parallel: 3\nmax_volumes: 1\nvolume_estimate_gb: 1 # small\n"}},
    {"usage": [100, 22]}, wt("add", "feat/a"), wt("add", "feat/b"),
])
case("add-disk-floor-setting", "disk_floor_gb in parallel.yaml", [
    {"write": {".ai-gent/parallel.yaml": "disk_floor_gb: 190\nvolume_estimate_gb: 2.5\n"}},
    wt("add", "feat/a"), wt("add", "feat/b"),
])
case("remove-containers", "test_remove_releases_the_tracks_containers_and_volumes", [
    wt("add", "feat/a"), git("switch", "-q", "dev"),
    {"engine_state": {
        "containers": [{"id": "c1", "labels": {P: "shop", W: "shop/feat-a"}},
                       {"id": "c2", "labels": {CP: "shop-feat-a"}},
                       {"id": "c3", "labels": {P: "shop", W: "shop/main"}},
                       {"id": "c4", "labels": {P: "other", W: "other/feat-a"}}],
        "volumes": [{"name": "shop-tools-target-feat-a", "labels": {P: "shop", W: "shop/feat-a"}},
                    {"name": "shop-feat-a_db", "labels": {CP: "shop-feat-a"}},
                    {"name": "shop-tools-target", "labels": {P: "shop", W: "shop/main"}},
                    {"name": "other-target-feat-a", "labels": {P: "other", W: "other/feat-a"}}]}},
    wt("remove", "feat/a"), STATE_STEP, SCOPED,
])
case("remove-keeps-the-worktree-when-a-volume-stays", "test_remove_keeps_the_worktree_when_a_volume_stays", [
    wt("add", "feat/a"), git("switch", "-q", "dev"),
    {"engine_state": {"volumes": [{"name": "v1", "labels": {P: "shop", W: "shop/feat-a"}}], "stuck": ["v1"]}},
    wt("remove", "feat/a"), {"exists": ".worktrees/feat-a"}, wt("remove", "feat/a", "--keep-containers"),
])
case("resources-orphans", "test_resources_reports_orphans_and_never_removes", [
    wt("add", "feat/a"),
    {"engine_state": {
        "images": [{"id": "i1", "labels": {P: "shop"}, "dangling": True}, {"id": "i2", "labels": {P: "shop"}},
                   {"id": "i3", "labels": {P: "other"}, "dangling": True}],
        "volumes": [{"name": "v-a", "labels": {P: "shop", W: "shop/feat-a"}},
                    {"name": "v-gone", "labels": {P: "shop", W: "shop/feat-gone"}},
                    {"name": "shop-old_db", "labels": {CP: "shop-old"}},
                    {"name": "x_db", "labels": {CP: "x"}}],
        "containers": [{"id": "c1", "labels": {P: "shop"}}, {"id": "bx", "name": "w", "status": "Storage", "external": True}]}},
    wt("resources", "--json"), wt("resources"), SCOPED, STATE_STEP,
])
case("resources-below-floor", "resources text, floor reached", [
    {"usage": [100, 5]}, wt("resources"), wt("resources", "--json"),
])
case("finish-removes-dangling-and-new-buildah-containers-only",
     "test_finish_removes_dangling_and_new_buildah_containers_only", [
    {"engine_state": {"containers": [{"id": "b-old", "name": "old-working-container", "status": "Storage", "external": True}]}},
    wt("baseline"),
    {"engine_update": {"containers_append": [{"id": "b-new", "name": "new-working-container", "status": "Storage", "external": True}],
                       "images": [{"id": "i1", "labels": {P: "shop"}, "dangling": True},
                                  {"id": "i2", "labels": {P: "shop"}, "dangling": True},
                                  {"id": "i3", "labels": {P: "other"}, "dangling": True}],
                       "stuck": ["i2"]}},
    wt("finish"), STATE_STEP, SCOPED,
])
case("finish-leaves-buildah-containers-while-a-build-runs",
     "test_finish_leaves_buildah_containers_while_a_build_runs", [
    wt("baseline"),
    {"engine_update": {"containers_append": [{"id": "b-new", "name": "w", "status": "Storage", "external": True}]}},
    {"builds": {"mine": [], "other": ["podman build -t elsewhere ."]}},
    wt("finish"), STATE_STEP,
    {"rm": ".worktrees/.containers-baseline.json"}, {"builds": {"mine": [], "other": []}},
    wt("finish"), STATE_STEP,
])
case("finish-a-removal-that-does-not-take", "finish exit 1", [
    wt("baseline"),
    {"engine_update": {"containers_append": [{"id": "b-new", "name": "w", "status": "Storage", "external": True}],
                       "stuck": ["b-new"]}},
    wt("finish"),
])
case("no-engine", "test_without_an_engine_containers_are_skipped", [
    wt("add", "feat/a"), wt("resources"), wt("baseline"), wt("finish"), wt("remove", "feat/a"),
], engine=False)


# ------------------------------------------------------------------ Python runner

ENV = {"GIT_AUTHOR_NAME": "t", "GIT_AUTHOR_EMAIL": "t@t", "GIT_COMMITTER_NAME": "t",
       "GIT_COMMITTER_EMAIL": "t@t", "GIT_CONFIG_GLOBAL": os.devnull, "GIT_CONFIG_NOSYSTEM": "1"}


def norm(text, root):
    return text.replace(root, "<ROOT>").replace(root.replace("/", "\\"), "<ROOT>")


def state_view(path):
    st = json.loads(Path(path).read_text())
    return {"containers": [c["id"] for c in st["containers"]], "volumes": [v["name"] for v in st["volumes"]],
            "images": [i["id"] for i in st["images"]], "calls": st["calls"]}


def scoped(path):
    problems = []
    for call in json.loads(Path(path).read_text())["calls"]:
        if "prune" in call:
            problems.append(f"prune in {call}")
        if call[:1] in (["ps"], ["images"]) or call[:2] == ["volume", "ls"]:
            if "--external" not in call and not any(a.startswith("label=") for a in call):
                problems.append(f"unfiltered listing {call}")
    return problems


def run_case(case, py_wt):
    """Run one case through wt.py; returns the recorded `expect` for every step."""
    import test_wt  # FAKE_ENGINE only; the tests stay unchanged
    results = []
    with tempfile.TemporaryDirectory() as tmp:
        tmp = str(Path(tmp).resolve())
        repo = Path(tmp) / "shop"
        saved = {k: os.environ.get(k) for k in [*ENV, "CONTAINER_ENGINE", "FAKE_ENGINE_STATE"]}
        old = os.getcwd()
        usage = [500, 200]
        builds = {"mine": [], "other": []}
        real_usage, real_builds, real_which = py_wt.store_usage, py_wt.builds_running, py_wt.shutil.which
        py_wt.store_usage = lambda e: (usage[0] * py_wt.GB, usage[1] * py_wt.GB, tmp)
        py_wt.builds_running = lambda r: (list(builds["mine"]), list(builds["other"]))
        try:
            os.environ.update(ENV)
            sh = lambda *a, cwd=repo: subprocess.run(["git", *a], cwd=cwd, check=True, capture_output=True, text=True).stdout
            sh("init", "-q", "-b", "main", str(repo), cwd=tmp)
            (repo / "specs").mkdir()
            (repo / "specs" / "README.md").write_text(case.get("readme", README))
            sh("add", ".")
            sh("commit", "-q", "-m", "chore: initial commit")
            sh("branch", "dev")
            fake = Path(tmp) / "fake-engine"
            fake.write_text(test_wt.FAKE_ENGINE)
            fake.chmod(0o755)
            state_path = Path(tmp) / "engine.json"
            state_path.write_text(json.dumps({"containers": [], "volumes": [], "images": [], "calls": []}))
            os.environ["CONTAINER_ENGINE"] = str(fake)
            os.environ["FAKE_ENGINE_STATE"] = str(state_path)
            if case.get("engine") is False:
                os.environ["CONTAINER_ENGINE"] = ""
                py_wt.shutil.which = lambda name: None
            os.chdir(repo)
            for step in case["steps"]:
                kind = next(k for k in step if k not in ("cwd", "expect"))
                cwd = repo / step.get("cwd", ".")
                if kind == "wt":
                    os.chdir(cwd)
                    out, err = io.StringIO(), io.StringIO()
                    with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
                        code = py_wt.main(list(step["wt"]))
                    os.chdir(repo)
                    got = {"code": code, "stdout": norm(out.getvalue(), tmp), "stderr": norm(err.getvalue(), tmp)}
                elif kind == "git":
                    sh(*step["git"], cwd=cwd)
                    got = None
                elif kind == "write":
                    for rel, text in step["write"].items():
                        (repo / rel).parent.mkdir(parents=True, exist_ok=True)
                        (repo / rel).write_text(text)
                    got = None
                elif kind == "rm":
                    (repo / step["rm"]).unlink()
                    got = None
                elif kind == "engine_state":
                    state_path.write_text(json.dumps({"containers": [], "volumes": [], "images": [], "calls": [], **step["engine_state"]}))
                    got = None
                elif kind == "engine_update":
                    st = json.loads(state_path.read_text())
                    for k, v in step["engine_update"].items():
                        if k == "containers_append":
                            st["containers"] += v
                        else:
                            st[k] = v
                    state_path.write_text(json.dumps(st))
                    got = None
                elif kind == "usage":
                    usage[:] = step["usage"]
                    got = None
                elif kind == "builds":
                    builds.update(step["builds"])
                    got = None
                elif kind == "state":
                    got = state_view(state_path)
                elif kind == "assert_scoped":
                    got = {"problems": scoped(state_path)}
                elif kind == "cat":
                    text = (repo / step["cat"]).read_text()
                    if "tail" in step:
                        text = "".join(f"{ln}\n" for ln in text.splitlines()[-step["tail"]:])
                    got = {"text": norm(text, tmp)}
                elif kind == "exists":
                    got = {"exists": (repo / step["exists"]).exists()}
                elif kind == "gitout":
                    got = {"text": norm(sh(*step["gitout"]).strip(), tmp)}
                else:
                    raise SystemExit(f"unknown step {kind}")
                results.append(got)
        finally:
            os.chdir(old)
            py_wt.store_usage, py_wt.builds_running, py_wt.shutil.which = real_usage, real_builds, real_which
            for k, v in saved.items():
                if v is None:
                    os.environ.pop(k, None)
                else:
                    os.environ[k] = v
    return results


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--bless", action="store_true")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()
    if not (SCRIPTS / "wt.py").exists():
        if args.bless:
            sys.exit("wt.py is gone (3.0.0 cutover): the fixtures stay, only the Rust runner remains")
        print("wt.py is gone; nothing to compare (the Rust runner owns the fixtures)")
        return 0
    import wt as py_wt
    cases = CASES
    if args.self_test:
        cases = [CASES[0]]
    failures = 0
    FIXTURES.mkdir(parents=True, exist_ok=True)
    for c in cases:
        results = run_case(c, py_wt)
        recorded = {**c, "readme": c.get("readme", README), "steps": [{**s, **({"expect": r} if r is not None else {})} for s, r in zip(c["steps"], results)]}
        path = FIXTURES / f"{c['name']}.json"
        if args.bless:
            path.write_text(json.dumps(recorded, indent=1, ensure_ascii=False) + "\n")
            continue
        if args.self_test:
            ok = results[0]["code"] == 0 and '"ready"' in results[0]["stdout"]
            print("self-test", "ok" if ok else "FAILED")
            return 0 if ok else 1
        if not path.exists() or json.loads(path.read_text()) != json.loads(json.dumps(recorded)):
            print(f"DRIFT {c['name']}: {path.name} differs from what wt.py does (run --bless and review)")
            failures += 1
        bad = [r for r in results if r and r.get("problems")]
        if bad:
            print(f"FAIL {c['name']}: {bad}")
            failures += 1
    extra = {p.name for p in FIXTURES.glob("*.json")} - {f"{c['name']}.json" for c in CASES}
    if extra and not args.bless and not args.self_test:
        print(f"stray fixtures: {sorted(extra)}")
        failures += 1
    if args.bless:
        print(f"blessed {len(cases)} cases into {FIXTURES.relative_to(ROOT)}")
        return 0
    print(f"{len(cases) - failures} of {len(cases)} wt parity cases match wt.py" if not failures else f"{failures} failure(s)")
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())
