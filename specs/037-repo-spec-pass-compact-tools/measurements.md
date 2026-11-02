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

## Not measured yet

- T006 gopass on Windows.
- T028 `scripts/test-reporun-github.py` against a real repository (owner).
- Group D (grammar table, spike verdict).
