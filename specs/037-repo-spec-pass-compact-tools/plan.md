# Implementation Plan: Repo, spec, pass-store and compact tools

**Branch**: `docs/specs-rust-tools` | **Date**: 2026-11-01 | **Spec**: `spec.md`

## Summary

Four ports, built in value-and-risk order: the pass-store module (A), `ai-gent spec` (B), `ai-gent repo` (C), `ai-gent compact` (D, conditional on a spike). Each group has its own parity gate (D10) and its own cutover edit list (D7). Groups can land in separate releases; the tasks are phased so a session that stops after any phase leaves a releasable state.

## Technical Context

**Crates** (candidates; the dependency policy is 032's): `clap`, `serde`, `serde_json`, `regex`, a hashing crate for the specsync digest (algorithm read from `digest()` in the source at T010), a blocking HTTPS client with rustls (`ureq` or `reqwest` blocking), `url`, `base64`, `tempfile`; for D: `tree-sitter` plus grammar crates, `walkdir`.
**Testing**: shared fixtures (format: `specs/035-token-tools/contracts/parity-fixtures.md`); a Rust HTTP stub for C serving the same scripted exchanges as the Python `Stub(BaseHTTPRequestHandler)` (`test_reporun.py` lines 132-176); portable fake executables for `pass`, `gh`, `az`, `glab`, `jira` (one test binary `ai-gent-fake-tool` whose behaviour is chosen by its file name and a state file), replacing the shell stubs the Python `Sandbox` writes (lines 177-232).
**Platforms**: Linux, macOS, Windows, x86_64 and aarch64.

## Module layout (crate paths assumed until 032)

```
rust/ai-gent-core/src/
  passstore/{mod,pass,gopass,redact}.rs                                           A
  spec/{mod,parse,readme,tracker,html,plan,digest}.rs                              B
  repo/{mod,platforms,remote,entry,cli,api,http,batch,chain,project_status}.rs     C
  compact/{mod,langs,tokenize,render,verify,outline,walk}.rs                       D
rust/ai-gent/src/cmd/{spec,repo,compact}.rs
rust/ai-gent-core/data/compact-langs.rs     generated from langs.py by scripts/gen-compact-langs.py   D
rust/fixtures/{passstore,spec,repo,compact}/
```

## Constraint check

| Constraint | Result |
|---|---|
| D7 | per-group cutover lists below |
| D9 | `--json` on `detect`, `chain`, `plan`, `features`; documented exit codes (`contracts/cli.md`) |
| D10 | repo 45 tests, spec 8, pass-store 3 + 6 (the `EntryTest` class in `test_reporun.py` and `PassTest` in `test_dbrun.py`), compact 9 tests over 17 fixture files |
| D13 | opencode's guard will call the binary and `rules.mjs` goes (a guard spec, not this one). This spec only changes the command word the guards look for; if D13 lands first the opencode edits disappear |
| Secrets | FR-A1..A5; canary test; no value in argv; the `secrets` guard unchanged |
| `shared/secrets.md` | unchanged unless open question 1 is answered yes |
| AGENTS.md / CLAUDE.md | propose first: the edit lists are for approval; the owner runs `scripts/test-reporun-github.py` before the cutover (it stays outside CI) |
| Package allowlist | the Python scripts, `shared/passstore.py` links and `compact/langs.py` leave the allowlist at their cutover; the binary is 032's |

## Group A: pass-store

Decision: a trait `Backend { exists, read_first_line_and_notes, insert }` with `PassCli` and `GopassCli`; `passstore::read` selects by availability, pass first. The redaction registry is process-global behind a mutex.
Parity fixtures: `EntryTest` (3: `test_entry_rules`, `test_notes`, `test_detect_never_decrypts`) and `PassTest` in `test_dbrun.py` (sources in order, prefix override, listing never shows the value, bad or unreadable entry, migrate; the migrate test stays with 038).
Cutover: `shared/passstore.py` and the symlinks `plugins/repo/scripts/passstore.py` and `plugins/db/scripts/passstore.py` go only when `dbrun.py` also leaves Python (038). Until then the Rust module serves `repo` and the Python copy serves `dbrun`; D7's "no dual paths" holds per tool, not per module. References today: `shared/secrets.md` (1), `plugins/db/scripts/dbrun.py` (1), `plugins/repo/scripts/reporun.py` (1).

## Group B: spec

Regex-based parsers: the Rust port uses `regex` with the same patterns (`TASK_RE`, `STORY_HEAD_RE`, `PHASE_STORY_RE`, `LAYER_RE`). `to_html` (specsync.py lines 230-284) is a small Markdown subset converter, ported line by line; a golden test enumerates the constructs.
Parity: 8 tests become 8 fixtures, plus `live-corpus`: `plan` for every `specs/NNN-*` of this repository, with the `specs/` tree stored in the fixture at the commit it was blessed against, so later spec edits do not break it.
Note: the new tasks.md files of 035-040 follow the formats `specsync` reads (`- [ ] Tnnn ...`, `## Phase N: User Story N`); the live-corpus fixture is a free check that they parse.
Cutover (`git grep 'specsync\.py'`: 9 lines in 7 files): `plugins/repo/skills/sync/SKILL.md` (1; `SPECSYNC="python3 <plugin>/scripts/specsync.py"` becomes `SPECSYNC="ai-gent spec"`), `plugins/project/hooks/{pipeline-model.ts (3), pipeline.test.tsx (1), pipeline-fixtures.ts (1)}`, `plugins/project/scripts/test_scripts.py` (1), `scripts/check.sh` (1, the unittest line near 250), `plugins/repo/scripts/test_specsync.py` (1, deleted). Prose mentions of `specsync` (33 lines in 13 files; `plugins/repo/references/sync.md` lines 27, 173, 179) are reworded to `ai-gent spec`. Delete `plugins/repo/scripts/specsync.py` and `test_specsync.py`.

## Group C: repo

Decision: blocking HTTP, no async runtime. The scripted stub listens on `127.0.0.1:0`; platform base URLs are overridable by an environment variable honoured only in test builds (the Python tests monkeypatch `api_base`).
Parity: the suite reports 45 tests: RemoteTest 2, ClassifyTest 2, EntryTest 3, ApiTest 11, CliTest 3, ProjectStatusTest 15, BatchTest 6, ChainTest 3 (a script over the `class` and `def test` lines gave exactly this split on 2026-11-01).
Cutover edit list. `git grep 'reporun\.py'` gives 25 lines in 12 files; the bare word `reporun` appears in 47 files and 237 lines, mostly prose, reworded where it names the script path:
- Skills and references: `plugins/repo/references/runner.md` (2; `REPORUN="python3 <plugin>/scripts/reporun.py"` becomes `REPORUN="ai-gent repo"`), `plugins/repo/skills/sync/SKILL.md` (1), `README.md` (1), `AGENTS.md` (1); prose in `plugins/git/skills/workflow/SKILL.md`, `plugins/git/skills/workflow/references/{issue-linking,landing}.md`, `plugins/devsecops/skills/watch/SKILL.md`, `plugins/project/skills/spec/SKILL.md`, `plugins/repo/references/landing.md`, `plugins/repo/references/platforms/azure-devops.md` and the rest of the 47-file list.
- Guards, changed together in the same release: `plugins/repo/hooks/guard.sh` (the command-word regex on line 30 matches `reporun(\.py)?`; it becomes `ai-gent[[:space:]]+repo`; the ask message on line 38), `opencode/guard/rules.mjs` (regex line 256, message line 274), `opencode/guard/test/rules.test.mjs` (11 mentions), `scripts/test-repo-guard.sh` (1), `shared/guard-ux.ts` (line 91, the `match:` string must equal the guard message) with the copies `plugins/{repo,git,db,devcontainer,secrets}/hooks/guard-ux.ts` regenerated by `scripts/consistency.py --sync-shared-code`, and `plugins/repo/hooks/guard-ux-hook.test.ts` (commands at lines 34-99).
- Mods: `plugins/repo/hooks/ci-model.ts` (2), `ci-model.test.ts` (1), `ci.ts`, `ci.test.tsx`.
- Evals: `plugins/repo/evals/behavior-sync-one-batch/{case.yaml, scaffold.sh}` name `reporun` [not read; check before editing].
- Manifest: `plugins/repo/.claude-plugin/plugin.json`.
- Scripts: `scripts/check.sh` (1), `scripts/test-reporun-github.py` (2): re-point at `ai-gent repo` and run once by the owner against a real repo before cutover.
- Delete: `plugins/repo/scripts/reporun.py` and `test_reporun.py`.

## Group D: compact

Languages today (verified by importing `langs.py`): 56 entries. L4 (24): java, c, cpp, objc, objcpp, cuda, csharp, rust, go, php, dart, proto, glsl, hlsl, solidity, json, javascript, jsx, typescript, tsx, kotlin, css, scss, less. L2 (16): swift, groovy, sql, lua, ruby, r, bash, toml, xml, html, zig, clojure, elixir, verilog, terraform, dockerfile. L1 (9): perl, powershell, scala, python, haskell, fsharp, nim, coffeescript, elm. L0 (7): yaml, make, markdown, rst, pug, sass, stylus. The 36 distinct tree-sitter names it references: bash, c, cpp, csharp, css, cuda, dart, elixir, glsl, go, groovy, haskell, hlsl, html, java, javascript, json, kotlin, lua, objc, php, python, ruby, rust, scala, scss, solidity, sql, swift, toml, tsx, typescript, verilog, xml, yaml, zig. Nineteen entries have none: clojure, coffeescript, dockerfile, elm, fsharp, less, make, markdown, nim, objcpp, perl, powershell, proto, pug, r, rst, sass, stylus, terraform (of these, objcpp, proto and less are L4 on the lexer alone today).

The Python tool is a **Pygments lexer plus an optional tree-sitter check**. Pygments lexers have no Rust equivalent with the same token classes, so a port cannot keep the lexer. Decision for the spike (D0): take tree-sitter leaf nodes as the token stream (comments and strings are nodes) and verify by comparing leaf sequences and statement-level node kinds (the Python `STRUCT_RE` rule). Consequences: a language needs a grammar to go above L1; `go` and `kotlin` grammars "reject some valid one-line code" (`references/languages.md`), so their joins stay verified by the derived rules, as today; `objcpp`, `proto` and `less` drop to L1 unless a grammar is found.

Grammar availability in Rust: the `tree-sitter-language-pack` crate lists 372 languages but fetches parsers on first use and caches them, and its latest docs.rs build failed [RESEARCHED: S1]. Runtime download breaks an offline install and the project's no-surprise-network posture, so the plan compiles grammar crates in. Binary size grows with each grammar (to be measured). Not verified: which of the 36 grammars has a maintained crate compatible with the chosen `tree-sitter` version [ASSUMPTION]; T060 checks all 36 and records the table in `tracker.md`.

What to drop if a grammar is missing, in order: (a) the language falls to L1 (blank lines and trailing whitespace; needs no tokenizer) and `--list-languages` marks it "no grammar"; (b) never claim a level without a verified token stream; (c) if more than 6 of the 24 current L4 languages lose their grammar, stop and take open question 4 to the owner. Keep at L4 first: java, c, cpp, csharp, rust, go, javascript, typescript, tsx, php, kotlin (the SKILL.md description names "Java, C#, C/C++, Go, Rust, JS/TS and more").

Parity: `test_compact.py` has `CompactTest` (4) and `VerifierTest` (5); `test_every_fixture_at_every_level` loops the 17 fixture files over levels 1-4 and compares the reached level to `EXPECTED`. Because the tokenizers differ, the gate is semantic, not byte-equal: (1) reached level is at least Python's, except for languages explicitly dropped; (2) the verifier accepts; (3) token sequences are equal; (4) size within SC-005. Byte equality is required for L0-L2 output (whitespace rules are deterministic) and for the strings-and-comments tests.
Local note: running `python3 -m unittest plugins/compact/skills/code/tests/test_compact.py` bare in this session reported 12 failures and 1 skip (cause not investigated). `scripts/check.sh` runs it through `uv` with Pygments 2.21.0 pinned; the parity oracle must use that environment.

Cutover (`git grep 'compact\.py'`: 20 lines in 8 files): `plugins/compact/skills/code/SKILL.md` (3; the `C="uv run -q --with pygments --with tree-sitter-language-pack python3 <dir>/scripts/compact.py"` line becomes `C="ai-gent compact"` and the sentence "needs Pygments" goes), `references/languages.md` (1; rewrite the table from the Rust `--list-languages`), `references/research.md` (1; keep the measurements), `README.md` (1), `scripts/check.sh` (the compact block near lines 291-320 with the Pygments pin is removed). Delete `scripts/compact.py`, `scripts/langs.py`, `tests/test_compact.py`; `tests/fixtures/` moves to `rust/fixtures/compact/`. The skill description names languages; reword it if the set shrinks.

## Risks

| Risk | Mitigation |
|---|---|
| A secret leaks through a new path (HTTP debug, panic message, `--json`) | FR-A5 canary test; secret-holding types get a redacting `Debug`; a test greps the source for `#[derive(Debug)]` on them |
| Five platforms' auth quirks | the Python tests are the fixtures; one stub per platform |
| Guard patterns in three places drift in the cutover | `scripts/check.sh` already runs the shell and opencode guard fixtures; change all in one PR; D13 may remove one |
| `gopass` unverified | open question 2; a manual Windows gate |
| Compact fidelity without Pygments | spike D0, semantic gate, honest drop list |
| The C port is large | split by platform in tasks, cut over once |
| `test-reporun-github.py` is not in CI | the owner runs it before cutover; recorded in the tracker |

## Phased approach

- **Phase A** (pass-store) then **Phase B** (spec): smallest, unblock the harness.
- **Phase C0** harness, **C1** detect and chain, **C2** cli, **C3** api, **C4** project-status and batch, **C5** guards and cutover.
- **Phase D0** (spike, time-boxed to one day of work): grammar table, binary size, a prototype of rust, java and typescript at L4 with the verifier, decision recorded. Pass: at least 18 of the 24 L4 languages keep L4 and the prototype passes its fixtures; else open question 4 goes to the owner.
- **Phase D1 and on**: port by join-rule family: free, then css, js, go, kotlin.
