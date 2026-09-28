"""Unit tests for the project plugin's scripts: python3 -m unittest plugins/project/scripts/test_scripts.py

sessions.py against a fake Claude Code home and a fake `opencode` executable;
repo_state.py against a throwaway repository with a bare remote.
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
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))
import repo_state  # noqa: E402
import sessions  # noqa: E402


def git(cwd, *args):
    subprocess.run(["git", "-C", str(cwd), *args], check=True, capture_output=True,
                   env={**os.environ, "GIT_AUTHOR_NAME": os.environ.get("T_NAME", "Me"), "GIT_AUTHOR_EMAIL": os.environ.get("T_EMAIL", "me@example.com"),
                        "GIT_COMMITTER_NAME": "Me", "GIT_COMMITTER_EMAIL": "me@example.com"})


def commit(cwd, name, author="Me"):
    Path(cwd, name).write_text(name)
    git(cwd, "add", name)
    with mock.patch.dict(os.environ, {"T_NAME": author, "T_EMAIL": f"{author.lower()}@example.com"}):
        git(cwd, "commit", "-q", "-m", f"add {name}")


def run(mod, *argv):
    out = io.StringIO()
    with contextlib.redirect_stdout(out):
        code = mod.main(list(argv))
    return code, out.getvalue()


class SessionsTest(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.repo = self.tmp / "work" / "shop"
        self.repo.mkdir(parents=True)
        git(self.repo, "init", "-q")
        self.home = self.tmp / "claude"
        slug = sessions.claude_slug(self.repo.resolve())
        self.proj = self.home / "projects" / slug
        self.proj.mkdir(parents=True)
        self.bin = self.tmp / "bin"
        self.bin.mkdir()
        self.env = mock.patch.dict(os.environ, {"CLAUDE_CONFIG_DIR": str(self.home), "PATH": f"{self.bin}:{os.environ['PATH']}",
                                                "CLAUDE_CODE_SESSION_ID": "current"})
        self.env.start()
        self.addCleanup(self.env.stop)

    def transcript(self, name, records):
        path = self.proj / f"{name}.jsonl"
        path.write_text("\n".join(json.dumps(r) for r in records) + "\n")
        return path

    def claude_session(self):
        cwd = str(self.repo)
        return self.transcript("old", [
            {"type": "user", "cwd": cwd, "timestamp": "2026-09-20T10:00:00Z", "gitBranch": "feat/cart",
             "message": {"role": "user", "content": "Add the cart page"}},
            {"type": "assistant", "timestamp": "2026-09-20T10:01:00Z", "gitBranch": "feat/cart", "message": {"content": [
                {"type": "text", "text": "Adding it."},
                {"type": "tool_use", "name": "Edit", "input": {"file_path": f"{cwd}/src/cart.ts"}},
            ]}},
            {"type": "user", "timestamp": "2026-09-20T10:01:05Z", "message": {"content": [{"type": "tool_result", "content": "SECRET FILE BODY"}]}},
            {"type": "assistant", "timestamp": "2026-09-20T10:02:00Z", "gitBranch": "feat/cart", "message": {"content": [
                {"type": "tool_use", "name": "Bash", "input": {"command": "DB_PASSWORD=hunter2 npm test"}},
            ]}},
            {"type": "user", "timestamp": "2026-09-20T10:02:30Z", "isCompactSummary": True, "message": {"content": "Summary: cart half done"}},
            {"type": "assistant", "timestamp": "2026-09-20T10:03:00Z", "isSidechain": True, "message": {"content": [{"type": "text", "text": "subagent chatter"}]}},
            {"type": "assistant", "timestamp": "2026-09-20T10:04:00Z", "message": {"content": [{"type": "text", "text": "Should I commit it?"}]}},
        ])

    def fake_opencode(self, sessions_json, export_json):
        (self.tmp / "list.json").write_text(json.dumps(sessions_json))
        (self.tmp / "export.json").write_text(json.dumps(export_json))
        exe = self.bin / "opencode"
        exe.write_text(f'#!/bin/sh\ncase "$2" in list) cat {self.tmp}/list.json;; export) cat {self.tmp}/export.json;; esac\n')
        exe.chmod(0o755)

    def test_claude_digest(self):
        self.claude_session()
        self.transcript("current", [{"type": "user", "cwd": str(self.repo), "message": {"content": "now"}}])
        code, out = run(sessions, "digest", "--repo", str(self.repo), "--tool", "claude", "--json")
        self.assertEqual(code, 0)
        d = json.loads(out)
        self.assertEqual(d["id"], "old")  # the running session is skipped
        self.assertEqual(d["branch"], "feat/cart")
        self.assertEqual(d["files_changed"], ["src/cart.ts"])
        self.assertEqual(d["commands"], ["DB_PASSWORD=<redacted> npm test"])
        self.assertEqual(d["compaction_summary"], "Summary: cart half done")
        self.assertIn("never answered", d["ending"])
        self.assertNotIn("SECRET FILE BODY", out)
        self.assertNotIn("subagent chatter", out)

    def test_stopped_mid_task(self):
        self.transcript("cut", [
            {"type": "user", "cwd": str(self.repo), "timestamp": "2026-09-20T10:00:00Z", "message": {"content": "Migrate it"}},
            {"type": "assistant", "timestamp": "2026-09-20T10:01:00Z", "message": {"content": [
                {"type": "tool_use", "name": "Bash", "input": {"command": "npm run migrate"}}]}},
        ])
        d = json.loads(run(sessions, "digest", "--repo", str(self.repo), "--tool", "claude", "--json")[1])
        self.assertIn("mid-task", d["ending"])

    def test_subdirectory_sessions_belong_only_when_their_cwd_is_inside(self):
        other = self.home / "projects" / (self.proj.name + "-api")
        other.mkdir()
        (other / "in.jsonl").write_text(json.dumps({"type": "user", "cwd": str(self.repo / "api"), "message": {"content": "hi"}}) + "\n")
        sibling = self.home / "projects" / (self.proj.name + "-legacy")
        sibling.mkdir()
        (sibling / "out.jsonl").write_text(json.dumps({"type": "user", "cwd": str(self.repo) + "-legacy", "message": {"content": "x"}}) + "\n")
        ids = {s["id"] for s in json.loads(run(sessions, "list", "--repo", str(self.repo), "--tool", "claude", "--json")[1])}
        self.assertEqual(ids, {"in"})

    def test_opencode_digest(self):
        self.fake_opencode(
            [{"id": "ses_1", "title": "Fix login", "updated": 1790000000000, "directory": str(self.repo)},
             {"id": "ses_2", "title": "Elsewhere", "updated": 1790000001000, "directory": "/elsewhere"}],
            {"messages": [
                {"type": "user", "time": {"created": 1789999990000}, "text": "Fix the login, token ghp_abcdefghijklmnopqrstuvwxyz0123"},
                {"type": "assistant", "time": {"created": 1789999991000}, "content": [
                    {"type": "text", "text": "Done — see auth.ts."},
                    {"type": "tool", "name": "edit", "state": {"status": "completed", "input": {"filePath": f"{self.repo}/auth.ts"}}},
                ]},
                {"type": "compaction", "time": {"created": 1789999992000}, "summary": "login fixed"},
            ]})
        listed = json.loads(run(sessions, "list", "--repo", str(self.repo), "--tool", "opencode", "--json")[1])
        self.assertEqual([s["id"] for s in listed], ["ses_1"])
        d = json.loads(run(sessions, "digest", "--repo", str(self.repo), "--tool", "opencode", "--json")[1])
        self.assertEqual(d["files_changed"], ["auth.ts"])
        self.assertEqual(d["compaction_summary"], "login fixed")
        self.assertIn("<token>", d["requests"][0])

    def test_both_tools_newest_first(self):
        path = self.claude_session()
        os.utime(path, (1790000005, 1790000005))
        self.fake_opencode([{"id": "ses_1", "title": "t", "updated": 1790000000000, "directory": str(self.repo)}], {"messages": []})
        listed = json.loads(run(sessions, "list", "--repo", str(self.repo), "--json")[1])
        self.assertEqual([s["tool"] for s in listed], ["claude", "opencode"])

    def test_redaction(self):
        for text, hidden in [("postgres://app:s3cret@db:5432/x", "s3cret"), ("Authorization: Bearer abcdefghijklmnop", "abcdefghijklmnop"),
                             ("export API_KEY='x y z'", "x y z"), ("-----BEGIN RSA PRIVATE KEY-----\nMIIE\n-----END RSA PRIVATE KEY-----", "MIIE")]:
            with self.subTest(text=text):
                self.assertNotIn(hidden, sessions.redact(text))


class RepoStateTest(unittest.TestCase):
    def setUp(self):
        self.tmp = Path(tempfile.mkdtemp())
        self.remote = self.tmp / "remote.git"
        git(self.tmp, "init", "-q", "--bare", "-b", "main", str(self.remote))
        self.repo = self.tmp / "repo"
        git(self.tmp, "clone", "-q", str(self.remote), str(self.repo))
        git(self.repo, "config", "user.email", "me@example.com")
        git(self.repo, "config", "user.name", "Me")
        git(self.repo, "checkout", "-q", "-b", "main")
        commit(self.repo, "a")
        git(self.repo, "push", "-q", "origin", "main")
        git(self.repo, "checkout", "-q", "-b", "dev")
        commit(self.repo, "b")
        commit(self.repo, "c", author="Other")
        git(self.repo, "push", "-q", "origin", "dev")
        git(self.repo, "checkout", "-q", "-b", "feat/x")
        commit(self.repo, "d")
        git(self.repo, "checkout", "-q", "-b", "feat/merged", "dev")
        git(self.repo, "checkout", "-q", "dev")
        commit(self.repo, "e")  # local dev ahead of origin/dev

    def snap(self, **kw):
        return repo_state.snapshot(str(self.repo), kw.get("since"), 30, [])

    def test_main_branches(self):
        s = self.snap()
        rows = {r["name"]: r for r in s["main_branches"]}
        self.assertEqual(s["base"], "main")
        self.assertEqual(set(rows), {"main", "dev"})
        self.assertEqual((rows["dev"]["local_ahead"], rows["dev"]["local_behind"]), (1, 0))
        self.assertEqual((rows["dev"]["vs_base_ahead"], rows["dev"]["vs_base_behind"]), (2, 0))  # origin/dev vs origin/main

    def test_recent_branches_skip_merged(self):
        refs = [b["ref"] for b in self.snap()["recent_branches"]]
        self.assertEqual(refs, ["feat/x"])  # feat/merged is contained in dev

    def test_working_tree(self):
        Path(self.repo, "a").write_text("changed")
        Path(self.repo, "new").write_text("n")
        Path(self.repo, "b").write_text("staged")
        git(self.repo, "add", "b")
        w = self.snap()["working_tree"]
        self.assertEqual((w["branch"], w["staged"], w["unstaged"], w["untracked"]), ("dev", ["b"], ["a"], ["new"]))
        git(self.repo, "stash", "-q")
        self.assertEqual(len(self.snap()["working_tree"]["stashes"]), 1)

    def test_since_splits_authors(self):
        c = self.snap(since="1970-01-02")["commits_since"]
        self.assertEqual(len(c["others"]), 1)
        self.assertEqual(c["total"], 5)

    def test_credentials_stripped_from_remotes(self):
        git(self.repo, "remote", "add", "up", "https://user:tok@example.com/x.git")
        self.assertIn("https://example.com/x.git", [r["url"] for r in self.snap()["remotes"]])


if __name__ == "__main__":
    unittest.main()
