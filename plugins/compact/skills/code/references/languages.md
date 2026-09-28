# Language support

`compact.py --list-languages` prints the live table. Levels: **L0** as-is, **L1** drop
blank lines and trailing whitespace, **L2** + indentation, **L3** + newlines,
**L4** + optional spaces. A request above a language's maximum is capped silently;
a verification failure steps down one flag at a time and says so on stderr
(`fallback[join:ast]`).

## Contents
- Full support (L4)
- Line-safe (L2) and blank-only (L1)
- Kept as-is (L0)
- Known limitations

## Full support (L4)

| Language | Newline rule used when joining | Notes |
|---|---|---|
| Java | free | text blocks, annotations, generics `>>` fine |
| C, C++, CUDA, ObjC, ObjC++ | free | each `#` directive kept verbatim on its own line (continuations too) |
| C# | free | `#region/#if` verbatim; C# 11 raw strings protected |
| Rust | free | lifetimes, raw strings, `///` doc lines |
| PHP | free | `<?php` and heredoc/nowdoc closers keep their newline; files with inline HTML after `?>` are left as-is |
| Dart, Protobuf, GLSL/HLSL, Solidity, JSON | free | |
| Go | go | the semicolons Go's lexer would insert are written out; checked by an independent re-derivation |
| JavaScript, TypeScript, JSX, TSX | js | newline removed only where ASI cannot fire; a statement-ending newline becomes `;`; never after `return`/`throw`/`break`/`continue`/`yield`, never before `++`/`--`, never inside JSX text |
| Kotlin | kotlin | joins only where the grammar allows a newline; statement ends become `;`; property accessors, annotations, infix calls (`a to b`) and `when`/`else` are respected |
| CSS, SCSS, Less | css | spaces kept wherever they could be a descendant combinator (`.a .b`, `a :hover`) or an operator (`calc(1px + 2px)`) |

## Line-safe (L2) and blank-only (L1)

L2 (indentation removed, line structure kept): Swift, Groovy/Gradle, SQL, Lua, Ruby,
R, Bash (unless it has heredocs), TOML, XML (unless `xml:space="preserve"`), HTML
(unless `<pre>`/`<textarea>`), Zig, Clojure, Elixir, Verilog, Terraform, Dockerfile.

L1 (only blank lines/trailing whitespace): **Python**, Haskell, F#, Nim, CoffeeScript,
Elm, Scala (Scala 3 indentation syntax), Perl, PowerShell.

For Python, the paper's own measurement was only 6.5% savings (it could only remove
intra-line spaces). Compacting Python is rarely worth a tool call; read it normally.

## Kept as-is (L0)

YAML, Makefile, Markdown, reStructuredText, Pug, Sass (indented), Stylus, and any file
whose lexer reports errors that no AST check can vouch for.

## Known limitations

- tree-sitter-kotlin and tree-sitter-go reject some valid one-line code, so Kotlin and Go
  are verified lexically (Go by re-deriving the semicolons the Go lexer inserts; Kotlin by
  the grammar-derived join rules, validated against ktfmt on 390 files).
- JS/TS: newlines are turned into `;` only when tree-sitter is installed *and* parses the
  original file (not the case for Flow-typed files); otherwise only the always-safe joins
  are made.
- Lexers are not parsers: exotic macros or template metaprogramming may cause a safe
  fallback to a lower level. That costs tokens, never correctness.
