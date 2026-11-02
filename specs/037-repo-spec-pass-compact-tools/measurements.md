# Measurements: 037 repo, spec, pass-store and compact tools

Status: Groups A (module), B and C delivered beside the Python tools; D: grammar research, spike and verdict recorded (T060-T062), the port itself (T064-T068) waits for the owner's answer to open question 4. Manual gates (T006, T028) are still to record here.

## Binary size (T001-T027 delivery)

Release build, `cargo build --release --locked -p ai-gent`, glibc x86_64 Linux, `opt-level = "z"`, LTO, strip:

| Build | Bytes |
|---|---|
| `dev` at 13bc8d0 (version, doctor, update, guard, tokens) | 1,870,504 |
| this branch (adds spec, repo, pass-store) | 2,162,848 |
| delta | +292,344 (+15.6%) |

No new dependencies: `regex`, `serde_json`, `clap` are unchanged; SHA-1, base64, percent-encoding and the URL pieces are hand-written (tested against vectors); HTTP is the system `curl` (see T023 evidence).

## Parity

| Gate | Fixtures | Result |
|---|---|---|
| specsync (8 tests) + live corpus | 16 | oracle and binary equal, 0 differences |
| reporun (45 tests) | 53 (45 mapped + edge cases + canary) | oracle and binary equal, 0 differences |

Disagreements found while porting: none left. One Python defect noticed (not ported, not fixed): a `batch` CLI step whose word is a whole `{{step.field}}` reference to a number crashes `reporun.py` with `AttributeError` (the fixture uses a string field).

Deliberate differences from `reporun.py` (all stricter, none visible to a fixture): absolute API URLs must match the base URL's scheme as well as its host; `REPORUN_TEST_BASE` is honoured only for loopback hosts; the HTTP client does not follow redirects; `Last sync` in `tracker.md` is the UTC date (Python used the local date).

## Compact spike (T060-T062)

Spike code: `rust/spikes/compact` (standalone crate, outside the workspace, never shipped). Grammar table: `grammar-table.md`.

**Prototype.** Tokens are the leaves of the tree-sitter tree (strings, comments, chars, regexes and JSX text are atomic); the renderer rewrites only the whitespace between tokens (blank, indent, join with the `free` rule and a `js` ASI rule, spaces); the verifier re-parses the output and demands equal token texts and the same statement-level structure, where `STRUCT_RE`, `AST_WRAPPERS` and `AST_SKIP` are ported from `compact.py` and leaf positions are marked in the sequence (a negative test shows a joined `return` and a changed string are rejected). Fallback goes one flag at a time, spaces, join, indent, blank.

Result of `cargo test --release` in the spike: `cache.rs`, `Inventory.java`, `service.ts` and `Widget.tsx` reach L1, L2, L3 and L4 with no fallback (`test_every_fixture_at_every_level` for those four files); the strings-and-comments test on `Inventory.java` passes.

One honest finding while building it: the first, stricter verifier (every named node kind) rejected `service.ts` at L3, because tree-sitter-typescript wraps `namespace X {}` in an `expression_statement` only when a newline precedes it. The Python rule (statement-level kinds, `expression_statement` ignored) is the one that matches the grammars' quirks; the spike uses it.

**Output size against `compact.py` 3.x** (bytes, same file and level; the Python numbers come from `uv run --with pygments --with tree-sitter-language-pack`):

| File | L3 Python | L3 spike | L4 Python | L4 spike |
|---|---|---|---|---|
| cache.rs | 1,332 | 1,330 | 1,243 | 1,239 |
| Inventory.java | 2,046 | 2,046 | 1,894 | 1,894 |
| service.ts | 1,443 | 1,442 | 1,309 | 1,308 |
| Widget.tsx | 425 | 456 (+7.3%) | 396 | 427 (+7.8%) |

Rust, Java and TypeScript are at or below the Python size (SC-005 holds for them). TSX fails SC-005 in the prototype: the Python tool writes a newline between JSX attributes as a space, drops whitespace between JSX text and a tag, and turns ASI newlines into `;` (`semi_join`); the spike has none of these. These are known, portable rules (they are written down in `compact.py`), not an obstacle in the design, but they are work the spike did not do.

