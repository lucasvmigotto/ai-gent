"""Unit tests for specsync: python3 -m unittest plugins/repo/scripts/test_specsync.py

A throwaway specs/ tree with one feature in Spec Kit's shapes — stories in
spec.md, tasks tagged [USn] or under a "User Story N" phase, layered
## Frontend / ## Backend sections — and the plan, state rules, tracker.md
round trip and retired features.
"""

import contextlib
import io
import json
import os
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import specsync  # noqa: E402

README = """# Features

| # | Feature | Priority | Depends on | Frontend | Backend |
|---|---|---|---|---|---|
| 012 | export | P1 | — | {fe} | {be} |
| 013 | legacy | P3 | — | — | — |

| # | Notes |
|---|---|
"""

SPEC = """# Feature Specification: CSV export

## User Scenarios & Testing

### User Story 1 - Export a timesheet (Priority: P1)

A manager downloads the month's timesheet as CSV.

**Acceptance Scenarios**:

1. **Given** a closed month, **When** export runs, **Then** a CSV downloads.

### User Story 2 — Schedule exports (Priority: P2)

Exports arrive by email every Monday.

## Requirements

- **FR-001** …
"""

TASKS = """# Tasks: export

## Phase 1: Setup

- [x] T001 Add the csv writer dependency

## Phase 2: User Story 1 - Export a timesheet (P1)

- [x] T002 [P] Test: CSV columns match the contract
- [ ] T003 Implement the writer in `api/export.py`

## Phase 3: User Story 2 — Schedule exports (P2)

- [ ] T004 Cron entry in `jobs/schedule.py`

## Frontend

- [x] T010 [P] [US1] Export button on the timesheet page
"""


class SpecsyncTest(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.feature = self.root / "specs" / "012-export"
        self.feature.mkdir(parents=True)
        (self.root / "specs" / "013-legacy").mkdir()
        (self.feature / "spec.md").write_text(SPEC)
        (self.feature / "tasks.md").write_text(TASKS)
        self.readme("In progress", "In progress")
        self.old_cwd = os.getcwd()
        os.chdir(self.root)

    def tearDown(self):
        os.chdir(self.old_cwd)
        self.tmp.cleanup()

    def readme(self, fe, be):
        (self.root / "specs" / "README.md").write_text(README.format(fe=fe, be=be))

    def plan(self):
        return specsync.build_plan(specsync.feature_dir("012"))

    def items(self):
        return {i["key"]: i for i in self.plan()["items"]}

    def test_items_and_story_assignment(self):
        items = self.items()
        self.assertEqual(set(items), {"feature", "US1", "US2"})
        self.assertEqual(items["feature"]["title"], "[012] CSV export")
        self.assertEqual(items["US2"]["title"], "[012] US2: Schedule exports")
        self.assertEqual(items["US1"]["priority"], "P1")
        # [US1] tag in a layer section and the phase heading both land in US1.
        self.assertIn("T002", items["US1"]["body"])
        self.assertIn("T010 Export button on the timesheet page (Frontend)", items["US1"]["body"])
        self.assertIn("T004", items["US2"]["body"])
        self.assertIn("T001", items["feature"]["body"])  # setup belongs to no story
        self.assertIn("<!-- ai-gent:sync specs/012-export US1 -->", items["US1"]["body"])
        self.assertIn("☑", items["US1"]["body_html"])

    def test_states(self):
        items = self.items()
        self.assertEqual(items["feature"]["state"], "doing")
        self.assertEqual(items["US1"]["state"], "doing")   # some tasks checked
        self.assertEqual(items["US2"]["state"], "todo")    # none checked
        self.readme("Implemented", "In progress")
        self.assertEqual(self.items()["feature"]["state"], "doing")  # the least advanced side wins
        self.readme("Verified", "Verified")
        items = self.items()
        self.assertEqual(items["feature"]["state"], "done")
        self.assertEqual(items["US2"]["state"], "done")     # Verified closes every story

    def test_all_tasks_checked_is_review(self):
        (self.feature / "tasks.md").write_text(TASKS.replace("- [ ] T003", "- [x] T003"))
        self.assertEqual(self.items()["US1"]["state"], "review")

    def test_tracker_round_trip_and_actions(self):
        self.assertTrue(all(i["action"] == "create" for i in self.plan()["items"]))
        out = self.root / "batch-out.json"
        records = [{"key": i["key"], "ref": f"#{n}", "id": 9000 + n, "url": f"https://x/{n}", "state": i["state"],
                    "hash": i["hash"]} for n, i in enumerate(self.plan()["items"], 101)]
        out.write_text(json.dumps({"ok": True, "records": records}))
        with contextlib.redirect_stdout(io.StringIO()):
            specsync.main(["record", "012", "--platform", "github", "--where", "o/r", "--from", str(out)])
        tracker = specsync.read_tracker(self.feature)
        self.assertEqual(tracker["platform"], "github")
        self.assertEqual(tracker["items"]["US1"]["ref"], "#102")
        self.assertTrue(all(i["action"] == "none" for i in self.plan()["items"]))
        # A checked task changes US1's body and state; nothing else moves.
        (self.feature / "tasks.md").write_text(TASKS.replace("- [ ] T003", "- [x] T003"))
        actions = {i["key"]: i["action"] for i in self.plan()["items"]}
        self.assertEqual(actions, {"feature": "none", "US1": "update+state", "US2": "none"})

    def test_orphans_reported(self):
        (self.feature / "tracker.md").write_text(
            "- Platform: github\n\n| Key | Item | Id | State | Hash | URL |\n|---|---|---|---|---|---|\n"
            "| US9 | #150 | 1 | todo | x | u |\n")
        self.assertEqual(self.plan()["orphans"], ["US9"])

    def test_retired_from_notes(self):
        (self.root / "specs" / "README.md").write_text(
            "| # | Feature | Frontend | Backend | Notes |\n|---|---|---|---|---|\n"
            "| 013 | legacy | — | — | **Retired by BCR-0001** |\n")
        plan = specsync.build_plan(specsync.feature_dir("013"))
        self.assertTrue(plan["retired"])
        self.assertEqual(plan["items"], [])

    def test_introspec_story_list(self):
        (self.feature / "spec.md").write_text(
            "# 012 — Export\n\n## Stories\n\n1. **CSV download**: a manager downloads the month.\n"
            "   - Accept: a CSV with the contract's columns.\n2. **Scheduled export** — every Monday.\n\n## Planned\n\n- later\n")
        (self.feature / "tasks.md").unlink()
        items = self.items()
        self.assertEqual(items["feature"]["title"], "[012] Export")
        self.assertEqual(items["US1"]["title"], "[012] US1: CSV download")
        self.assertIn("Accept: a CSV", items["US1"]["body"])
        self.assertEqual(items["US2"]["title"], "[012] US2: Scheduled export")
        self.assertNotIn("later", items["US2"]["body"])

    def test_status_with_notes_after_it(self):
        self.readme("Implemented (pending a real host)", "Implemented")
        self.assertEqual(self.items()["feature"]["state"], "review")


if __name__ == "__main__":
    unittest.main()
