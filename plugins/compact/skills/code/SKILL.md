---
name: code
description: Read source code token-lean — a compact view with indentation, newlines and optional spaces removed (strings, comments and preprocessor lines kept, every result self-verified) and an outline with original line numbers, for Java, C#, C/C++, Go, Rust, JS/TS, Kotlin, PHP and more. Read-only. Use before reading large or many source files, mapping or reviewing a codebase, or when tokens are tight.
---

# Compact code reading

Layout is for humans; for a model most of it is token overhead. Removing
indentation, newlines and optional spaces cut input tokens by about 25% with
no measurable accuracy loss (arXiv:2508.13666). On this user's own Java and
TypeScript, a compact read costs **36–39% fewer tokens** than the Read
tool's line-numbered view (42–44% with comments dropped) —
`references/research.md` has the numbers.

This copy only **reads**: it never writes, formats or rewrites a file.

## Running it

`scripts/compact.py` (relative to this file) needs Pygments; tree-sitter adds
an AST check. Without installing anything:

```bash
C="uv run -q --with pygments --with tree-sitter-language-pack python3 <this skill's dir>/scripts/compact.py"
```

(or plain `python3 …/compact.py` where Pygments is installed).

```bash
$C src/main/java/shop/OrderService.java      # one file, level 3
$C -L 4 src/                                 # a whole tree, denser
$C -c src/                                   # comments dropped (directives kept): logic only
$C --outline 1 -n Big.java                   # one member per line, original line numbers
$C -o <scratch>/view.txt src/ && wc -c <scratch>/view.txt   # large output: write, then read in parts
```

Levels (`-L`): 1 blank lines · 2 + indentation · **3 + newlines (default)** ·
4 + optional spaces. Each language caps the level to what's safe for it
(`--list-languages`); per-language rules are in `references/languages.md`.

## When to use it

- **Yes:** files over ~150 lines, many files at once, mapping an unfamiliar
  codebase (`project:survey`, `project:introspec`), reviews and audits that
  read code in bulk, understanding logic before a change.
- **No:**
  - small files — the call costs more than it saves;
  - Python, YAML, Makefile, Markdown and other layout-sensitive files — only
    blank lines would go; read them normally (or `cat`/`sed -n`, which already
    skip Read's line numbers);
  - anything shown to the user — show the real file.

## What you get

- Strings, char literals and multi-line strings verbatim; comments kept
  unless `-c`; preprocessor lines and every newline the language needs
  (after `//` comments; Go/JS/Kotlin statement ends become `;` or stay).
- Every result is re-lexed and, with tree-sitter, re-parsed against the
  original. On any doubt it steps down a level and says so on stderr, e.g.
  `compact: App.tsx tsx L3 … fallback[spaces:ast]`.
- Output is soft-wrapped at an original line break every ~1,500 characters.
  Past about 25k characters, write it with `-o` and read it in parts.

## Editing after reading

The compact view is for **understanding**. An edit's old text must come from
the file as it is on disk:

1. Locate: `--outline 1 -n File` (the number before `|` is the original
   line) or grep.
2. Read those exact lines normally (Read with an offset and limit).
3. Edit from that text, in the file's own formatting.
