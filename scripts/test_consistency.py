#!/usr/bin/env python3
"""Tests scripts/consistency.py on throwaway trees. Called by check.sh."""
import os
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import consistency  # noqa: E402


def tree(files: dict[str, str]) -> Path:
    root = Path(tempfile.mkdtemp())
    for name, text in files.items():
        p = root / name
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(text)
    return root


SKILL = "plugins/git/skills/workflow/SKILL.md"


class Sections(unittest.TestCase):
    def test_heading_found(self):
        root = tree({SKILL: "---\nname: workflow\n---\n## Landing\n", "plugins/repo/a.md": "`git:workflow` → *Landing*\n"})
        self.assertEqual(consistency.check_sections(root), [])

    def test_bold_lead_in_and_reference_found(self):
        root = tree({
            SKILL: "---\nname: workflow\n---\n- **A new repository** starts\n",
            "plugins/git/skills/workflow/references/x.md": "# Issues and work items\n",
            "shared/a.md": "`git:workflow` says (*A new repository*) and `git:workflow`, *Issues and work items*\n",
        })
        self.assertEqual(consistency.check_sections(root), [])

    def test_missing_heading_fails(self):
        root = tree({SKILL: "---\nname: workflow\n---\n## Landing\n", "shared/a.md": "see `git:workflow` → *Merging*\n"})
        problems = consistency.check_sections(root)
        self.assertEqual(len(problems), 1)
        self.assertIn("no section 'Merging'", problems[0])


class Retired(unittest.TestCase):
    def test_clean(self):
        self.assertEqual(consistency.check_retired(tree({"shared/a.md": "Ask up to four questions per prompt.\n"})), [])

    def test_each_phrasing_fails(self):
        for text in ("Ask 3–5 questions.", "one question at a time", "at most 5 questions", "all 12 plugins", "47 skills"):
            with self.subTest(text=text):
                self.assertEqual(len(consistency.check_retired(tree({"README.md": text + "\n"}))), 1)


class Containers(unittest.TestCase):
    def test_creating_without_cleanup_fails(self):
        root = tree({"plugins/x/skills/y/SKILL.md": "Give each track a named cache volume.\n"})
        self.assertEqual(len(consistency.check_cleanup(root)), 1)

    def test_creating_with_cleanup_passes(self):
        for text in ("remove them per `../../references/containers.md` §9a", "then `wt.py remove <branch>`",
                     "run the project's `clean` recipe"):
            with self.subTest(text=text):
                root = tree({"plugins/x/skills/y/SKILL.md": "Start one with `wt.py add`.\n",
                             "plugins/x/skills/y/references/r.md": text + "\n"})
                self.assertEqual(consistency.check_cleanup(root), [])

    def test_prune_taught_as_a_command_fails(self):
        self.assertEqual(len(consistency.check_prune(tree({"shared/a.md": "Run `podman system prune -a`.\n"}))), 1)
        self.assertEqual(len(consistency.check_prune(tree({"shared/a.md": "`$CONTAINER_ENGINE image prune`\n"}))), 1)

    def test_prune_said_never_passes(self):
        root = tree({"shared/a.md": "Never `podman system prune`; `git fetch --prune` is fine.\n"})
        self.assertEqual(consistency.check_prune(root), [])


class Budget(unittest.TestCase):
    def test_ceiling(self):
        root = tree({"skills/a/SKILL.md": "---\nname: a\ndescription: " + "x" * 100 + "\n---\n"})
        os.environ["DESC_TOTAL_CEILING"] = "99"
        try:
            self.assertEqual(len(consistency.check_budget(root)[0]), 1)
            os.environ["DESC_TOTAL_CEILING"] = "100"
            self.assertEqual(consistency.check_budget(root)[0], [])
        finally:
            del os.environ["DESC_TOTAL_CEILING"]


if __name__ == "__main__":
    unittest.main()
