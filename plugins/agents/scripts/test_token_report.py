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


TASKS_MD = """# Tasks
## Phase 1
- [ ] T001 [US1] build the widget in `src/a.py` — Executor: builder · Reviewer: thinker · Effort: high
- [ ] T002 [US1] test the widget — Executor: builder · Reviewer: thinker · Effort: medium
- [ ] T003 [US1] polish — Executor: builder · Reviewer: thinker · Effort: low
## Backend
- [ ] T004 [US1] serve it — Executor: builder · Reviewer: thinker · Effort: medium
"""
CARDS_MD = """# Cards
| Card | Tasks | Executor | Reviewer | Effort |
|---|---|---|---|---|
| `001-core` | T001, T002 | builder | thinker | high |
| `001-polish` | T003 | builder | thinker | low |

## Card `001-core`: T001, T002

**Route**: Executor `builder` · Effort `high` · Tokens: projected ~400k | used — · Dispatch description: `T001, T002: 001-widget core`

## Card `001-polish`: T003

**Route**: Executor `builder` · Effort `low` · Tokens: projected ~100k | used — · Dispatch description: `T003: 001-widget polish`
"""
LEDGER = """# Ledger
| Wave | Feature | Card | State |
|---|---|---|---|
| 0 | 001-widget | `001-core` | done |
| 0 | 001-widget | `001-polish` | planned |
"""


def usage(billable_out, cache_read=1000):
    return {"input_tokens": 0, "cache_creation_input_tokens": 0, "cache_read_input_tokens": cache_read,
            "output_tokens": billable_out}


