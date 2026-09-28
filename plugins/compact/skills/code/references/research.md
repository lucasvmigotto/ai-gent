# Research notes and measurements

## Contents
- The paper (arXiv:2508.13666)
- What this skill measured
- Round-trip validation on real code
- Measured on this user's code (ai-gent)
- Caveats
- Re-running the measurements

## The paper

Pan, Sun, Zhang, Lo, Du - *The Hidden Cost of Readability: How Code Formatting Silently
Consumes Your LLM Budget* (arXiv:2508.13666). Fill-in-the-middle completion on McEval
(Java, C++, C#, Python), 10 models including Claude-3.7, GPT-4o, Gemini-1.5, DeepSeek-V3.

- Removing indentation, newlines and non-essential spaces cut **input tokens by 24.5%**
  on average (Java 34.9%, C++ 31.1%, C# 25.3%, Python 6.5% - Python only lost spaces).
- **No statistically significant Pass@1 change**; the largest drop was 4.2% on a small
  model (Phi-3.5); Claude-3.7 went 68.5 -> 69.0 (Java), 72.9 -> 72.9 (C++), 90.3 -> 87.7 (C#).
- Per element, **newlines were the largest token cost for Claude-3.7** (14.6% of tokens);
  indentation 7.9-9.6% depending on the tokenizer.
- Comments were kept; spaces the grammar needs (between keywords/identifiers) were kept.
- Models keep their usual formatting in *outputs* even when inputs are compact (output
  saving only 2.5%). Prompting ("Output code without formatting, maintaining syntax")
  gave GPT-4o 27.2% fewer output tokens with no loss; Gemini-1.5 then produced syntax
  errors (it removed required spaces). Fine-tuning on 50 samples gave 25-36%.
- Their bidirectional tool (Uncrustify-based) restored formatting with 100% AST
  equivalence on McEval, ~76 ms per sample.

Design consequences for this skill: default to level 3 (keep ordinary single spaces,
which avoids the Gemini failure mode and costs ~nothing in tokens), make removal
lexer-aware and verified instead of regex-based, and restore layout with the project's
real formatter.

## What this skill measured

Token counts use two offline proxies: `@anthropic-ai/tokenizer` (the published,
older Claude tokenizer) and OpenAI's `o200k_base`. Current Claude models use a newer
tokenizer, so treat absolute numbers as indicative; the ranking of levels was the same
under both proxies.

**506 real files** (React Native Java/Kotlin/C++, pythonnet C#, Go stdlib, crates.io Rust,
rxjs TypeScript, react-query TSX, npm JavaScript), change in tokens vs the raw file,
Claude-proxy tokenizer:

| language | files | Read view (`cat -n`) | L2 | L3 | L4 | L4 + strip comments |
|---|---|---|---|---|---|---|
| java | 60 | +33% | +0% | -9% | -11% | -30% |
| kotlin | 60 | +28% | +0% | -6% | -7% | -30% |
| csharp | 60 | +38% | -0% | -11% | -12% | -32% |
| cpp | 60 | +33% | -0% | -9% | -10% | -23% |
| go | 60 | +23% | -8% | -15% | -17% | -37% |
| rust | 60 | +22% | -0% | -7% | -8% | -31% |
| typescript | 60 | +23% | +0% | -3% | -5% | -68% |
| javascript | 60 | +30% | -0% | -8% | -11% | -32% |
| tsx | 26 | +42% | -0% | -9% | -10% | -12% |
| **all** | 506 | **+28.6%** | -1.6% | **-9.3%** | **-10.7%** | -33.4% |

Same files with `o200k_base`: L2 -8.7%, L3 -10.2%, L4 -14.0%, L4+strip -36.1%.

Take-aways:
1. With the Claude-proxy tokenizer, indentation is almost free (runs of spaces merge
   into one token) and **newlines are what cost** - matching the paper's per-element
   finding for Claude-3.7. Level 2 alone is not worth a tool call.
2. Compared with what the Read tool actually puts in context (a numbered view), a level-3
   compact read is **~29% cheaper** and level 4 ~31%; with `-c` ~48%.
3. Real repositories carry many comments (licence headers, docs), which is why the raw
   savings are below the paper's McEval numbers (small, comment-light functions).
   `--strip-comments` is the biggest single lever when you only need the logic.

On the 17 hand-written fixtures (dense, tricky code) the Claude proxy gave L3 -12.7%,
L4 -15.1%, L4+strip -19.3%, and -33.6% / -35.5% / -38.6% vs the numbered view.

## Round-trip validation on real code

This copy is read-only: the formatter (`format_code.py`) and `tests/run_tests.py`
aren't included. The validation below still matters here — it is what showed that
the compact text is the same program as the file.

`tests/corpus_check.py --format` compacts each file, formats both the original and the
compact text with the same formatter, and compares the results:
*exact* (byte-identical), *blanks* (identical except blank lines), *layout* (same code
tokens, the formatter chose another layout or re-wrapped a comment), *FAIL*.

| language | formatter | files | reached L3 / L4 | exact / blanks / layout / FAIL (L3) |
|---|---|---|---|---|
| Kotlin | ktfmt | 170 | 170 / 169 | 104 / 66 / 0 / 0 |
| Java | google-java-format | 180 | 180 / 179 | 56 / 119 / 5 / 0 |
| TypeScript | prettier | 269 | 269 / 269 | 154 / 100 / 15 / 0 |
| TSX | prettier | 29 | 26 / 25 | 0 / 28 / 1 / 0 |
| JavaScript (incl. Flow+JSX) | prettier | 397 | 397 / 396 | 38 / 338 / 18 / 0 |
| Go | gofmt | 297 | 297 / 297 | 82 / 79 / 133 / 0 |
| Rust | rustfmt | 295 | 293 / 293 | 108 / 138 / 48 / 1* |
| C / C++ / ObjC | clang-format | 299 | 299 / 299 | 4 / 261 / 16 / 0 |
| C# | clang-format (strict) | 148 | 148 / 148 | 12 / 111 / 15 / 0 |

\* rustfmt wrapped one `match` arm in braces for one layout and not the other - equivalent
code, but different tokens. "Reached" counts files where verification accepted the
requested level; the rest fell back automatically (never a wrong result).

The fixture suite (`tests/run_tests.py`) additionally compiles and **runs** the compact
text for Java, C, C++, Go, Rust, JS, TS and PHP and compares stdout with the original
(all identical, including an ASI-heavy no-semicolon JS file), and checks
`format(compact) == format(original)` for every level.

Bugs found by this validation and fixed (useful to know what the checks guard against):
Go build-constraint blank lines, Kotlin infix calls (`a to\nb`), Kotlin `private set` /
`@get:Annotation` placement, `++` split by Pygments, TS `Array<Array<T>>` looking like
`>>`, JSX text/attribute whitespace, `${` in template literals, `//` comments inside JSX
tags that Pygments does not recognise, C `#else` inside `#if 0` blocks, Rust `ident#`
(reserved in 2021), import groups re-sorted by gofmt/rustfmt when blank lines vanish,
clang-format rewriting C# raw strings (now refused).

## Measured on this user's code (ai-gent)

`o200k_base` tokens (a proxy; Claude's tokenizer differs), 2026-09-28. "Read view"
is the file with line numbers, which is what the Read tool puts in context.

| code | files | Read view vs raw | raw vs Read view | L3 vs Read view | L3 `-c` vs Read view |
|---|---|---|---|---|---|
| Java (a Spring/Tomcat service) | 60 | +41% | −29% | −36% | −42% |
| TypeScript (an Angular client) | 34 | +45% | −31% | −39% | −44% |
| Python (ai-gent's scripts) | 5 | +26% | −21% | −21% | −22% |
| shell (ai-gent's hooks) | 3 | +21% | −17% | −18% | −39% |

Most of the saving is avoiding the line-numbered view; compaction adds 10–19% on
brace languages and next to nothing on Python and shell. `corpus_check.py` on the same
Java and TypeScript (69 and 34 files) reached L3 and L4 with no fallback; one
already-minified JavaScript file fell back safely.

## Caveats

- The paper measured accuracy on fill-in-the-middle; for large multi-file reasoning the
  evidence is weaker. Keep comments (default) when intent matters.
- Pygments is a lexer, not a parser; the skill compensates with re-lexing, optional
  tree-sitter AST comparison, per-language newline rules and conservative fallbacks.

## Re-running the measurements

```bash
uv run --with pygments --with tree-sitter-language-pack python3 tests/corpus_check.py path/to/repo
```

(`--format` also needs the formatters, which this read-only copy doesn't use.)
