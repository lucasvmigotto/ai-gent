"""Unit tests for token_report.py: python3 -m unittest plugins/agents/scripts/test_token_report.py

A fake ~/.claude*/projects/<slug> tree with a coordinator session, two subagents (one
tagged with task ids, one not) and a specs/ feature: session auto-discovery, usage summing
and dedup by message id, stage markers, the project-wide roll-up, the per-feature tokens.md
link and the Unmapped section.
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
import token_report as tr  # noqa: E402

USAGE = {"input_tokens": 1, "cache_creation_input_tokens": 10, "cache_read_input_tokens": 100, "output_tokens": 2}


def msg(mid, model, usage, ts):
    return json.dumps({"type": "assistant", "timestamp": ts,
                        "message": {"id": mid, "model": model, "usage": usage}})


def re_slug(path):
    import re
    return re.sub(r"[^A-Za-z0-9-]", "-", str(path))


def edit_msg(mid, model, usage, ts):
    block = {"type": "tool_use", "id": "tu1", "name": "Write", "input": {"file_path": "src/a.py", "content": "a\nb\nc"}}
    return json.dumps({"type": "assistant", "timestamp": ts,
                        "message": {"id": mid, "model": model, "usage": usage, "content": [block]}})


class TokenReportTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.home = Path(self.tmp.name)
        self.work = self.home / "work"
        self.work.mkdir()
        self.slug = "-tmp-fake-project"
        self.root = self.home / ".claude" / "projects" / self.slug
        (self.root / "sess1" / "subagents").mkdir(parents=True)

        (self.root / "sess1.jsonl").write_text(
            msg("m1", "claude-opus-5-5", USAGE, "2026-10-20T10:00:00Z") + "\n" +
            msg("m1", "claude-opus-5-5", USAGE, "2026-10-20T10:00:00Z") + "\n"  # repeated id: deduped, not doubled
        )
        (self.root / "sess1" / "subagents" / "agent-a1.jsonl").write_text(
            json.dumps({"type": "user", "message": {"role": "user", "content": "Goal: complete T001, T002 of specs/001-widget/tasks.md"}}) + "\n" +
            edit_msg("a1m1", "claude-sonnet-5-5", USAGE, "2026-10-20T10:01:00Z"))
        (self.root / "sess1" / "subagents" / "agent-a1.meta.json").write_text(
            json.dumps({"description": "T001, T002: build widget"}))
        (self.root / "sess1" / "subagents" / "agent-a2.jsonl").write_text(
            msg("a2m1", "claude-sonnet-5-5", USAGE, "2026-10-20T10:02:00Z"))
        (self.root / "sess1" / "subagents" / "agent-a2.meta.json").write_text(
            json.dumps({"description": "Survey unrelated repo"}))

        specs = self.work / "specs" / "001-widget"
        specs.mkdir(parents=True)
        (specs / "tasks.md").write_text("- [ ] T001 [US1] build the widget\n- [ ] T002 [US1] test the widget\n")

        self.old_home = os.environ.get("HOME")
        os.environ["HOME"] = str(self.home)
        self.old_cwd = os.getcwd()
        os.chdir(self.work)

    def tearDown(self):
        os.chdir(self.old_cwd)
        if self.old_home is None:
            os.environ.pop("HOME", None)
        else:
            os.environ["HOME"] = self.old_home
        self.tmp.cleanup()

    def test_discover_sessions_finds_the_one_session(self):
        roots = [str(self.root)]
        self.assertEqual(tr.discover_sessions(roots), ["sess1"])

    def test_dedup_by_message_id_counts_once(self):
        msgs = {}
        tr.collect(str(self.root / "sess1.jsonl"), msgs)
        self.assertEqual(len(msgs), 1)

    def test_label_of_prefers_leading_task_ids(self):
        self.assertEqual(tr.label_of("T001, T002: build widget"), "T001, T002")
        self.assertEqual(tr.label_of("C4: Oracle Free and runner placeholder"), "C4")
        self.assertEqual(tr.label_of("Survey unrelated repo"), "Survey unrelated repo")

    def test_gather_row_carries_prompt_and_edit_size(self):
        _, rows, _, _ = tr.gather([str(self.root)], ["sess1"])
        row = next(r for r in rows if r["aid"] == "a1")
        self.assertIn("specs/001-widget", row["prompt"])
        self.assertEqual((row["files"], row["lines"], row["family"]), (["src/a.py"], 3, "sonnet"))

    def test_project_name_comes_from_the_slug_not_a_hardcoded_project(self):
        self.assertEqual(tr.project_name("-home-lucas-codes-rstory"), "rstory")
        self.assertEqual(tr.project_name("-home-x-codes-my-repo"), "my-repo")
        self.assertEqual(tr.project_name(re_slug(self.work)), "work")

    def test_report_writes_project_wide_rollup(self):
        tr.cmd_report(tr.p.parse_args(["report", f"--project={self.slug}", "--out=rollup.md"]))
        text = Path("rollup.md").read_text()
        self.assertIn("Token usage report — project", text)  # derived from the slug, whatever the project
        self.assertNotIn("llm4glpi", text)
        self.assertIn("coordinator sess1", text)
        self.assertIn("T001, T002", text)

    def test_tasks_links_feature_and_flags_unmapped(self):
        tr.cmd_tasks(tr.p.parse_args(["tasks", f"--project={self.slug}", "--specs-dir=specs",
                                       "--rollup-out=rollup.md"]))
        feature = Path("specs/001-widget/tokens.md").read_text()
        self.assertIn("T001, T002", feature)
        self.assertIn("Feature total", feature)
        rollup = Path("rollup.md").read_text()
        self.assertIn("Unmapped", rollup)
        self.assertIn("Survey unrelated repo", rollup)

    def test_estimate_by_role_averages_matching_cards(self):
        tr.cmd_report(tr.p.parse_args(["report", f"--project={self.slug}", "--out=rollup.md"]))
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            tr.cmd_estimate(tr.p.parse_args(["estimate", "--role=builder", "--rollup=rollup.md"]))
        text = out.getvalue()
        self.assertIn("2 sample(s)", text)  # both subagents ran on claude-sonnet-5-5

    def test_estimate_by_label_matches_one_card(self):
        tr.cmd_report(tr.p.parse_args(["report", f"--project={self.slug}", "--out=rollup.md"]))
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            tr.cmd_estimate(tr.p.parse_args(["estimate", "--label=T001, T002", "--rollup=rollup.md"]))
        self.assertIn("1 sample(s)", out.getvalue())

    def test_estimate_with_no_history_says_so_plainly(self):
        tr.cmd_report(tr.p.parse_args(["report", f"--project={self.slug}", "--out=rollup.md"]))
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            tr.cmd_estimate(tr.p.parse_args(["estimate", "--role=scout", "--rollup=rollup.md"]))
        self.assertIn("no history", out.getvalue())

    def test_estimate_with_no_rollup_file_says_so_plainly(self):
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            tr.cmd_estimate(tr.p.parse_args(["estimate", "--role=builder", "--rollup=missing.md"]))
        self.assertIn("no history", out.getvalue())

    def test_snapshot_and_backfill_merge_without_duplicates(self):
        snaps_path = str(self.work / "snaps.json")
        tr.cmd_snapshot(tr.p.parse_args(["snapshot", "--label=manual one", "--at=2026-10-20T00:00:00Z",
                                          f"--snapshots={snaps_path}"]))
        log = "2026-10-19T00:00:00+00:00|chore: initial commit\n2026-10-20T00:00:00+00:00|manual one\n"
        proc = subprocess.run([sys.executable, str(Path(__file__).resolve().parent / "token_report.py"),
                                "backfill-git", "--repo=-", f"--snapshots={snaps_path}"],
                               input=log, capture_output=True, text=True, check=True)
        self.assertIn("backfilled", proc.stdout)
        snaps = tr.load_snapshots(snaps_path)
        self.assertEqual(len(snaps), 2)  # the duplicate (same at+label) merges, not triples


if __name__ == "__main__":
    unittest.main()