**Binary size.** Standalone probe, opt-level z, LTO, strip, baseline (tree-sitter core and a stub) 303,360 bytes. Grammar growth: rust +1.16 MB, java +0.47 MB, typescript/tsx +2.91 MB (the three prototype grammars, 4.5 MB together); c +0.66, cpp +3.47, c-sharp +5.37, go +0.27, javascript +0.46, php +1.13, kotlin-ng +3.47, objc +5.34, cuda +7.08, hlsl +4.26, dart +1.51, glsl +0.89, solidity +0.57, json +0.05, css +0.17, scss +0.19. The 10 grammars of the plan's "keep at L4 first" list (java, c, cpp, csharp, rust, go, javascript, typescript/tsx, php, kotlin) add 19.4 MB; the 19 crates behind the 24 L4 languages add 39.4 MB; all 36 names add 78.5 MB. The `ai-gent` binary is 2.2 MB today and the budget in `specs/032-rust-workspace-and-release/data-model.md` is 8 MB per target (an [ASSUMPTION] there).

**Verdict (T062).**

- Grammar availability: 21 of the 24 L4 languages have a crate (`objcpp`, `proto`, `less` do not) and 36 of 36 tree-sitter names do. The "at least 18 of 24" count passes on availability.
- Prototype: green on the four fixtures, all levels.
- What is proven: 4 languages (3 grammars) in 2 of the 5 join families. What is not: the `go`, `kotlin` and `css` families, the other `free` languages (preprocessor lines in c, cpp, objc, cuda, csharp, glsl, hlsl), and the L4 rules of `jsx`. The plan's own risk note applies: `go` and `kotlin` grammars reject valid one-line code, so their joins are checked by derived rules, as today.
- **Binary size is the blocking finding.** Compiling in the languages the SKILL.md names costs 19 MB and the full L4 set 39 MB, against 8 MB for the whole binary. This is a decision for the owner, not for this task: (a) raise the budget for a `compact` build, (b) ship `compact` as a separate binary or an optional cargo feature with its own archive, (c) keep a small set (rust, java, javascript/typescript, go, c, json, css, about 6 MB) and let the rest fall to L1, or (d) let the grammars load at run time (the plan rejected network fetches; a local grammar directory is not evaluated).

The rule says: pass when the prototype is green and at least 18 of 24 keep L4. The prototype is green and 21 can; but the rule did not price the grammars. The spike therefore **passes the test and raises a size question that open question 4 must answer** before T064-T068 are built. Nothing past T063 was started.

## Compact spike gate for D29 (T063b, 2026-11-02)

Spike: `rust/spikes/compact` (still outside the workspace and never shipped). Scope (D29): rust, java, c, cpp, csharp, go, javascript (with JSX), typescript, tsx, kotlin, css, and scss for the hazard. `cargo test --release` in the spike: 13 tests green, including `proto_every_fixture_at_every_level`, which runs the 13 in-scope fixtures at levels 1 to 4 and asserts: no fallback, the level reached equals `min(level, EXPECTED)` (scss 2), and the output is at most 1.005 times the `compact.py` output (the `PYTHON` table in `tests/fixtures.rs` holds the sizes of `compact.py -q -LN` with Pygments 2.21.0).

Output bytes, Rust spike against `compact.py` (L1 / L2 / L3 / L4); the Rust numbers are from `cargo run --release --example sizes plugins/compact/skills/code/tests/fixtures`:

| File | Python | Rust spike |
|---|---|---|
| Widget.tsx | 568 / 440 / 425 / 396 | 568 / 440 / 425 / 396 |
| service.ts | 1567 / 1491 / 1443 / 1309 | 1567 / 1491 / 1442 / 1308 |
| app.mjs | 1473 / 1341 / 1281 / 1162 | same |
| nosemi.mjs | 446 / 414 / 401 / 352 | same |
| cache.rs | 1550 / 1378 / 1332 / 1243 | 1550 / 1378 / 1332 / 1241 |
| Inventory.java | 2530 / 2096 / 2046 / 1894 | same |
| matrix.c | 1102 / 984 / 965 / 890 | same |
| graph.cpp | 1260 / 1139 / 1104 / 983 | same |
| Program.cs | 1382 / 1104 / 1079 / 1014 | same |
| server.go | 1533 / 1456 / 1422 / 1309 | same |
| Shapes.kt | 1705 / 1421 / 1396 / 1282 | same |
| site.css | 401 / 381 / 364 / 346 | same |
| theme.scss | 182 / 162 / 162 / 162 | same |

