#!/usr/bin/env python3
"""SC-005 size comparison of `ai-gent-compact` against `compact.py` (spec 037 T067).

  python3 scripts/compare-compact.py [--bin PATH] [--levels 3,4] [--max-files N] [PATH...]

Compacts the same files with both tools at each level and prints, per language, the files, the
bytes of each and their ratio (Rust over Python). It fails (exit 1) when a file's output is more
than 0.5% larger than the Python output, or when the Rust tool reaches a lower level than Python.
Only the languages D29 compacts are compared. Without PATH it uses the fixtures of the compact skill
and this repository's own sources. The Python side runs through `uv` with Pygments 2.21.0 pinned,
like `scripts/check.sh`. Removed with `compact.py` at the 3.0.0 cutover.
"""
import argparse
import json
import re
import subprocess
import sys
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SKILL = ROOT / "plugins/compact/skills/code"
SCOPE = {"java", "csharp", "c", "cpp", "go", "rust", "javascript", "jsx", "kotlin", "css", "scss",
         "typescript", "tsx"}
LIMIT = 1.005


def python_side(files, levels):
    sys.path.insert(0, str(SKILL / "scripts"))
    import compact  # noqa: E402
    from langs import detect_language  # noqa: E402
    for f in files:
        lang = detect_language(f)
        try:
            code = open(f, encoding="utf-8").read()
        except (OSError, UnicodeDecodeError):
            continue
        for lv in levels:
            text, info = compact.compact_text(code, lang, lv, wrap=1500)
            print(json.dumps({"file": f, "lang": info["lang"], "level": lv,
                              "bytes": len(text.encode()), "reached": compact.level_of(info["flags"]),
                              "fallback": info["fallback"], "error": info.get("error")}))


def source_files(paths, max_files):
    if paths:
        roots = [Path(p) for p in paths]
    else:
        roots = [SKILL / "tests/fixtures"]
    out = []
    for r in roots:
        if r.is_file():
            out.append(str(r))
            continue
        for p in sorted(r.rglob("*")):
            if p.is_file() and not any(x in p.relative_to(r).parts for x in ("node_modules", "target", ".git", ".worktrees", "vendor", "dist")):
                out.append(str(p))
    if not paths:
        tracked = subprocess.run(["git", "ls-files"], cwd=ROOT, capture_output=True, text=True).stdout.split("\n")
        out += [str(ROOT / p) for p in tracked if p.endswith((".rs", ".ts", ".tsx", ".mjs", ".js", ".css", ".go", ".java", ".kt"))
                and not p.startswith("plugins/compact/skills/code/tests/fixtures") and (ROOT / p).is_file()
                and (ROOT / p).stat().st_size < 200_000]
    seen, files = set(), []
    for f in out:
        if f not in seen:
            seen.add(f)
            files.append(f)
    return files[:max_files] if max_files else files


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("paths", nargs="*")
    ap.add_argument("--bin", default=str(ROOT / "rust/target/debug/ai-gent-compact"))
    ap.add_argument("--levels", default="3,4")
    ap.add_argument("--max-files", type=int, default=0)
    ap.add_argument("--python-side", action="store_true", help=argparse.SUPPRESS)
    a = ap.parse_args()
    levels = [int(x) for x in a.levels.split(",")]
    if a.python_side:
        python_side(a.paths, levels)
        return 0
    files = source_files(a.paths, a.max_files)
    cmd = ["uv", "run", "-q", "--with", "pygments==2.21.0", "--with", "tree-sitter-language-pack",
           "python3", __file__, "--python-side", "--levels", a.levels] + files
    py = subprocess.run(cmd, capture_output=True, text=True)
    if py.returncode:
        print(py.stderr, file=sys.stderr)
        return 2
    rows = [json.loads(line) for line in py.stdout.splitlines() if line.startswith("{")]
    per_lang = defaultdict(lambda: [0, 0, 0])
    problems = []
    for r in rows:
        if r["lang"] not in SCOPE and r["lang"] != "jsx":
            continue
        p = subprocess.run([a.bin, "-q", "-L", str(r["level"]), r["file"]], capture_output=True)
        if p.returncode:
            problems.append(f"{r['file']} L{r['level']}: exit {p.returncode} {p.stderr.decode()[:80]}")
            continue
        s = subprocess.run([a.bin, "-L", str(r["level"]), r["file"]], capture_output=True)
        m = re.search(rb" L(\d) ", s.stderr)
        reached = int(m.group(1)) if m else -1
        n = len(p.stdout)
        agg = per_lang[(r["lang"], r["level"])]
        agg[0] += 1
        agg[1] += r["bytes"]
        agg[2] += n
        if n > r["bytes"] * LIMIT:
            problems.append(f"{r['file']} L{r['level']}: {n} bytes, python {r['bytes']} ({n / r['bytes']:.3f})")
        if reached < r["reached"]:
            problems.append(f"{r['file']} L{r['level']}: reached L{reached}, python L{r['reached']}")
    print("| language | level | files | python bytes | rust bytes | ratio |\n|---|---|---|---|---|---|")
    for (lang, lv), (n, pb, rb) in sorted(per_lang.items()):
        print(f"| {lang} | L{lv} | {n} | {pb} | {rb} | {rb / pb if pb else 1:.4f} |")
    for line in problems:
        print("FAIL", line)
    print(f"{len(problems)} problem(s)")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
