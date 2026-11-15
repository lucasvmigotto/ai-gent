#!/usr/bin/env python3
"""Parity gate for `ai-gent tokens` (spec 035, decision D10).

The cases in rust/fixtures/tokens/*.json are the Python tests' scenarios turned into data
(plugins/agents/scripts/test_token_report.py): a file tree, the commands to run, and what the Python
tool printed and wrote. This script is the oracle runner; `cargo test -p ai-gent --test parity_tokens`
is the Rust runner over the same files.

  scripts/test-parity-tokens.py              run every case through token_report.py (the oracle)
  scripts/test-parity-tokens.py --bin PATH   run every case through the ai-gent binary instead
  scripts/test-parity-tokens.py --bless      regenerate the fixtures from the Python tool, then review the diff
  scripts/test-parity-tokens.py --self-test  the harness itself, with one trivial case

--bless refuses to run once plugins/agents/scripts/token_report.py is gone (it is deleted at the 3.0.0
cutover; the fixtures then stay and only the Rust runner remains).

Time: file contents may hold `@NOW-<seconds>@`, replaced by the run's clock as `%Y-%m-%dT%H:%M:%SZ`, so
the budget cases ("5 minutes ago") are the same on every day. Output is normalised before comparing:
the temp root becomes `<ROOT>` and `Generated YYYY-MM-DD HH:MM` becomes `Generated <TS>` (Python prints
local time, the binary UTC).
"""
import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from datetime import datetime, timedelta, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PY_TOOL = ROOT / "plugins" / "agents" / "scripts" / "token_report.py"
FIXTURES = ROOT / "rust" / "fixtures" / "tokens"
DEFAULT_BIN = ROOT / "rust" / "target" / "debug" / "ai-gent"
SLUG = "-tmp-widgets"

USAGE = {"input_tokens": 1, "cache_creation_input_tokens": 10, "cache_read_input_tokens": 100, "output_tokens": 2}


# ----------------------------------------------------------------- scenario DSL

def usage(out, cache_read=1000, cache_write=0):
    return {"input_tokens": 0, "cache_creation_input_tokens": cache_write, "cache_read_input_tokens": cache_read,
            "output_tokens": out}


def msg(mid, model, u, ts):
    return json.dumps({"type": "assistant", "timestamp": ts, "message": {"id": mid, "model": model, "usage": u}})


def ago(seconds):
    return f"@NOW-{seconds}@"


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

**Route**: Executor `builder` · Effort `high` · Tags: extend, backend, protocol, rust, tokio, known-pattern, integration, M · Tokens: projected ~400k | used — · Dispatch description: `T001, T002: 001-widget core`

## Card `001-polish`: T003