class TasksFixture(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.home = Path(self.tmp.name)
        self.work = self.home / "work"
        (self.work / "specs" / "001-widget").mkdir(parents=True)
        (self.work / "specs" / "002-gadget").mkdir(parents=True)
        (self.work / "specs" / "001-widget" / "tasks.md").write_text(TASKS_MD)
        (self.work / "specs" / "001-widget" / "cards.md").write_text(CARDS_MD)
        (self.work / "specs" / "002-gadget" / "tasks.md").write_text("- [ ] T001 gadget — Executor: builder\n")
        (self.work / ".ai-gent").mkdir()
        (self.work / ".ai-gent" / "orchestrate-status.md").write_text(LEDGER)
        self.slug = "-tmp-widgets"
        self.root = self.home / ".claude" / "projects" / self.slug
        self.sub = self.root / "s1" / "subagents"
        self.sub.mkdir(parents=True)
        self.n = 0
        self.old_home, self.old_cwd = os.environ.get("HOME"), os.getcwd()
        os.environ["HOME"] = str(self.home)
        os.chdir(self.work)

    def tearDown(self):
        os.chdir(self.old_cwd)
        if self.old_home is None:
            os.environ.pop("HOME", None)
        else:
            os.environ["HOME"] = self.old_home
        self.tmp.cleanup()

    def agent(self, aid, desc, out, prompt="", ts="2026-10-20T10:00:00Z", root=None, calls=1, model="claude-sonnet-5-5"):
        root = root or self.root
        d = root / "s1" / "subagents"
        d.mkdir(parents=True, exist_ok=True)
        lines = [json.dumps({"type": "user", "message": {"role": "user", "content": prompt}})]
        for i in range(calls):
            lines.append(msg(f"{aid}-{i}", model, usage(out), ts))
        (d / f"agent-{aid}.jsonl").write_text("\n".join(lines))
        (d / f"agent-{aid}.meta.json").write_text(json.dumps({"description": desc}))

    def main(self, out, ts="2026-10-20T09:00:00Z", root=None):
        root = root or self.root
        (root / "s1.jsonl").write_text(msg("c1", "claude-opus-5-5", usage(out), ts))

    def run_tasks(self, *extra):
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            tr.cmd_tasks(tr.p.parse_args(["tasks", f"--project={self.slug}", "--specs-dir=specs",
                                           "--snapshots=none.json", *extra]))
        return buf.getvalue()

    def attrs(self):
        _, rows, _, _ = tr.gather([str(self.root)], ["s1"])
        feats = tr.load_features("specs")
        return {r["aid"]: tr.attribute(r, feats) for r in rows}


class TasksTest(TasksFixture):
    def test_attribution_levels(self):
        self.agent("a1", "T001, T002: 001-widget core", 100)
        self.agent("a2", "Review 001-polish (T003)", 50)
        self.agent("a3", "T001: 002-gadget build", 40)
        self.agent("a4", "T001 something", 30)  # T001 exists in two features: ambiguous, not guessed
        self.agent("a5", "Do the 001 follow-ups", 20)
        self.agent("a6", "Survey the repo", 10, prompt="no specs here")
        self.agent("a7", "Implement it", 10, prompt="Read specs/002-gadget/tasks.md and specs/002-gadget/spec.md")
        self.agent("a8", "T001–T003: 001-widget build", 10)
        at = self.attrs()
        self.assertEqual((at["a1"]["card"], at["a1"]["confidence"]), ("001-core", "exact"))
        self.assertEqual((at["a2"]["card"], at["a2"]["feature"]), ("001-polish", "001-widget"))
        self.assertEqual((at["a3"]["feature"], at["a3"]["card"], at["a3"]["confidence"]), ("002-gadget", None, "exact"))
        self.assertEqual(at["a4"]["confidence"], "agent")
        self.assertEqual((at["a5"]["feature"], at["a5"]["confidence"]), ("001-widget", "feature"))
        self.assertEqual(at["a6"]["confidence"], "agent")
        self.assertEqual((at["a7"]["feature"], at["a7"]["confidence"]), ("002-gadget", "feature"))
        self.assertEqual(at["a8"]["tasks"], ["T001", "T002", "T003"])  # a range expands

    def test_expand_ids_and_tokens_format(self):
        self.assertEqual(tr.expand_ids("T001–T003, T007 and T001"), ["T001", "T002", "T003", "T007"])
        self.assertEqual(tr.parse_tok("~420k"), 420000)
        self.assertEqual(tr.parse_tok("1.5M"), 1500000)
        self.assertIsNone(tr.parse_tok("—"))
        self.assertEqual((tr.fmt_tok(391_400), tr.fmt_tok(1_240_000)), ("391k", "1.24M"))

    def test_tasks_writes_lines_tracker_history_ledger(self):
        self.agent("a1", "T001, T002: 001-widget core", 391_000)
        self.agent("a2", "Review 001-core", 9_000, ts="2026-10-20T11:00:00Z")
        self.agent("a3", "Survey the repo", 5_000)
        self.main(100_000)
        out = self.run_tasks()
        t = Path("specs/001-widget/tasks.md").read_text()
        self.assertIn("Effort: high · Tokens: projected ~400k | used 400k", t)
        self.assertIn("Effort: medium · Tokens: with T001", t)
        self.assertIn("Effort: low · Tokens: projected ~100k | used —", t)  # planned only: the projection is carried over
        tracker = Path("specs/001-widget/tracker.md").read_text()
        self.assertIn("`001-core` | ~400k | 400k | +0%", tracker)
        self.assertIn("Coordinator share", tracker)
        ledger = Path(".ai-gent/orchestrate-status.md").read_text()
        self.assertIn("| Tokens |", ledger)
        self.assertIn("~400k / 400k", ledger)
        self.assertIn("~100k / —", ledger)
        hist = tr.read_history("docs/product/token-history.jsonl")
        core = next(r for r in hist if r["card"] == "001-core")
        self.assertEqual((core["used"], core["projected"], core["layer"], core["effort"], core["confidence"]),
                         (400_000, 400_000, "backend", "high", "exact"))
        self.assertEqual(core["coordinator"], 100_000)  # the only feature in the window gets all of it
        self.assertEqual(core["project"], "widgets")
        self.assertIn("unattributed (reported, not guessed): 1 agent(s)", out)
        self.assertIn("Survey the repo", out)

    def test_rerun_updates_numbers_without_duplicating(self):
        self.agent("a1", "T001, T002: 001-widget core", 100_000)
        self.main(10_000)
        self.run_tasks()
        first = [Path(f).read_text() for f in ("specs/001-widget/tasks.md", "specs/001-widget/tracker.md",
                                                 ".ai-gent/orchestrate-status.md", "docs/product/token-history.jsonl")]
        self.run_tasks()
        self.assertEqual(first, [Path(f).read_text() for f in ("specs/001-widget/tasks.md", "specs/001-widget/tracker.md",
                                                                  ".ai-gent/orchestrate-status.md", "docs/product/token-history.jsonl")])
        self.agent("a1", "T001, T002: 001-widget core", 250_000)
        self.run_tasks()
        t = Path("specs/001-widget/tasks.md").read_text()
        self.assertEqual(t.count("Tokens:"), 3)  # T001, T002 (with T001) and T003: one tag per line
        self.assertIn("used 250k", t)
        self.assertNotIn("used 100k", t)
        self.assertEqual(Path("specs/001-widget/tracker.md").read_text().count(tr.TRACKER_START), 1)
        self.assertEqual(len([r for r in tr.read_history("docs/product/token-history.jsonl") if r["card"] == "001-core"]), 1)

    def test_resumed_agent_counts_once_across_config_dirs(self):
        other = self.home / ".claude-b" / "projects" / self.slug
        self.agent("a1", "T003: 001-widget polish", 100, calls=2)
        self.agent("a1", "T003: 001-widget polish", 100, calls=3, root=other, ts="2026-10-21T10:00:00Z")  # resumed on another account
        self.main(1)
        self.run_tasks()
        rec = next(r for r in tr.read_history("docs/product/token-history.jsonl") if r["card"] == "001-polish")
        self.assertEqual(rec["agents"], ["a1"])
        self.assertEqual(rec["calls"], 3)  # message ids a1-0..a1-2: the shared ones dedupe, the extra one adds
        self.assertEqual(rec["used"], 300)

    def test_coordinator_split_by_window_weights(self):
        self.agent("a1", "T001: 002-gadget build", 300, ts="2026-10-20T10:00:00Z")
        self.agent("a2", "T003: 001-widget polish", 100, ts="2026-10-20T10:30:00Z")
        self.agent("a3", "T003: 001-widget polish", 50, ts="2026-10-21T10:30:00Z")
        (self.root / "s1.jsonl").write_text("\n".join([
            msg("c1", "claude-opus-5-5", usage(80), "2026-10-20T09:00:00Z"),
            msg("c2", "claude-opus-5-5", usage(500), "2026-10-22T09:00:00Z")]))
        snaps = [{"at": "2026-10-20T12:00:00", "label": "landed"}, {"at": "2026-10-21T12:00:00", "label": "landed 2"}]
        _, rows, _, main_msgs = tr.gather([str(self.root)], ["s1"])
        feats = tr.load_features("specs")
        share, unassigned, total = tr.split_coordinator(main_msgs, tr.build_groups(rows, feats), snaps)
        self.assertEqual(round(share["002-gadget"]), 60)  # 80 shared 300:100
        self.assertEqual(round(share["001-widget"]), 20)
        self.assertEqual((unassigned, total), (500, 580))  # no subagent ran in the last window

    def test_dry_run_writes_nothing(self):
        self.agent("a1", "T001, T002: 001-widget core", 100)
        before = Path("specs/001-widget/tasks.md").read_text()
        out = self.run_tasks("--dry-run")
        self.assertIn("dry run", out)
        self.assertEqual(Path("specs/001-widget/tasks.md").read_text(), before)
        self.assertFalse(Path("docs/product/token-history.jsonl").exists())
        self.assertFalse(Path("specs/001-widget/tracker.md").exists())


def rec(i, used, conf="exact", model="sonnet", effort="medium", layer="backend", tasks=("T001", "T002"), source="measured", agents=None, project="p"):
    return {"id": f"{project}:{i}", "source": source, "confidence": conf, "project": project, "feature": "001-x", "card": f"c{i}",
            "tasks": list(tasks), "layer": layer, "effort": effort, "executor_model": model, "used": used, "date": "2026-10-2%d" % (i % 9),
            "agents": agents or [f"ag{i}"]}


class EstimateTest(unittest.TestCase):
    def test_few_records_fall_back_to_the_starting_table(self):
        est = tr.estimate_card([rec(i, 999) for i in range(4)], "sonnet", "medium", "backend", 4)
        self.assertEqual((est["basis"], est["median"], est["p75"], est["n"]), ("starting table", 350_000, 550_000, 4))
        self.assertEqual(tr.estimate_card([], "opus", "high", tasks=9)["median"], round(700_000 * 1.6))

    def test_history_takes_over_with_median_p75_and_widens_the_match(self):
        recs = [rec(i, v) for i, v in enumerate([100, 200, 300, 400, 500])]
        est = tr.estimate_card(recs, "sonnet", "medium", "backend", 2)
        self.assertEqual((est["basis"], est["median"], est["p75"], est["n"], est["tier"]),
                         ("history", 300, 400, 5, "layer+effort+model+size"))
        # five records, but only three are the same layer: the match widens to effort+model+size
        mixed = [rec(i, v, layer="backend" if i < 3 else "qa") for i, v in enumerate([100, 200, 300, 400, 500])]
        self.assertEqual(tr.estimate_card(mixed, "sonnet", "medium", "backend", 2)["tier"], "effort+model+size")
        self.assertEqual(tr.estimate_card(recs, "opus", "medium")["basis"], "starting table")  # other model: no match

    def test_confidence_weights_pull_the_median_to_exact_records(self):
        recs = [rec(i, 1000 + i, conf="exact") for i in range(2)] + [rec(i + 10, 100 + i, conf="agent") for i in range(3)]
        est = tr.estimate_card(recs, "sonnet", "medium", "backend", 2)
        self.assertEqual(est["mix"], {"exact": 2, "agent": 3})
        self.assertEqual(est["median"], 1000)  # 2 exact (1.0 each) outweigh 3 agent-only (0.3 each)

    def test_load_history_drops_a_seed_the_repo_already_measured(self):
        with tempfile.TemporaryDirectory() as d:
            a, b = Path(d) / "a.jsonl", Path(d) / "b.jsonl"
            a.write_text(json.dumps(rec(1, 10, agents=["x"])) + "\n")
            b.write_text(json.dumps(rec(2, 20, source="seed", agents=["x"])) + "\n" + json.dumps(rec(3, 30, source="seed", agents=["y"])) + "\n")
            self.assertEqual(sorted(r["used"] for r in tr.load_history([str(a), str(b)])), [10, 30])

    def test_estimate_command_prints_projection_basis_and_card_line(self):
        with tempfile.TemporaryDirectory() as d:
            h = Path(d) / "h.jsonl"
            h.write_text("".join(json.dumps(rec(i, 100 * (i + 1) * 1000)) + "\n" for i in range(5)))
            out = io.StringIO()
            with contextlib.redirect_stdout(out):
                tr.cmd_estimate(tr.p.parse_args(["estimate", "--role=builder", "--effort=medium", "--layer=backend",
                                                  "--tasks=2", f"--history={h}"]))
            text = out.getvalue()
        self.assertIn("projection ~300k billable (median), upper bound ~400k (p75)", text)
        self.assertIn("5 similar record(s)", text)
        self.assertIn("Tokens: projected ~300k | used —", text)


class IndexSeedTest(TasksFixture):
    def setUp(self):
        super().setUp()
        self.ai_home = self.home / "ai-home"
        os.environ["AI_GENT_HOME"] = str(self.ai_home)
        self.addCleanup(os.environ.pop, "AI_GENT_HOME", None)
        self.codes = self.home / "codes"
        (self.codes / "widgets").mkdir(parents=True)
        shutil_copy(self.work / "specs", self.codes / "widgets" / "specs")

    def seed(self, *extra):
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            tr.cmd_seed(tr.p.parse_args(["seed", f"--project={self.slug}", f"--codes-dir={self.codes}", *extra]))
        return buf.getvalue()

    def test_seed_marks_confidence_and_writes_only_the_index(self):
        self.agent("a1", "T001, T002: 001-widget core", 300)
        self.agent("a2", "Do the 001 follow-ups", 200)
        self.agent("a3", "Survey the repo", 100)
        before = {str(p): p.read_text() for p in (self.codes / "widgets" / "specs").rglob("*.md")}
        out = self.seed()
        recs = tr.read_history(str(self.ai_home / "token-history.jsonl"))
        self.assertEqual(sorted((r["confidence"], r["source"]) for r in recs),
                         [("agent", "seed"), ("exact", "seed"), ("feature", "seed")])
        self.assertIn("widgets | 1 | 1 | 3 | 1 exact, 1 feature, 1 agent", out)
        self.assertEqual(before, {str(p): p.read_text() for p in (self.codes / "widgets" / "specs").rglob("*.md")})
        self.assertFalse(Path("docs/product/token-history.jsonl").exists())
        self.seed()  # idempotent
        self.assertEqual(len(tr.read_history(str(self.ai_home / "token-history.jsonl"))), 3)

    def test_index_merges_repo_histories_keeps_seeds_and_drops_superseded_ones(self):
        self.agent("a1", "T001, T002: 001-widget core", 300)
        self.agent("a3", "Survey the repo", 100)
        self.seed()
        self.run_tasks()  # the repo now measures a1 and a3 itself
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            tr.cmd_index(tr.p.parse_args(["index", f"--repo={self.work}", f"--index={self.ai_home / 'token-history.jsonl'}"]))
        recs = tr.read_history(str(self.ai_home / "token-history.jsonl"))
        self.assertEqual({r["source"] for r in recs}, {"measured"})
        self.assertEqual(len(recs), 2)


def shutil_copy(src, dst):
    import shutil
    shutil.copytree(src, dst)


if __name__ == "__main__":
    unittest.main()