What was proven, per condition:

- **TSX (SC-005).** `Widget.tsx` went from +7.8% to equal at L4 (396 bytes, 0%). Ported: the JSX tag and text classification (from the tree-sitter tree, not from a lexer: words of JSX text, tag tokens, gaps between children), the attribute rule (a newline between attributes is a space), the text/tag rule (whitespace with a newline between JSX text and a tag or `{expr}` goes; a space stays, it is text), and `semi_join`, `value_end`, `js_block_close` for the `js` and `kotlin` families. Whitespace-only JSX text is not content for the tree-shape check.
- **go, kotlin, css.** All three reach L4 with the same bytes as Python. The tree-sitter-go and tree-sitter-kotlin-ng grammars drop tokens when they re-parse joined code (the plan's risk note, now measured: `sealed class Shape {...;data class ...` loses a `;`), so for Go and Kotlin the verifier is lexical, the `lexical_diff` of `compact.py`: a whitespace-free character stream, a `;` only where the original had a statement-ending newline (for Go, only where its lexer inserts one, and a place where it would insert one keeps a newline or a `;`), no fusing of tokens that were apart, strings, comments and preprocessor lines verbatim and in order. A file whose original does not parse (macros in C, `UNUSED int x;`) takes the same lexical path, as `bad_a` does in Python; the dropped text comes back as plain tokens.
- **Preprocessor lines (c, cpp, csharp).** A leaf starting with `#` at the start of a line becomes one verbatim token through the end of the directive (`\` continuations and a `/* */` opened on it included); no newline next to it is joined; `matrix.c` (continued macros, `#ifdef` and an unknown macro) matches Python byte for byte.
- **Look-around patterns, no heavier regex engine.** `rust/spikes/compact/src/lookaround.rs`: `has_line_comment_hazard` (`(?<![:/])//` of scss and less) and `csharp_raw_strings` (`\$*("{3,})[\s\S]*?\1(?!")` of csharp `protect`, including the greedy-then-backtrack group length and the lazy scan), each a few lines with tests whose expected values were computed with Python's `re`; the C# tokenizer refuses a raw string that comes out split. `less` has no grammar and stays outline only; its pattern is the same function.

Two known differences from Python, both inside the D29 scope: (1) a source whose original does not parse at all in a non-C-family grammar is still accepted through the lexical path, which is weaker than Python's lexer view only where Pygments would have flagged a token; (2) Kotlin and Go joins are only as safe as their rules, as in Python (the verifier cannot tell `return\nx` from `return x`; the rule `value_end` keeps that newline, covered by `kotlin_keeps_newlines_where_the_grammar_needs_them`).

**Binary size of the D29 scope.** A release binary (opt-level z, LTO, strip) linking tree-sitter and the 11 grammars above plus the spike code: 18.5 MB on x86_64 Linux (glibc), `cargo build --release --example sizes` in the spike. That is under the 25 MB proposal for `ai-gent-compact` (about 35% headroom for the CLI, the outline and the other targets) and far over the 8 MB of `ai-gent`, which is why D29 splits it.

## Compact port (T064-T067, 2026-11-02)

`ai-gent-compact` (release profile, x86_64 Linux glibc): 18,845,392 bytes, under the 25 MB proposal for its own budget. Size comparison with `compact.py` (`scripts/compare-compact.py`): 0 problems over the 17 fixtures and about 330 files in all (the fixtures and this repository's tracked sources), per-language ratios 0.9564 to 1.0002. CI builds it on all six targets (musl legs install `musl-tools`, because the grammars are C).

Known differences from the Python tool: SCSS is read with the CSS grammar (`tree-sitter-scss` 1.0.0 does not build with MSVC), so SCSS with nesting is checked lexically; `.h` files with Objective-C markers and every language outside the D29 scope are printed as they are; input that is not UTF-8 is read lossily; `less` has no grammar and is outline only.

## Not measured yet

- T006 gopass on Windows.
- T028 `scripts/test-reporun-github.py` against a real repository (owner).
- Group D (grammar table, spike verdict).
