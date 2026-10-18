"""Unit tests for wt.py: python3 -m unittest plugins/agents/scripts/test_wt.py

A throwaway repository with a dev branch and a specs/README.md: which
features are ready, worktrees under .worktrees/ kept out of git, their
compose names and port offsets, listing, and removal only when merged and
clean.
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

    def tearDown(self):
        os.chdir(self.old)
        for k, v in self.saved.items():
            if v is None:
                os.environ.pop(k, None)
            else:
                os.environ[k] = v
        self.tmp.cleanup()

    def git(self, *args, cwd=None):
        return subprocess.run(["git", *args], cwd=cwd or self.repo, check=True, capture_output=True, text=True).stdout

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


if __name__ == "__main__":
    unittest.main()
