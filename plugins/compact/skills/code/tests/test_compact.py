"""Tests for the read-only compact view: python3 -m unittest plugins/compact/skills/code/tests/test_compact.py

Needs Pygments (scripts/check.sh runs it with `uv run --with pygments`); skipped
without it. Every fixture is compacted at every level: the result must pass the
script's own verification, reach the level its language allows without a
fallback, and lex to the same code as the file. The outline's line numbers must
point at the original lines. Nothing may write to the file.

Manual stress test on any codebase (read-only):

  uv run --with pygments --with tree-sitter-language-pack \
    python3 plugins/compact/skills/code/tests/test_compact.py --corpus path/to/repo
"""

import contextlib
import io
import os
import sys
import tempfile
import unittest
import unittest.mock
from pathlib import Path

HERE = Path(__file__).resolve().parent
FIXTURES = HERE / "fixtures"
sys.path.insert(0, str(HERE.parent / "scripts"))

try:
    import pygments  # noqa: F401
    import compact
    from langs import LANGS, detect_language, iter_source_files
except ImportError:  # pragma: no cover - reported as a skip
    compact = None

# the level each fixture must reach (its language's maximum; SCSS with `//` comments keeps lines)
EXPECTED = {"yaml": 0, "python": 1, "scss": 2}


@unittest.skipIf(compact is None, "needs Pygments (pip install pygments, or uv run --with pygments)")
class CompactTest(unittest.TestCase):
    def fixtures(self):
        for path in sorted(FIXTURES.iterdir()):
            lang = detect_language(str(path))
            self.assertIsNotNone(lang, path.name)
            yield path, lang, path.read_text(encoding="utf-8")

    def test_every_fixture_at_every_level(self):
        for path, lang, code in self.fixtures():
            for level in (1, 2, 3, 4):
                with self.subTest(file=path.name, level=level):
                    text, info = compact.compact_text(code, lang, level)
                    self.assertFalse(info["fallback"], info)
                    self.assertNotIn("error", info)
                    reached = compact.level_of(info["flags"])
                    self.assertEqual(reached, min(level, EXPECTED.get(lang, 4)))
                    if info["flags"]:
                        spec = LANGS[info["lang"]]
                        self.assertFalse(compact.lexical_diff(code, text, spec, info["lang"]))

    def test_strings_and_comments_survive(self):
        code = (FIXTURES / "Inventory.java").read_text()
        text, _ = compact.compact_text(code, "java", 4)
        self.assertIn('"a  b\\tc"', text)
        self.assertIn("Javadoc with   odd   spacing must survive verbatim.", text)
        self.assertIn("  indented line inside a text block", text)
        stripped, _ = compact.compact_text(code, "java", 4, strip_comments=True)
        self.assertNotIn("block comment", stripped)
        self.assertIn('"a  b\\tc"', stripped)

    def test_outline_numbers_point_at_the_original_lines(self):
        path = FIXTURES / "Inventory.java"
        lines = path.read_text().splitlines()
        text, _ = compact.compact_text(path.read_text(), "java", 3, outline=1, numbers=True)
        checked = 0
        for row in text.splitlines():
            number, sep, rest = row.partition("|")
            if not sep or not number.strip().isdigit():
                continue
            original = lines[int(number) - 1].strip()
            head = rest.strip().split()[0] if rest.strip() else ""
            self.assertTrue(original.startswith(head) or head in original, (number, rest[:60], original))
            checked += 1
        self.assertGreater(checked, 5)

    def test_read_only(self):
        with tempfile.TemporaryDirectory() as d:
            p = Path(d, "A.java")
            p.write_text((FIXTURES / "Inventory.java").read_text())
            before = (p.read_bytes(), p.stat().st_mtime_ns)
            out, err = io.StringIO(), io.StringIO()
            with contextlib.redirect_stderr(err):
                with self.assertRaises(SystemExit):
                    compact.main(["-i", str(p)])  # the in-place option doesn't exist in this copy
                code = compact.main(["-q", "-o", os.path.join(d, "view.txt"), str(p)])
            self.assertEqual(code, 0)
            self.assertEqual((p.read_bytes(), p.stat().st_mtime_ns), before)


