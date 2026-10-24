#!/usr/bin/env python3
"""Tests scripts/consistency.py on throwaway trees. Called by check.sh."""
import json
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


CARDS = "specs/004-daemon/cards.md"


class Cards(unittest.TestCase):
    def test_every_card_has_a_projection(self):
        ok = "# Cards\n\n## Card `a`: T001\n\n**Route**: Effort `low` · Tokens: projected ~100k | used —\n"
        self.assertEqual(consistency.check_cards(tree({CARDS: ok})), [])

    def test_card_without_a_projection_fails(self):
        bad = ("# Cards\n\n## Card `a`: T001\n\n**Route**: Tokens: projected ~1k\n\n"
               "## Card `b`: T002\n\n**Route**: Effort `low`\n")
        problems = consistency.check_cards(tree({CARDS: bad}))
        self.assertEqual(len(problems), 1)
        self.assertIn("cards.md:7", problems[0])

    def test_fenced_examples_and_other_headings_are_not_cards(self):
        text = "# Cards\n\n```markdown\n## Card `x`\n```\n\n## Notes\n\nnothing\n"
        self.assertEqual(consistency.check_cards(tree({CARDS: text})), [])

    def test_template_must_show_the_line(self):
        ref = "plugins/agents/skills/orchestrate/references/cards.md"
        self.assertEqual(len(consistency.check_cards(tree({ref: "# Cards\n\n## Template\n"}))), 1)
        self.assertEqual(consistency.check_cards(tree({ref: "# Cards\n\nTokens: projected ~420k\n"})), [])  # no vocab: tags unchecked
        both = {ref: "# Cards\n\nTags: extend, rust · Tokens: projected ~420k\n", "shared/token-tags.json": VOCAB_JSON}
        self.assertEqual(consistency.check_cards(tree(both)), [])
        both[ref] = "# Cards\n\nTokens: projected ~420k\n"
        self.assertEqual(len(consistency.check_cards(tree(both))), 1)

    def test_worktrees_are_not_linted(self):
        self.assertEqual(consistency.check_cards(tree({".worktrees/x/" + CARDS: "## Card `a`\n"})), [])


VOCAB_JSON = json.dumps({"facets": {
    "kind": {"required": True, "multi": False, "values": {"extend": "x", "fix": "x"}},
    "lang": {"required": True, "multi": False, "values": {"rust": "x"}},
    "stack": {"required": False, "multi": True, "values": {"tokio": "rust", "react": "typescript"}, "aliases": {"react-dom": "react"}}}})
VOCAB_DOC = "`extend` `fix` `rust` `tokio` `react`"


class Tags(unittest.TestCase):
    def files(self, line, extra=None):
        card = f"# Cards\n\n## Card `a`: T001\n\n**Route**: {line} · Tokens: projected ~1k\n"
        return {CARDS: card, "shared/token-tags.json": VOCAB_JSON, "shared/token-tags.md": VOCAB_DOC, **(extra or {})}

    def problems(self, line):
        return consistency.check_cards(tree(self.files(line)))

    def test_one_value_per_facet_passes_stack_optional_and_aliased(self):
        self.assertEqual(self.problems("Tags: extend, rust"), [])
        self.assertEqual(self.problems("Tags: fix, rust, tokio, react-dom"), [])

    def test_missing_unknown_and_duplicate_facets_fail(self):
        self.assertIn("no lang tag", self.problems("Tags: extend")[0])
        self.assertIn("unknown tag 'cobol'", self.problems("Tags: extend, rust, cobol")[0])
        self.assertIn("2 kind tags", self.problems("Tags: extend, fix, rust")[0])
        self.assertIn("no `Tags:` line", self.problems("Effort `low`")[0])

    def test_vocabulary_values_must_be_documented_and_unique(self):
        ok = consistency.check_vocab(tree({"shared/token-tags.json": VOCAB_JSON, "shared/token-tags.md": VOCAB_DOC}))
        self.assertEqual(ok, [])
        missing = consistency.check_vocab(tree({"shared/token-tags.json": VOCAB_JSON, "shared/token-tags.md": "`extend`"}))
        self.assertTrue(any("'tokio' (stack) isn't documented" in m for m in missing))
        dup = json.loads(VOCAB_JSON)
        dup["facets"]["lang"]["values"]["fix"] = "x"
        self.assertTrue(any("'fix' is in both" in m for m in consistency.check_vocab(
            tree({"shared/token-tags.json": json.dumps(dup), "shared/token-tags.md": VOCAB_DOC + " `fix`"}))))


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
