"""Unit tests for wt.py: python3 -m unittest plugins/agents/scripts/test_wt.py

A throwaway repository with a dev branch and a specs/README.md: which
features are ready, worktrees under .worktrees/ kept out of git, their
compose names, port offsets and labels, listing, and removal only when
merged and clean. Every container command goes to a fake CONTAINER_ENGINE
(a script over a JSON state file) — the tests never touch a real engine —
which fails on any prune and records every call.
"""

import contextlib
import io
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import wt  # noqa: E402

README = """# Features

| # | Feature | Priority | Depends on | Frontend | Backend |
|---|---|---|---|---|---|
| 001 | auth | P0 | — | Implemented | Verified |
| 002 | orgs | P0 | 001 | Planned | In progress |
| 003 | billing | P1 | 002 | Planned | Planned |
| 004 | search | P2 | 001 | N/A | Planned |
| 005 | reports | P2 | — | Verified | Verified |
"""

FAKE_ENGINE = r'''#!/usr/bin/env python3
import json, os, sys
path = os.environ["FAKE_ENGINE_STATE"]
st = json.load(open(path))
argv = sys.argv[1:]
st["calls"].append(argv)
def save():
    json.dump(st, open(path, "w"))
def opt(name):
    return [argv[i + 1] for i, a in enumerate(argv[:-1]) if a == name]
def match(obj, filters):
    for f in filters:
        k, _, v = f.partition("=")
        if k == "dangling":
            if bool(obj.get("dangling")) != (v == "true"):
                return False
        elif k == "label":
            lk, eq, lv = v.partition("=")
            labels = obj.get("labels", {})
            if lk not in labels or (eq and labels[lk] != lv):
                return False
    return True
def fmt(obj, f):
    for k, v in (("ID", "id"), ("Names", "name"), ("Status", "status"), ("Name", "name")):
        f = f.replace("{{." + k + "}}", str(obj.get(v, "")))
    return f.replace("\\t", "\t")
out, code = "", 0
if any(a == "prune" for a in argv):
    code = 99
elif argv[:1] == ["info"]:
    out = st.get("graphroot", "/")
elif argv[:2] == ["system", "df"]:
    out = "TYPE TOTAL"
elif argv[:1] == ["ps"]:
    rows = [c for c in st["containers"] if bool(c.get("external")) == ("--external" in argv) or
            ("--external" in argv and not c.get("external"))]
    rows = [c for c in rows if match(c, opt("--filter"))]
    out = "\n".join(c["id"] if "-aq" in argv else fmt(c, opt("--format")[0]) for c in rows)
elif argv[:2] == ["volume", "ls"]:
    out = "\n".join(v["name"] for v in st["volumes"] if match(v, opt("--filter")))
elif argv[:2] == ["volume", "inspect"]:
    out = json.dumps([{"Name": v["name"], "Labels": v.get("labels", {})} for v in st["volumes"] if v["name"] in argv[2:]])
elif argv[:1] == ["images"]:
    out = "\n".join(i["id"] for i in st["images"] if match(i, opt("--filter")))
elif argv[:2] == ["volume", "rm"]:
    if argv[2] in st.get("stuck", []):
        code = 1
    st["volumes"] = [v for v in st["volumes"] if v["name"] != argv[2] or code]
elif argv[:1] == ["rm"]:
    st["containers"] = [c for c in st["containers"] if c["id"] != argv[-1] or c["id"] in st.get("stuck", [])]
elif argv[:1] == ["rmi"]:
    if argv[1] in st.get("stuck", []):
        code = 1
    st["images"] = [i for i in st["images"] if i["id"] != argv[1] or code]
else:
    code = 2
save()
print(out)
sys.exit(code)
'''

P, W = "me.lucasvmigotto.ai-gent.project", "me.lucasvmigotto.ai-gent.worktree"

ENV = {"GIT_AUTHOR_NAME": "t", "GIT_AUTHOR_EMAIL": "t@t", "GIT_COMMITTER_NAME": "t",
       "GIT_COMMITTER_EMAIL": "t@t", "GIT_CONFIG_GLOBAL": "/dev/null"}


class WtTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.repo = Path(self.tmp.name) / "shop"
        self.saved = {k: os.environ.get(k) for k in ENV}
        os.environ.update(ENV)
        self.git("init", "-q", "-b", "main", str(self.repo), cwd=self.tmp.name)
        (self.repo / "specs").mkdir()
        (self.repo / "specs" / "README.md").write_text(README)
        self.git("add", ".")
        self.git("commit", "-q", "-m", "chore: initial commit")
        self.git("branch", "dev")
        self.old = os.getcwd()
        os.chdir(self.repo)
        self.engine_env = {k: os.environ.get(k) for k in ("CONTAINER_ENGINE", "FAKE_ENGINE_STATE")}
        fake = Path(self.tmp.name) / "fake-engine"
        fake.write_text(FAKE_ENGINE)
        fake.chmod(0o755)
        self.state_path = Path(self.tmp.name) / "engine.json"
        self.set_state()
        os.environ["CONTAINER_ENGINE"] = str(fake)
        os.environ["FAKE_ENGINE_STATE"] = str(self.state_path)
        self.usage = (500 * wt.GB, 200 * wt.GB, self.tmp.name)                 # total, free, path
        self.real_usage, wt.store_usage = wt.store_usage, lambda e: self.usage
        self.real_builds, wt.builds_running = wt.builds_running, lambda r: ([], [])

    def tearDown(self):
        os.chdir(self.old)
        wt.store_usage, wt.builds_running = self.real_usage, self.real_builds
        for k, v in self.engine_env.items():
            if v is None:
                os.environ.pop(k, None)
            else:
                os.environ[k] = v
        for k, v in self.saved.items():
            if v is None:
                os.environ.pop(k, None)
            else:
                os.environ[k] = v
        self.tmp.cleanup()

    def git(self, *args, cwd=None):
        return subprocess.run(["git", *args], cwd=cwd or self.repo, check=True, capture_output=True, text=True).stdout

    def set_state(self, **kw):
        state = {"containers": [], "volumes": [], "images": [], "calls": [], **kw}
        self.state_path.write_text(json.dumps(state))

    def state(self):
        return json.loads(self.state_path.read_text())

    def assert_scoped(self):
        """No prune, and every listing is filtered by a label (or dangling plus a label)."""
        for call in self.state()["calls"]:
            self.assertNotIn("prune", call)
            if call[:1] in (["ps"], ["images"]) or call[:2] == ["volume", "ls"]:
                if "--external" in call:
                    continue
                self.assertTrue(any(a.startswith("label=") for a in call), call)

    def run_wt(self, *argv):
        out, err = io.StringIO(), io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            code = wt.main(list(argv))
        return code, out.getvalue(), err.getvalue()

    def test_plan(self):
        code, out, _ = self.run_wt("plan", "--json")
        self.assertEqual(code, 0)
        plan = json.loads(out)
        self.assertEqual([x["number"] for x in plan["ready"]], ["002", "004"])  # 001 is built
        self.assertEqual(plan["ready"][1]["todo"], {"backend": "Planned"})         # N/A isn't a track
        self.assertEqual([(x["number"], x["waiting"]) for x in plan["blocked"]], [("003", ["002"])])
        self.assertEqual(plan["done"], ["001", "005"])

    def test_add_list_remove(self):
        code, out, err = self.run_wt("add", "feat/002-orgs")
        self.assertEqual(code, 0, err)
        info = json.loads(out)
        path = self.repo / ".worktrees" / "feat-002-orgs"
        self.assertEqual(Path(info["path"]), path)
        self.assertEqual((info["base"], info["compose_project"], info["port_offset"]), ("dev", "shop-feat-002-orgs", 100))
        self.assertEqual(self.git("status", "--porcelain"), "")                    # .worktrees/ is excluded
        code, out, _ = self.run_wt("add", "feat/004-search")
        self.assertEqual(json.loads(out)["port_offset"], 200)

        (path / "orgs.py").write_text("x = 1\n")
        code, out, _ = self.run_wt("list")
        self.assertIn("uncommitted changes", out)
        with self.assertRaises(wt.Failed):
            wt.cmd_remove(type("A", (), {"branch": "feat/002-orgs"}))                # dirty
        self.git("add", ".", cwd=path)
        self.git("commit", "-q", "-m", "feat: orgs", cwd=path)
        code, out, _ = self.run_wt("list", "--json")
        row = next(x for x in json.loads(out) if x["branch"] == "feat/002-orgs")
        self.assertEqual((row["ahead"], row["behind"], row["dirty"]), (1, 0, False))
        code, _, err = self.run_wt("remove", "feat/002-orgs")
        self.assertNotEqual(code, 0)
        self.assertIn("land it first", err)                                        # unmerged
        self.git("switch", "-q", "dev")
        self.git("merge", "-q", "--ff-only", "feat/002-orgs")
        code, out, err = self.run_wt("remove", "feat/002-orgs")
        self.assertEqual(code, 0, err)
        self.assertFalse(path.exists())
        self.assertNotIn("feat/002-orgs", self.git("branch"))

    def test_refusals(self):
        self.git("branch", "develop")
        code, _, err = self.run_wt("add", "feat/x")
        self.assertIn("ambiguous", err)                                            # dev and develop: ask
        code, _, err = self.run_wt("add", "feat/x", "--base", "dev")
        self.assertEqual(code, 0, err)
        code, _, err = self.run_wt("add", "feat/x", "--base", "dev")
        self.assertIn("exists", err)

    def test_offsets_never_reuse_a_live_one(self):
        self.run_wt("add", "feat/a")
        self.run_wt("add", "feat/b")
        self.git("switch", "-q", "dev")
        self.run_wt("remove", "feat/a")                                            # nothing to land: removable
        code, out, _ = self.run_wt("add", "feat/c")
        self.assertEqual(json.loads(out)["port_offset"], 100)                       # a's freed offset, not b's 200

    def test_remove_after_a_rebase_merge(self):
        self.run_wt("add", "feat/x")
        wt_path = self.repo / ".worktrees" / "feat-x"
        (wt_path / "x.py").write_text("x = 1\n")
        self.git("add", ".", cwd=wt_path)
        self.git("commit", "-q", "-m", "feat: x", cwd=wt_path)
        self.git("switch", "-q", "dev")
        self.git("cherry-pick", "feat/x")                                     # same change, a new hash
        code, out, err = self.run_wt("remove", "feat/x")
        self.assertEqual(code, 0, err)
        self.assertNotIn("feat/x", self.git("branch"))

    def test_plan_reads_the_worktree_it_runs_in(self):
        self.run_wt("add", "feat/specs")
        wt_path = self.repo / ".worktrees" / "feat-specs"
        (wt_path / "specs" / "README.md").write_text(README.replace("| 004 | search | P2 | 001 | N/A | Planned |",
                                                                      "| 004 | search | P2 | 001 | N/A | Implemented |"))
        os.chdir(wt_path)
        code, out, _ = self.run_wt("plan", "--json")
        self.assertEqual([x["number"] for x in json.loads(out)["ready"]], ["002"])  # this worktree's README

    def test_from_inside_a_worktree(self):
        self.run_wt("add", "feat/002-orgs")
        os.chdir(self.repo / ".worktrees" / "feat-002-orgs")
        code, out, _ = self.run_wt("list", "--json")
        self.assertEqual([x["branch"] for x in json.loads(out)], ["feat/002-orgs"])

    def test_add_prints_labels_and_checks_disk(self):
        code, out, err = self.run_wt("add", "feat/a")
        self.assertEqual(code, 0, err)
        self.assertEqual(json.loads(out)["labels"], {P: "shop", W: "shop/feat-a"})
        self.assertIn("AI_GENT_WORKTREE=feat-a", err)
        self.usage = (500 * wt.GB, 65 * wt.GB, self.tmp.name)                   # 65 - 2 x 10 < 50 (10%)
        code, _, err = self.run_wt("add", "feat/b")
        self.assertNotEqual(code, 0)
        self.assertIn("floor", err)
        self.assertFalse((self.repo / ".worktrees" / "feat-b").exists())            # refused before git
        code, _, err = self.run_wt("add", "feat/b", "--min-free-gb", "0")
        self.assertEqual(code, 0, err)
        self.assert_scoped()

    def test_add_reads_settings_and_caps_volumes(self):
        (self.repo / ".ai-gent").mkdir()
        (self.repo / ".ai-gent" / "parallel.yaml").write_text("max_parallel: 3\nmax_volumes: 1\n"
                                                               "volume_estimate_gb: 1 # small\n")
        self.usage = (100 * wt.GB, 22 * wt.GB, self.tmp.name)                    # floor 20: 22 - 1 ok
        self.assertEqual(self.run_wt("add", "feat/a")[0], 0)
        code, _, err = self.run_wt("add", "feat/b")
        self.assertIn("cap is 1", err)

    def test_remove_releases_the_tracks_containers_and_volumes(self):
        self.run_wt("add", "feat/a")
        self.git("switch", "-q", "dev")
        self.set_state(
            containers=[{"id": "c1", "labels": {P: "shop", W: "shop/feat-a"}},
                        {"id": "c2", "labels": {"com.docker.compose.project": "shop-feat-a"}},
                        {"id": "c3", "labels": {P: "shop", W: "shop/main"}},
                        {"id": "c4", "labels": {P: "other", W: "other/feat-a"}}],
            volumes=[{"name": "shop-tools-target-feat-a", "labels": {P: "shop", W: "shop/feat-a"}},
                     {"name": "shop-feat-a_db", "labels": {"com.docker.compose.project": "shop-feat-a"}},
                     {"name": "shop-tools-target", "labels": {P: "shop", W: "shop/main"}},
                     {"name": "other-target-feat-a", "labels": {P: "other", W: "other/feat-a"}}])
        code, out, err = self.run_wt("remove", "feat/a")
        self.assertEqual(code, 0, err)
        st = self.state()
        self.assertEqual([c["id"] for c in st["containers"]], ["c3", "c4"])        # main and other project stay
        self.assertEqual([v["name"] for v in st["volumes"]], ["shop-tools-target", "other-target-feat-a"])
        self.assertIn("removed volume shop-tools-target-feat-a", out)
        self.assert_scoped()

    def test_remove_keeps_the_worktree_when_a_volume_stays(self):
        self.run_wt("add", "feat/a")
        self.git("switch", "-q", "dev")
        self.set_state(volumes=[{"name": "v1", "labels": {P: "shop", W: "shop/feat-a"}}], stuck=["v1"])
        code, _, err = self.run_wt("remove", "feat/a")
        self.assertNotEqual(code, 0)
        self.assertIn("still there", err)
        self.assertTrue((self.repo / ".worktrees" / "feat-a").exists())
        code, _, err = self.run_wt("remove", "feat/a", "--keep-containers")
        self.assertEqual(code, 0, err)

    def test_resources_reports_orphans_and_never_removes(self):
        self.run_wt("add", "feat/a")
        self.set_state(
            images=[{"id": "i1", "labels": {P: "shop"}, "dangling": True}, {"id": "i2", "labels": {P: "shop"}},
                    {"id": "i3", "labels": {P: "other"}, "dangling": True}],
            volumes=[{"name": "v-a", "labels": {P: "shop", W: "shop/feat-a"}},
                     {"name": "v-gone", "labels": {P: "shop", W: "shop/feat-gone"}},
                     {"name": "shop-old_db", "labels": {"com.docker.compose.project": "shop-old"}},
                     {"name": "x_db", "labels": {"com.docker.compose.project": "x"}}])
        code, out, err = self.run_wt("resources", "--json")
        self.assertEqual(code, 0, err)
        rep = json.loads(out)
        self.assertEqual(rep["dangling_images"], ["i1"])
        self.assertEqual({v["name"]: v["orphan"] for v in rep["volumes"]},
                         {"v-a": False, "v-gone": True, "shop-old_db": True})
        calls = self.state()["calls"]
        self.assertFalse([c for c in calls if c[:1] in (["rm"], ["rmi"]) or c[:2] == ["volume", "rm"]])
        self.assert_scoped()

    def test_finish_removes_dangling_and_new_buildah_containers_only(self):
        self.set_state(containers=[{"id": "b-old", "name": "old-working-container", "status": "Storage",
                                    "external": True}])
        self.assertEqual(self.run_wt("baseline")[0], 0)
        st = self.state()
        st["containers"].append({"id": "b-new", "name": "new-working-container", "status": "Storage",
                                 "external": True})
        st["images"] = [{"id": "i1", "labels": {P: "shop"}, "dangling": True},
                        {"id": "i2", "labels": {P: "shop"}, "dangling": True},
                        {"id": "i3", "labels": {P: "other"}, "dangling": True}]
        st["stuck"] = ["i2"]
        self.state_path.write_text(json.dumps(st))
        code, out, err = self.run_wt("finish")
        self.assertEqual(code, 0, err)
        st = self.state()
        self.assertEqual([c["id"] for c in st["containers"]], ["b-old"])
        self.assertEqual([i["id"] for i in st["images"]], ["i2", "i3"])            # in use, other project
        self.assertIn("image i2 (in use)", out)
        self.assertFalse([c for c in st["calls"] if c[:1] == ["rmi"] and "-f" in c])
        self.assert_scoped()

    def test_finish_leaves_buildah_containers_while_a_build_runs(self):
        self.run_wt("baseline")
        st = self.state()
        st["containers"].append({"id": "b-new", "name": "w", "status": "Storage", "external": True})
        self.state_path.write_text(json.dumps(st))
        wt.builds_running = lambda r: ([], ["podman build -t elsewhere ."])
        code, out, _ = self.run_wt("finish")
        self.assertIn("a build is running", out)
        self.assertEqual([c["id"] for c in self.state()["containers"]], ["b-new"])
        os.remove(self.repo / ".worktrees" / ".containers-baseline.json")
        wt.builds_running = lambda r: ([], [])
        code, out, _ = self.run_wt("finish")
        self.assertIn("no baseline", out)
        self.assertEqual([c["id"] for c in self.state()["containers"]], ["b-new"])

    def test_without_an_engine_containers_are_skipped(self):
        os.environ["CONTAINER_ENGINE"] = ""
        real = wt.shutil.which
        wt.shutil.which = lambda name: None
        try:
            code, _, err = self.run_wt("add", "feat/a")
            self.assertEqual(code, 0, err)
            self.assertIn("skipping the disk check", err)
            code, _, err = self.run_wt("resources")
            self.assertIn("no container engine", err)
        finally:
            wt.shutil.which = real


if __name__ == "__main__":
    unittest.main()