@unittest.skipIf(compact is None, "needs Pygments (pip install pygments, or uv run --with pygments)")
class VerifierTest(unittest.TestCase):
    """The safety net must reject a wrong compact text, whatever the renderer did."""

    ORIGINAL = 'class A{void f(){String s="a  b";int k=i - -j;int m=a + +b;boolean t=a && !c;int n=x = -1;}}\n'

    def diff(self, new, lang="java", original=None):
        return compact.lexical_diff(original or self.ORIGINAL, new, LANGS[lang], lang)

    def test_string_spacing_is_content(self):
        self.assertIsNotNone(self.diff(self.ORIGINAL.replace('"a  b"', '"a b"')))

    def test_fused_operators_rejected(self):
        for before, after in [("i - -j", "i--j"), ("a + +b", "a++b")]:
            with self.subTest(fused=after):
                self.assertIsNotNone(self.diff(self.ORIGINAL.replace(before, after)))
        go = "package main\nfunc f(x int) bool { return x < -1 }\n"
        self.assertIsNotNone(self.diff(go.replace("x < -1", "x<-1"), "go", go))

    def test_layout_only_changes_accepted(self):
        compacted = self.ORIGINAL.replace("a && !c", "a&&!c").replace("x = -1", "x=-1").replace("i - -j", "i- -j")
        self.assertIsNone(self.diff(compacted))

    def test_scss_line_comment_never_swallows_code(self):
        # Pygments misses this `//` comment inside a rule; joining lines would comment out line-height
        code = "pre {\n  font-size: 13px; // 14px to 13px\n  line-height: 1;\n}\n"
        for level in (3, 4):
            text, _ = compact.compact_text(code, "scss", level)
            comment_line = next(line for line in text.splitlines() if "//" in line)
            self.assertTrue(comment_line.rstrip().endswith("// 14px to 13px"), text)
        url = ".a{background:url(http://x/y.png);}\n.b {\n  color: red;\n}\n"
        self.assertEqual(compact.level_of(compact.compact_text(url, "scss", 4)[1]["flags"]), 4)

    def test_unparsable_result_rejected(self):
        if compact._ts_parser("java") is None:
            self.skipTest("needs tree-sitter-language-pack")
        broken = "class A{void f(){int k=1 int m=2;}}\n"  # a missing ';' the lexical view can't see
        with unittest.mock.patch.object(compact, "lexical_diff", return_value=None):
            self.assertEqual(compact.verify("class A{void f(){int k=1;int m=2;}}\n", broken, LANGS["java"], "java", False),
                             (False, "ast"))


def corpus(root: str) -> int:
    """Compact every supported file under `root` at L3 and L4; report levels and fallbacks."""
    rows, problems = {}, []
    for path in iter_source_files(root):
        lang = detect_language(path)
        try:
            code = open(path, encoding="utf-8", errors="surrogateescape").read()
        except OSError:
            continue
        if len(code) > 200_000:
            continue
        for level in (3, 4):
            _, info = compact.compact_text(code, lang, level)
            key = (info["lang"], level)
            n, fb = rows.get(key, (0, 0))
            rows[key] = (n + 1, fb + bool(info["fallback"] or info.get("error")))
            if info["fallback"] or info.get("error"):
                problems.append(f"{path}: L{level} {','.join(info['fallback'])} {info.get('error', '')}")
    print("| language | level | files | fallbacks |\n|---|---|---|---|")
    for (lang, level), (n, fb) in sorted(rows.items()):
        print(f"| {lang} | L{level} | {n} | {fb} |")
    for p in problems[:20]:
        print("  " + p)
    return 0


if __name__ == "__main__":
    if len(sys.argv) == 3 and sys.argv[1] == "--corpus":
        if compact is None:
            sys.exit("needs Pygments")
        sys.exit(corpus(sys.argv[2]))
    unittest.main()