**Route**: Executor `builder` · Effort `low` · Tokens: projected ~100k | used — · Dispatch description: `T003: 001-widget polish`
"""
LEDGER = """# Ledger
| Wave | Feature | Card | State |
|---|---|---|---|
| 0 | 001-widget | `001-core` | done |
| 0 | 001-widget | `001-polish` | planned |
"""
Q = {"kind": "extend", "layer": "backend", "surface": "protocol", "lang": "rust", "novelty": "known-pattern",
     "verification": "integration", "size": "M"}


class Scn:
    """A scenario: files (relative to the temp root: home/, work/, ai-home/) and the runs."""

    def __init__(self, base="tasks"):
        self.files, self.runs = {}, []
        if base == "tasks":
            w = "work/"
            self.files[w + "specs/001-widget/tasks.md"] = TASKS_MD
            self.files[w + "specs/001-widget/cards.md"] = CARDS_MD
            self.files[w + "specs/002-gadget/tasks.md"] = "- [ ] T001 gadget — Executor: builder\n"
            self.files[w + ".ai-gent/orchestrate-status.md"] = LEDGER
        self.slug = SLUG

    def root(self, cfg=".claude"):
        return f"home/{cfg}/projects/{self.slug}"

    def agent(self, aid, desc, out, prompt="", ts="2026-10-20T10:00:00Z", cfg=".claude", calls=1, model="claude-sonnet-5-5"):
        d = f"{self.root(cfg)}/s1/subagents/agent-{aid}"
        lines = [json.dumps({"type": "user", "message": {"role": "user", "content": prompt}})]
        lines += [msg(f"{aid}-{i}", model, usage(out), ts) for i in range(calls)]
        self.files[d + ".jsonl"] = "\n".join(lines)
        self.files[d + ".meta.json"] = json.dumps({"description": desc})

    def main(self, out, ts="2026-10-20T09:00:00Z", cfg=".claude"):
        self.files[f"{self.root(cfg)}/s1.jsonl"] = msg("c1", "claude-opus-5-5", usage(out), ts)

    def put(self, rel, text):
        self.files[rel] = text

    def run(self, *argv, stdin=None):
        self.runs.append({"argv": list(argv), "stdin": stdin})
        return self

    def tasks(self, *extra):
        return self.run("tasks", f"--project={self.slug}", "--specs-dir=specs", "--snapshots=none.json", *extra)

    def budget(self, *extra, yaml="session_billable: 1M\nweek_billable: 5M\n"):
        self.files["work/budget.yaml"] = yaml
        return self.run("budget", f"--project={self.slug}", "--budget=budget.yaml", "--specs-dir=specs", *extra)


def rec(i, used, conf="exact", model="sonnet", effort="medium", layer="backend", tasks=("T001", "T002"),
        source="measured", agents=None, project="p", **extra):
    r = {"id": f"{project}:{i}", "source": source, "confidence": conf, "project": project, "feature": "001-x",
         "card": f"c{i}", "tasks": list(tasks), "layer": layer, "effort": effort, "executor_model": model,
         "used": used, "date": "2026-10-2%d" % (i % 9), "agents": agents or [f"ag{i}"]}
    r.update(extra)
    return r


def trec(i, used, tags=None, **kw):
    r = rec(i, used, **{k: v for k, v in kw.items() if k != "role"})
    r["tags"] = {**Q, **(tags or {})}
    r["executor_role"] = kw.get("role", "builder")
    return r


def jsonl(recs):
    return "".join(json.dumps(r, ensure_ascii=False) + "\n" for r in recs)


def history_scn(recs, *argv, base="tasks"):
    s = Scn(base)
    s.put("work/h.jsonl", jsonl(recs))
    return s.run(*argv, "--history=h.jsonl")


# ---------------------------------------------------------------------- cases

def cases():
    out = {}

    def add(name, derived, scn):
        out[name] = (derived, scn)

    # --- report / snapshot (TokenReportTest)
    def base_report():
        s = Scn(base=None)
        r = f"home/.claude/projects/{SLUG}"
        s.put(f"{r}/sess1.jsonl", msg("m1", "claude-opus-5-5", USAGE, "2026-10-20T10:00:00Z") + "\n" +
              msg("m1", "claude-opus-5-5", USAGE, "2026-10-20T10:00:00Z") + "\n")
        a1 = [json.dumps({"type": "user", "message": {"role": "user", "content": "Goal: complete T001, T002 of specs/001-widget/tasks.md"}}),
              json.dumps({"type": "assistant", "timestamp": "2026-10-20T10:01:00Z", "message": {"id": "a1m1", "model": "claude-sonnet-5-5", "usage": USAGE,
                          "content": [{"type": "tool_use", "id": "tu1", "name": "Write", "input": {"file_path": "src/a.py", "content": "a\nb\nc"}}]}})]
        s.put(f"{r}/sess1/subagents/agent-a1.jsonl", "\n".join(a1))
        s.put(f"{r}/sess1/subagents/agent-a1.meta.json", json.dumps({"description": "T001, T002: build widget"}))
        s.put(f"{r}/sess1/subagents/agent-a2.jsonl", msg("a2m1", "claude-sonnet-5-5", USAGE, "2026-10-20T10:02:00Z"))
        s.put(f"{r}/sess1/subagents/agent-a2.meta.json", json.dumps({"description": "Survey unrelated repo"}))
        s.put("work/specs/001-widget/tasks.md", "- [ ] T001 [US1] build the widget\n- [ ] T002 [US1] test the widget\n")
        s.slug = SLUG
        return s

    def report_args(s, *a):
        return s.run("report", "--project=-tmp-fake-project", *a)

    def fake():
        s = Scn(base=None)
        b = base_report()
        for k, v in b.files.items():
            s.files[k.replace(SLUG, "-tmp-fake-project")] = v
        return s

    add("report-rollup", "TokenReportTest::test_report_writes_project_wide_rollup", report_args(fake(), "--out=rollup.md"))
    s = report_args(fake(), "--summary", "--top=1")
    report_args(s, "--out=top.md", "--top=1")
    add("report-summary-and-top", "TokenReportTest::test_report_summary_is_a_few_lines_and_top_trims_the_file", s)
    add("report-needs-out-or-summary", "TokenReportTest::test_report_without_out_or_summary_is_rejected", report_args(fake()))
    s = fake()
    big = usage(2, cache_write=60_000)
    s.put("home/.claude/projects/-tmp-fake-project/sess1.jsonl", "\n".join([
        msg("i1", "claude-opus-5-5", usage(2), "2026-10-20T10:00:00Z"), msg("i2", "claude-opus-5-5", big, "2026-10-20T10:02:00Z"),
        msg("i3", "claude-opus-5-5", big, "2026-10-20T10:20:00Z"), msg("i4", "claude-opus-5-5", big, "2026-10-20T10:21:00Z")]))
    add("report-summary-idle-rebuilds", "TokenReportTest::test_idle_rebuilds_counts_cache_writes_after_gaps_over_five_minutes", report_args(s, "--summary"))
    # Not a case: token_report.py render() crashes (TypeError) when stage markers exist, because the loop
    # variable `top` of the By-stage table shadows the --top argument. The Rust renderer does not; its
    # By-stage output is covered by a unit test (tokens::report::tests) and listed in the PR as a disagreement.
    s = Scn(base=None)
    s.run("snapshot", "--label=manual one", "--at=2026-10-20T00:00:00Z", "--snapshots=snaps.json")
    s.run("backfill-git", "--repo=-", "--snapshots=snaps.json",
          stdin="2026-10-19T00:00:00+00:00|chore: initial commit\n2026-10-20T00:00:00+00:00|manual one\n2026-10-21T00:00:00+02:00|Merge branch 'feat/x' into dev\n")
    add("snapshot-and-backfill", "TokenReportTest::test_snapshot_and_backfill_merge_without_duplicates", s)

    # --- tasks (TasksTest, TasksJsonTest, EventsTest, LegacyTest)
    s = Scn()
    for aid, desc, o, p in [("a1", "T001, T002: 001-widget core", 100, ""), ("a2", "Review 001-polish (T003)", 50, ""),
                            ("a3", "T001: 002-gadget build", 40, ""), ("a4", "T001 something", 30, ""),
                            ("a5", "Do the 001 follow-ups", 20, ""), ("a6", "Survey the repo", 10, "no specs here"),
                            ("a7", "Implement it", 10, "Read specs/002-gadget/tasks.md and specs/002-gadget/spec.md"),
                            ("a8", "T001–T003: 001-widget build", 10, "")]:
        s.agent(aid, desc, o, prompt=p)
    s.tasks("--no-legacy")
    add("tasks-attribution-levels", "TasksTest::test_attribution_levels", s)

    s = Scn()
    s.agent("a1", "T001, T002: 001-widget core", 391_000)
    s.agent("a2", "Review 001-core", 9_000, ts="2026-10-20T11:00:00Z")
    s.agent("a3", "Survey the repo", 5_000)
    s.main(100_000)
    s.tasks()
    add("tasks-writes-lines-tracker-history-ledger", "TasksTest::test_tasks_writes_lines_tracker_history_ledger", s)

    s = Scn()
    s.agent("a1", "T001, T002: 001-widget core", 100_000)
    s.main(10_000)
    s.tasks().tasks()
    add("tasks-rerun-is-idempotent", "TasksTest::test_rerun_updates_numbers_without_duplicating (first half)", s)
    s = Scn()
    s.agent("a1", "T001, T002: 001-widget core", 250_000)
    s.main(10_000)
    s.tasks()
    s.put("work/specs/001-widget/tasks.md", TASKS_MD.replace("T001 [US1] build the widget in `src/a.py` — Executor: builder · Reviewer: thinker · Effort: high",
                                                            "T001 [US1] build the widget in `src/a.py` — Executor: builder · Reviewer: thinker · Effort: high · Tokens: projected ~1k | used 100k"))
    s.tasks()
    add("tasks-rerun-replaces-the-tag", "TasksTest::test_rerun_updates_numbers_without_duplicating (second half)", s)

    s = Scn()
    s.agent("a1", "T003: 001-widget polish", 100, calls=2)
    s.agent("a1", "T003: 001-widget polish", 100, calls=3, cfg=".claude-b", ts="2026-10-21T10:00:00Z")
    s.main(1)
    s.tasks()
    add("tasks-resumed-agent-across-config-dirs", "TasksTest::test_resumed_agent_counts_once_across_config_dirs", s)

    s = Scn()
    s.agent("a1", "T001: 002-gadget build", 300, ts="2026-10-20T10:00:00Z")
    s.agent("a2", "T003: 001-widget polish", 100, ts="2026-10-20T10:30:00Z")
    s.agent("a3", "T003: 001-widget polish", 50, ts="2026-10-21T10:30:00Z")
    s.put(f"{s.root()}/s1.jsonl", "\n".join([msg("c1", "claude-opus-5-5", usage(80), "2026-10-20T09:00:00Z"),
                                              msg("c2", "claude-opus-5-5", usage(500), "2026-10-22T09:00:00Z")]))
    s.put("work/snaps.json", json.dumps([{"at": "2026-10-20T12:00:00", "label": "landed"}, {"at": "2026-10-21T12:00:00", "label": "landed 2"}]))
    s.run("tasks", f"--project={SLUG}", "--specs-dir=specs", "--snapshots=snaps.json", "--no-legacy")
    add("tasks-coordinator-split-by-window-weights", "TasksTest::test_coordinator_split_by_window_weights", s)

    s = Scn()
    s.agent("a1", "T001, T002: 001-widget core", 100)
    s.tasks("--dry-run")
    add("tasks-dry-run-writes-nothing", "TasksTest::test_dry_run_writes_nothing", s)

    def json_base():
        s = Scn()
        s.agent("a1", "T001, T002: 001-widget core", 391_000, ts="2026-10-20T10:00:00Z", calls=2)
        s.agent("a2", "Review 001-core", 9_000, ts="2026-10-20T11:00:00Z")
        s.agent("a3", "Survey the repo", 5_000)
        return s

    add("tasks-json-shape", "TasksJsonTest::test_json_shape_and_z_timestamps", json_base().tasks("--dry-run", "--json"))
    add("tasks-json-writes-nothing", "TasksJsonTest::test_json_writes_nothing", json_base().tasks("--dry-run", "--json"))
    add("tasks-json-needs-dry-run", "TasksJsonTest::test_json_without_dry_run_is_rejected", json_base().run("tasks", f"--project={SLUG}", "--json"))
    s = json_base().tasks("--dry-run").tasks("--dry-run", "--summary", "--top=1").tasks("--dry-run", "--summary", "--top=0")
    add("tasks-summary-and-top", "TasksJsonTest::test_tasks_summary_and_top_shorten_the_text", s)
    add("tasks-history-carries-model-id", "TasksJsonTest::test_history_record_carries_the_full_model_id", json_base().tasks())

    s = Scn()
    stop = json.dumps({"type": "assistant", "timestamp": "2026-10-20T10:05:00Z",
                       "message": {"id": "syn", "model": "<synthetic>", "usage": usage(0), "content": []}})
    d = f"{s.root()}/s1/subagents/agent-a1"
    s.put(d + ".jsonl", "\n".join([json.dumps({"type": "user", "message": {"role": "user", "content": "p"}}),
                                   msg("a1-0", "claude-sonnet-5-5", usage(1000), "2026-10-20T10:00:00Z"), stop,
                                   msg("a1-1", "claude-sonnet-5-5", usage(300), "2026-10-20T11:00:00Z"),
                                   msg("a1-2", "claude-sonnet-5-5", usage(100), "2026-10-20T11:01:00Z")]))
    s.put(d + ".meta.json", json.dumps({"description": "T003: 001-widget polish"}))
    s.agent("a2", "Fix polish findings (T003)", 200, ts="2026-10-20T12:00:00Z")
    s.agent("a3", "Review 001-polish", 50, ts="2026-10-20T13:00:00Z")
    s.main(1)
    s.tasks()
    add("events-resume-fix-rounds-stay-out-of-clean-cost", "EventsTest::test_resume_after_a_limit_stop_and_fix_rounds_stay_out_of_clean_cost", s)

    s = Scn()
    s.agent("a1", "T001, T002: build widget", 100)
    s.agent("a2", "Survey unrelated repo", 50)
    s.main(1)
    s.tasks()
    add("legacy-tokens-md-and-unmapped", "LegacyTest::test_tasks_still_writes_tokens_md_and_the_unmapped_section_unless_no_legacy (first half)", s)
    s = Scn()
    s.agent("a1", "T001, T002: build widget", 100)
    s.main(1)
    s.tasks("--no-legacy")
    add("legacy-no-legacy", "LegacyTest::test_tasks_still_writes_tokens_md_and_the_unmapped_section_unless_no_legacy (second half)", s)

    # --- estimate (EstimateTest, TagEstimateTest, TagsTest, LegacyTest)
    five = [rec(i, 100 * (i + 1) * 1000) for i in range(5)]
    add("estimate-history-text", "EstimateTest::test_estimate_command_prints_projection_basis_and_card_line",
        history_scn(five, "estimate", "--role=builder", "--effort=medium", "--layer=backend", "--tasks=2", base=None))
    add("estimate-history-json", "EstimateTest::test_history_takes_over_with_median_p75_and_widens_the_match",
        history_scn(five, "estimate", "--role=builder", "--effort=medium", "--layer=backend", "--tasks=2", "--json", base=None))
    add("estimate-starting-table", "EstimateTest::test_few_records_fall_back_to_the_starting_table",
        history_scn([rec(i, 999) for i in range(4)], "estimate", "--model=sonnet", "--effort=medium", "--layer=backend", "--tasks=4", "--rollup=none.md", base=None))
    add("estimate-starting-table-opus-json", "EstimateTest::test_few_records_fall_back_to_the_starting_table (opus)",
        history_scn([], "estimate", "--model=opus", "--effort=high", "--tasks=9", "--json", base=None))
    recs = [rec(i, v, layer="backend" if i < 3 else "qa") for i, v in enumerate([100, 200, 300, 400, 500])]
    add("estimate-widens-to-effort-model-size", "EstimateTest::test_history_takes_over_with_median_p75_and_widens_the_match (mixed)",
        history_scn(recs, "estimate", "--role=builder", "--layer=backend", "--tasks=2", "--json", base=None))
    recs = [rec(i, 1000 + i) for i in range(2)] + [rec(i + 10, 100 + i, conf="agent") for i in range(3)]
    add("estimate-confidence-weights", "EstimateTest::test_confidence_weights_pull_the_median_to_exact_records",
        history_scn(recs, "estimate", "--role=builder", "--layer=backend", "--tasks=2", base=None))
    tags_arg = "--tags=extend, backend, protocol, rust, known-pattern, integration, M"
    add("estimate-tags-text", "TagEstimateTest::test_command_prints_level_confidence_and_context",
        history_scn([trec(i, 100_000 * (i + 1)) for i in range(5)], "estimate", tags_arg, base=None))
    add("estimate-tags-json", "TagEstimateTest::test_full_match_is_high_confidence_with_median_and_p75",
        history_scn([trec(i, v) for i, v in enumerate([100, 200, 300, 400, 500])], "estimate", tags_arg, "--json", base=None))
    add("estimate-tags-backoff-size", "TagEstimateTest::test_backoff_drops_size_then_stack_then_verification_then_layer",
        history_scn([trec(i, 100 * (i + 1), tags={"size": "S"}) for i in range(5)], "estimate", tags_arg, "--json", base=None))
    add("estimate-tags-backoff-layer", "TagEstimateTest::test_backoff_drops_size_then_stack_then_verification_then_layer (layer)",
        history_scn([trec(i, 100, tags={"layer": "tui", "verification": "unit"}) for i in range(5)], "estimate", tags_arg, "--json", base=None))
    add("estimate-tags-too-few-fall-back", "TagEstimateTest::test_too_few_matches_even_at_the_last_level_fall_back",
        history_scn([trec(i, 100, tags={"kind": "fix", "lang": "go"}) for i in range(9)], "estimate", tags_arg, base=None))
    add("estimate-tags-too-few-opus", "TagEstimateTest::test_too_few_matches_even_at_the_last_level_fall_back (opus)",
        history_scn([trec(i, 100, tags={"kind": "fix", "lang": "go"}) for i in range(9)], "estimate", "--model=opus", tags_arg, "--json", base=None))
    st = [trec(i, 100, tags={"stack": ["react"]}) for i in range(3)] + [trec(i + 9, 900, tags={"stack": ["tokio", "clap"]}) for i in range(3)]
    add("estimate-tags-stack-overlap", "TagEstimateTest::test_stack_overlap_adds_weight_without_excluding",
        history_scn(st, "estimate", "--tags=extend, backend, protocol, rust, tokio, clap, known-pattern, integration, M", "--json", base=None))
    add("estimate-tags-inferred-half", "TagEstimateTest::test_inferred_tags_weigh_half",
        history_scn([trec(i, 100) for i in range(2)] + [dict(trec(i + 9, 900), tags_source="inferred") for i in range(4)], "estimate", tags_arg, "--json", base=None))
    peers = []
    for g in range(5):
        tg = {"surface": "storage", "kind": ["extend", "fix", "refactor", "docs", "ci"][g]}
        peers += [trec(g, 100, tags=tg), dict(trec(g + 20, 200, tags=tg), executor_model="opus", executor_role="thinker")]
    add("estimate-tags-context-learned", "TagEstimateTest::test_context_factor_is_learned_from_peer_groups_and_applied",
        history_scn(peers + [trec(40 + i, 100) for i in range(5)], "estimate", "--role=thinker", tags_arg, "--json", base=None))
    add("estimate-tags-context-prior", "TagEstimateTest::test_context_factor_is_learned_from_peer_groups_and_applied (prior)",
        history_scn([trec(i, 100) for i in range(5)], "estimate", "--role=thinker", tags_arg, base=None))
    add("estimate-tags-bias", "TagEstimateTest::test_bias_correction_needs_five_records_per_tag",
        history_scn([dict(trec(i, 200), projected=100) for i in range(5)], "estimate", tags_arg, base=None))
    add("estimate-tags-events-out-of-cost", "TagEstimateTest::test_events_stay_out_of_task_cost",
        history_scn([dict(trec(i, 500), used_clean=100, events={"fix_rounds": 2}) for i in range(5)], "estimate", tags_arg, "--json", base=None))
    add("estimate-seed-dedup", "EstimateTest::test_load_history_drops_a_seed_the_repo_already_measured",
        history_scn([rec(1, 10, agents=["x"]), rec(2, 20, source="seed", agents=["x"]), rec(3, 30, source="seed", agents=["y"])],
                    "estimate", "--role=builder", "--json", base=None))

    s = Scn()
    s.agent("a1", "T001, T002: build widget", 1000)
    s.agent("a2", "T003: polish", 3000)
    s.main(1)
    s.run("report", f"--project={SLUG}", "--out=rollup.md")
    for extra in (["--role=builder"], ["--label=T003"], ["--label=Nope"], ["--role=scout"]):
        s.run("estimate", "--rollup=rollup.md", "--history=none.jsonl", *extra)
    add("estimate-rollup-fallback", "LegacyTest::test_estimate_falls_back_to_the_rollup_when_history_is_empty", s)

    # --- index / seed (IndexSeedTest)
    def seed_base():
        s = Scn()
        for rel in ("specs/001-widget/tasks.md", "specs/001-widget/cards.md", "specs/002-gadget/tasks.md"):
            s.put("codes/widgets/" + rel, s.files["work/" + rel])
        return s

    s = seed_base()
    s.agent("a1", "T001, T002: 001-widget core", 300)
    s.agent("a2", "Do the 001 follow-ups", 200)
    s.agent("a3", "Survey the repo", 100)
    s.run("seed", f"--project={SLUG}", "--codes-dir=<ROOT>/codes", "--index=<ROOT>/ai-home/token-history.jsonl")
    s.run("seed", f"--project={SLUG}", "--codes-dir=<ROOT>/codes", "--index=<ROOT>/ai-home/token-history.jsonl")
    add("seed-marks-confidence-and-is-idempotent", "IndexSeedTest::test_seed_marks_confidence_and_writes_only_the_index", s)
    s = seed_base()
    s.put("codes/widgets/Cargo.toml", '[dependencies]\ntokio = "1"\nreact-dom = "1"\n')
    d = f"{s.root()}/s1/subagents/agent-a1"
    block = {"type": "tool_use", "id": "tu1", "name": "Write", "input": {"file_path": "src/a.py", "content": "a\nb\nc"}}
    edit = json.dumps({"type": "assistant", "timestamp": "2026-10-20T10:01:00Z", "message": {"id": "a1m", "model": "claude-sonnet-5-5", "usage": USAGE, "content": [block]}})
    s.put(d + ".jsonl", "\n".join([json.dumps({"type": "user", "message": {"role": "user", "content": "p"}}), edit]))
    s.put(d + ".meta.json", json.dumps({"description": "Fix socket protocol findings"}))
    s.run("seed", f"--project={SLUG}", "--codes-dir=<ROOT>/codes", "--index=<ROOT>/ai-home/token-history.jsonl", "--dry-run")
    s.run("seed", f"--project={SLUG}", "--codes-dir=<ROOT>/codes", "--index=<ROOT>/ai-home/token-history.jsonl")
    add("seed-infers-tags-from-manifests-and-files", "IndexSeedTest::test_seed_infers_coarse_low_confidence_tags_from_manifests_and_touched_files", s)
    s = seed_base()
    s.put("codes/widgets/Cargo.toml", '[dependencies]\ntokio = "1"\n')
    s.put(f"{s.root()}/s1/subagents/agent-a1.jsonl", "\n".join([json.dumps({"type": "user", "message": {"role": "user", "content": "p"}}),
                                                                 edit.replace("src/a.py", "src/a.rs")]))
    s.put(f"{s.root()}/s1/subagents/agent-a1.meta.json", json.dumps({"description": "Fix socket protocol findings"}))
    s.run("seed", f"--project={SLUG}", "--codes-dir=<ROOT>/codes", "--index=<ROOT>/ai-home/token-history.jsonl")
    add("seed-rust-stack", "IndexSeedTest::test_seed_infers_coarse_low_confidence_tags_from_manifests_and_touched_files (rust)", s)
    s = Scn()
    s.agent("a1", "T001, T002: 001-widget core", 300)
    s.agent("a3", "Survey the repo", 100)
    for rel in ("specs/001-widget/tasks.md", "specs/001-widget/cards.md", "specs/002-gadget/tasks.md"):
        s.put("codes/widgets/" + rel, s.files["work/" + rel])
    s.run("seed", f"--project={SLUG}", "--codes-dir=<ROOT>/codes", "--index=<ROOT>/ai-home/token-history.jsonl")
    s.tasks()
    s.run("index", "--repo=<ROOT>/work", "--index=<ROOT>/ai-home/token-history.jsonl")
    add("index-merges-and-drops-superseded-seeds", "IndexSeedTest::test_index_merges_repo_histories_keeps_seeds_and_drops_superseded_ones", s)

    # --- budget (UsageSinceTest, BudgetTest)
    def bs():
        s = Scn()
        s.agent("a1", "T001: 002-gadget build", 400_000, ts=ago(300))
        s.main(100_000, ts=ago(600))
        return s

    s = bs()
    s.budget("--projected=sonnet:200k")
    s.budget("--projected=sonnet:200k,sonnet:200k,sonnet:200k")
    s.budget("--projected=opus:900k")
    add("budget-verdicts-proceed-shrink-stop", "BudgetTest::test_verdicts_proceed_shrink_stop_after_what_the_session_used", s)
    s = bs()
    a = ("--projected=sonnet:200k,sonnet:200k,sonnet:200k",)
    s.budget(*a, "--json")
    s.budget(*a)
    s.budget("--projected=opus:900k", "--json")
    add("budget-json-matches-text", "BudgetTest::test_json_is_one_object_with_the_same_verdict_and_nothing_else", s)

    def steady(s, n=4, out=50_000, spacing=5):
        s.put(f"{s.root()}/s1.jsonl", "\n".join(msg(f"r{i}", "claude-sonnet-5-5", usage(out), ago(60 * (1 + i * spacing))) for i in range(n)))

    s = Scn()
    steady(s)
    s.budget("--projected=sonnet:10k", "--json")
    s.budget("--projected=sonnet:10k")
    add("budget-rate-and-eta", "BudgetTest::test_budget_reports_the_rate_and_each_caps_eta", s)
    s = Scn()
    s.put(f"{s.root()}/s1.jsonl", msg("old", "claude-sonnet-5-5", usage(100), ago(7200)))
    s.budget("--projected=sonnet:10k", "--json")
    s.budget("--projected=sonnet:10k")
    add("budget-idle", "BudgetTest::test_budget_idle_has_a_zero_rate_and_no_eta", s)
    s = Scn()
    steady(s, n=2)
    s.budget("--projected=sonnet:10k", "--json")
    s.budget("--projected=sonnet:10k")
    add("budget-too-few-messages", "BudgetTest::test_budget_below_the_minimum_messages_has_no_rate_and_no_eta", s)
    s = Scn()
    steady(s)
    s.budget("--projected=sonnet:10k", yaml="max_agents: 2\n")
    s.budget("--projected=sonnet:10k", "--json", yaml="max_agents: 2\n")
    add("budget-without-caps-prints-rate", "BudgetTest::test_budget_without_caps_still_prints_the_rate", s)
    s = Scn()
    steady(s, n=4, spacing=5)
    s.budget("--projected=sonnet:10k", "--json", yaml="session_billable: 1M\nweek_billable: 5M\nrate_window_minutes: 12\n")
    add("budget-rate-window-from-yaml", "BudgetTest::test_budget_rate_window_comes_from_budget_yaml", s)
    s = Scn()
    steady(s)
    s.budget("--projected=sonnet:10k", "--json", yaml="session_billable: 1M\nweek_billable: 5M\nsession_window_hours: 2\n")
    s.budget("--projected=sonnet:10k", yaml="session_billable: 1M\nweek_billable: 5M\nsession_window_hours: 2\n")
    add("budget-session-window", "BudgetTest::test_budget_session_window_reads_the_week_once", s)
    s = Scn()
    s.agent("a1", "T001: 002-gadget build", 4_900_000, ts=ago(3 * 86400))
    s.main(1, ts=ago(60))
    s.budget("--cards=001-core,001-polish")
    add("budget-cards-from-cards-md-and-week-cap", "BudgetTest::test_cards_come_from_cards_md_and_the_week_cap_counts_too", s)
    s = Scn()
    s.budget("--projected=300k", yaml="max_agents: 3\n")
    add("budget-no-caps-proceeds", "BudgetTest::test_no_caps_means_proceed_with_a_hint", s)
    s = Scn()
    s.budget("--projected=opus:1k,opus:1k")
    add("budget-two-opus-queued", "BudgetTest::test_two_opus_cards_are_queued_beyond_the_cap", s)
    s = Scn()
    s.budget("--projected=opus:1k,opus:1k,sonnet:1k", yaml="max_opus: 2\nmax_agents: 3\n")
    s.put("work/h.jsonl", jsonl([trec(i, 100_000 * (i + 1)) for i in range(5)]))
    s.put("work/specs/001-widget/cards.md", CARDS_MD.replace("Tokens: projected ~400k | used — · ", "").replace(
        "Tags: extend, backend, protocol, rust, tokio, known-pattern, integration, M", "Tags: extend, backend, protocol, rust, known-pattern, integration, M"))
    s.budget("--cards=001-core", "--history=h.jsonl")
    s.budget("--cards=001-core", "--history=h.jsonl", "--json")
    add("budget-parallel-opus-and-tag-estimates", "BudgetTest::test_parallel_opus_adds_event_risk_and_tags_drive_the_card_projection", s)
    s = Scn()
    s.run("budget", f"--project={SLUG}", "--budget=none.yaml")
    add("budget-no-cards-fails", "cmd_budget: sys.exit when no cards are named", s)
    s = bs()
    s.put(f"{s.root()}/s9.jsonl", msg("m2", "claude-sonnet-5-5", usage(200), ago(300)))
    s.budget("--projected=sonnet:10k", "--session=s9", "--json")
    add("budget-dedupes-by-message-id-across-sessions", "UsageSinceTest::test_usage_since_sums_billable_after_the_cutoff_and_dedupes_by_id", s)
    return out


def trivial():
    s = Scn(base=None)
    s.put("work/snaps.json", "[]")
    s.run("snapshot", "--label=one", "--at=2026-10-20T00:00:00Z", "--snapshots=snaps.json")
    return {"self-test": ("harness self-test", s)}


# ----------------------------------------------------------------- materialise

def clock():
    return datetime.now(timezone.utc)


def expand(text, now):
    return re.sub(r"@NOW-(\d+)@", lambda m: (now - timedelta(seconds=int(m.group(1)))).strftime("%Y-%m-%dT%H:%M:%SZ"), text)


def normalise(text, tmp):
    text = text.replace(str(tmp), "<ROOT>")
    return re.sub(r"Generated \d{4}-\d{2}-\d{2} \d{2}:\d{2}", "Generated <TS>", text)


def materialise(case, tmp, now):
    for rel, text in case["files"].items():
        p = tmp / rel
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(expand(text.replace("<ROOT>", str(tmp)), now), encoding="utf-8")
    (tmp / "work").mkdir(exist_ok=True)
    (tmp / "ai-home").mkdir(exist_ok=True)


def tree(tmp):
    return {str(p.relative_to(tmp)): p.read_bytes() for p in sorted(tmp.rglob("*")) if p.is_file()}


def execute(case, tool_cmd, subcmd_prefix):
    """Run a case: returns (runs=[{argv, exit, stdout}], files=changed/new text files)."""
    tmp = Path(tempfile.mkdtemp(prefix="parity-tokens-"))
    try:
        now = clock()
        materialise(case, tmp, now)
        before = tree(tmp)
        env = {k: v for k, v in os.environ.items() if k not in ("CLAUDE_CONFIG_DIR", "AI_GENT_HOME", "NO_COLOR")}
        env.update(HOME=str(tmp / "home"), AI_GENT_HOME=str(tmp / "ai-home"), NO_COLOR="1", TZ="UTC")
        env.pop("USERPROFILE", None)
        runs = []
        for run in case["runs"]:
            argv = [a.replace("<ROOT>", str(tmp)) for a in run["argv"]]
            proc = subprocess.run([*tool_cmd, *subcmd_prefix, *argv], cwd=tmp / "work", env=env, input=run.get("stdin"),
                                  capture_output=True, text=True, timeout=120)
            runs.append({"argv": run["argv"], "stdin": run.get("stdin"), "exit": proc.returncode, "stdout": normalise(proc.stdout, tmp)})
        after = tree(tmp)
        files = {}
        for rel, data in after.items():
            if before.get(rel) != data:
                try:
                    files[rel] = normalise(data.decode("utf-8"), tmp)
                except UnicodeDecodeError:
                    files[rel] = "<binary>"
        return runs, files
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


def load_case(path):
    return json.loads(path.read_text(encoding="utf-8"))


def compare(name, case, runs, files):
    problems = []
    for i, (want, got) in enumerate(zip(case["runs"], runs)):
        if want.get("exit") != got["exit"]:
            problems.append(f"{name} run {i} ({' '.join(want['argv'][:2])}): exit {got['exit']}, expected {want.get('exit')}")
        if want.get("stdout") != got["stdout"]:
            problems.append(f"{name} run {i} ({' '.join(want['argv'][:2])}): stdout differs\n--- expected\n{want.get('stdout')}\n--- got\n{got['stdout']}")
    if case["expect_files"] != files:
        for rel in sorted(set(case["expect_files"]) | set(files)):
            if case["expect_files"].get(rel) != files.get(rel):
                problems.append(f"{name}: file {rel} differs\n--- expected\n{case['expect_files'].get(rel)}\n--- got\n{files.get(rel)}")
    return problems


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--bin", help="run the cases through this ai-gent binary instead of the Python tool")
    ap.add_argument("--bless", action="store_true")
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("--only", help="substring of a case name")
    a = ap.parse_args()
    if a.bin:
        tool, prefix = [str(Path(a.bin).resolve())], ["tokens"]
    else:
        tool, prefix = [sys.executable, str(PY_TOOL)], []
    if a.self_test:
        name, (derived, scn) = next(iter(trivial().items()))
        case = {"name": name, "derived_from": derived, "files": scn.files, "runs": scn.runs}
        runs, files = execute(case, [sys.executable, str(PY_TOOL)], [])
        ok = runs[0]["exit"] == 0 and runs[0]["stdout"].startswith("snapshot 'one' at 2026-10-20T00:00:00 (1 total)") \
            and "work/snaps.json" in files
        print("self-test ok" if ok else f"self-test FAILED: {runs} {list(files)}")
        return 0 if ok else 1
    if a.bless:
        if not PY_TOOL.exists():
            print("refusing to bless: plugins/agents/scripts/token_report.py is gone (the fixtures are the oracle now)")
            return 1
        FIXTURES.mkdir(parents=True, exist_ok=True)
        for old in FIXTURES.glob("*.json"):
            old.unlink()
        for name, (derived, scn) in cases().items():
            case = {"name": name, "derived_from": derived, "files": scn.files, "runs": scn.runs}
            runs, files = execute(case, tool, prefix)
            case["runs"] = [{"argv": r["argv"], "stdin": r["stdin"], "exit": r["exit"], "stdout": r["stdout"]} for r in runs]
            case["expect_files"] = files
            (FIXTURES / f"{name}.json").write_text(json.dumps(case, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")
        print(f"blessed {len(list(FIXTURES.glob('*.json')))} cases into {FIXTURES.relative_to(ROOT)}")
        return 0
    problems, n = [], 0
    for path in sorted(FIXTURES.glob("*.json")):
        if a.only and a.only not in path.stem:
            continue
        case = load_case(path)
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
