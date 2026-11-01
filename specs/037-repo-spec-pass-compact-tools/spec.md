# Feature Specification: Repo, spec, pass-store and compact tools

**Feature Branch**: `docs/specs-rust-tools` (specification only)

**Created**: 2026-11-01

**Status**: Draft

**Input**: `docs/product/rust-decisions.md` (D2, D3, D7, D9, D10, D13), `docs/product/release-3-notes.md`, `shared/secrets.md`, `shared/research.md`, specs 007 (pass-store), 009 (repo-runner), 010 (spec-sync), and the code: `plugins/repo/scripts/reporun.py` (901 lines) with `test_reporun.py` (45 tests), `plugins/repo/scripts/specsync.py` (438 lines) with `test_specsync.py` (8 tests), `shared/passstore.py` (72 lines, linked into `plugins/repo/scripts/` and `plugins/db/scripts/`), `plugins/compact/skills/code/scripts/compact.py` (1291 lines) and `langs.py` (289 lines) with `tests/test_compact.py` (9 tests over 17 fixtures). All Python suites except compact (see below) passed on 2026-11-01.

**Depends on**: 032 (workspace), 035 for the shared fixture harness (`specs/035-token-tools/contracts/parity-fixtures.md`). 038 (db runner spike) reuses the pass-store module this spec builds.

## Context and ground rules

Four tools in one spec. They share only the pass-store module and the cutover procedure, so the stories are grouped (A to D) and ordered by value and risk:

| Group | Tool | Value | Risk | Order |
|---|---|---|---|---|
| A | pass-store module (`ai-gent-core::passstore`, no subcommand) | foundation for B/C and 038 | low code, medium Windows question | 1 |
| B | `ai-gent spec` (specsync) | pipeline read by `repo:sync` and the pipeline pane; no network, no secrets | low | 2 |
| C | `ai-gent repo` (reporun) | highest: every `repo:*` skill, the CI band, the guards | high: secrets, HTTP, five platforms, guard patterns in three places | 3 |
| D | `ai-gent compact` | a token saver, not a pipeline dependency | highest: the Python tool is built on Pygments, which has no Rust equivalent | 4, conditional |

- **Naming**: the request calls them `ai-gent repo` and `ai-gent spec`; D9 lists `repo`, `spec`, `compact`. `passstore` is a library, not a subcommand: `passstore.py` has `check, has, read, write` and nothing a user runs [OBSERVED: `shared/passstore.py`]. A user-facing `ai-gent secrets ...` is not proposed (open question 3).
- **Secrets contract (`shared/secrets.md`)**: pass first, plain-text file last, and **only a runner reads a value**. In the binary, "runner" means the `repo` subcommand (and `db`, later); no other subcommand, log line, panic message, `--json` output or error may contain a secret. A value goes to one client/API call via stdin, a mode-600 file or that process's environment, never argv. Decrypt late: `detect`, classification and planning never decrypt (test `test_detect_never_decrypts`).
- Claude Code vs others: none of these four depends on Claude Code. `compact` and `repo` are used by Codex and Tier-3 agents through skills, so a missing binary must fail with the install command (D8) in every tier.
- Windows (D3): see FR-A4 for pass, FR-C9 for platform CLIs, FR-D6 for compact.

## User Scenarios & Testing *(mandatory)*

### Group A: pass-store module

#### User Story A1 - Read and check entries without leaking (Priority: P1)

Rust code asks `passstore::has(entry)` (a file check, no passphrase), `read(entry)` (first line is the secret, later lines are notes) and `write(entry, value)` (`pass insert --multiline`, never overwrites). Names obey `ENTRY_RE`, forbid `..`, leading `-` or `/`. `AI_GENT_PASS_PREFIX` and `PASSWORD_STORE_DIR` are honoured.

**Independent Test**: fixtures from `EntryTest` (3 tests) in `test_reporun.py` and `PassTest` (6 tests) in `test_dbrun.py`, run against a fake `pass` executable and a temp store.

**Acceptance Scenarios**:
1. **Given** an invalid name (`../x`, `-x`, `/x`), **Then** `check` fails with a message that has no value.
2. **Given** `<store>/<entry>.gpg` exists, **Then** `has` is true without running `pass`.
3. **Given** `pass show` exits non-zero, times out (90 s) or returns an empty first line, **Then** `read` fails with a message ending with "unlock it in your own terminal" and no stderr value.
4. **Given** `write` on an existing entry, **Then** it fails with "already exists".

#### User Story A2 - Windows without pass in PATH (Priority: P2)

On native Windows the module finds a backend: `pass` (Git Bash or WSL shim) when on PATH, else `gopass` [RESEARCHED: S4, gopass installs on Windows with `winget install gopass.gopass` plus Git and Gpg4win, and can use age instead of GPG]. When neither exists, `read` fails with "pass is not installed" and the install pointer, and the plain-text-file path of the tool remains the documented last resort.

**Independent Test**: unit tests with fake `pass`/`gopass` scripts (`.cmd` on Windows) on the Windows runner.

**Acceptance Scenarios**:
1. **Given** only `gopass` on PATH, **Then** `read` runs the gopass command that prints the first line only [ASSUMPTION: `gopass show -o ENTRY`; verify the flag and that it never prompts on stdin] and `has` also checks the gopass store layout.
2. **Given** both, **Then** `pass` wins (contract says pass).

### Group B: `ai-gent spec`

#### User Story B1 - Plan, record, features (Priority: P1)

`ai-gent spec plan <NNN|specs/NNN-feature> [--json]` prints the items a feature implies (one per feature, one per user story with its tasks as a checklist, each with a state); `record` writes a batch's results into `tracker.md`; `features` lists every feature's state and pending changes.

**Independent Test**: the 8 tests of `test_specsync.py`: `test_items_and_story_assignment`, `test_states`, `test_all_tasks_checked_is_review`, `test_tracker_round_trip_and_actions`, `test_orphans_reported`, `test_retired_from_notes`, `test_introspec_story_list`, `test_status_with_notes_after_it`.

**Acceptance Scenarios**:
1. A story is `todo` until one task is checked, `doing` while some are, `review` when all are, `done` when the feature is Verified; the feature follows `specs/README.md` (Planned to todo; In progress/Partial to doing; Implemented to review; Verified to done; the least advanced side wins).
2. Items carry the marker `<!-- ai-gent:sync {path} {key} -->` and a content digest; unchanged items yield no action.
3. This repository's own `specs/` (including the new tasks.md files of 035-040) parses with the same output as Python (a live-corpus fixture).

### Group C: `ai-gent repo`

#### User Story C1 - Detect without decrypting (Priority: P1)

`ai-gent repo detect [--json]` reports the platform, the CLI and the token state (entry present or not) for this repository; nothing is decrypted.

**Independent Test**: `RemoteTest` (2 tests: `test_platforms`, `test_forced_platform`), `EntryTest::test_detect_never_decrypts`.

**Acceptance Scenarios**: GitHub, Azure DevOps (several URL forms), GitLab, Bitbucket, Forgejo/Gitea/Codeberg, Jira remotes are recognised; `git config` overrides force a platform.

#### User Story C2 - Run the platform CLI with the token in its environment (Priority: P1)

`ai-gent repo cli <platform> [--write] -- <args>` runs `gh`, `az`, `glab` or `jira` with the token in that process's environment only; reads need no flag, writes need `--write`; token-printing and arbitrary-code subcommands are blocked; output is redacted.

**Independent Test**: `ClassifyTest::test_cli`, `CliTest` (3 tests).

**Acceptance Scenarios**: classification by READ_VERBS/`BLOCKED_CLI`; `--write` required otherwise (exit 3 "refused"); the secret never appears in argv, stdout, stderr or errors.

#### User Story C3 - REST API with auth built in-process (Priority: P1)

`ai-gent repo api <platform> METHOD PATH [--data FILE|-] [--query k=v ...] [--write] [--paginate] [--max-pages N]`, with `{placeholders}` filled from the remote, bearer/basic/token headers per platform, `Link`/next-page following, write detection by method and by GraphQL body, hosts restricted to the platform's own.

**Independent Test**: `ApiTest` (11 tests) against a local HTTP stub (`Stub(http.server.BaseHTTPRequestHandler)`, lines 132-176).

**Acceptance Scenarios**: GitHub bearer, Azure basic + `api-version`, Forgejo `token` header, Jira basic with email, GitLab path-encoded project, Bitbucket pagination, other hosts refused, errors redacted, missing entry reported with the `pass insert` hint, `Forgejo` has no CLI.

#### User Story C4 - Project status, batch and chain (Priority: P2)

`project-status` (GitHub Projects v2 field and option matching, case- and punctuation-insensitive, aliases), `batch FILE [--write] [--dry-run] [--out FILE]` (several calls from a JSON plan with `{results.name.field}` references, one confirmation, stops at the first failure), `chain [--json]` (work, integration, staging?, production).

**Independent Test**: `ProjectStatusTest` (15), `BatchTest` (6), `ChainTest` (3).

### Group D: `ai-gent compact`

#### User Story D1 - Compact view of the languages that matter (Priority: P2)

`ai-gent compact [-l LANG] [-L 0..4] [-c] [--outline DEPTH] [-n] [-w N] [-o FILE] [-q] [--no-verify] [--no-ast] [--list-languages] PATH...` prints source with indentation, newlines and optional spaces removed, strings and comments kept, every result verified, with automatic fallback to a safer level.

**Independent Test**: `CompactTest` (4), `VerifierTest` (5) and the 17 fixture files in `plugins/compact/skills/code/tests/fixtures/` at levels 1-4.

**Acceptance Scenarios**:
1. Per-language maximum level honoured (24 languages at L4, 16 at L2, 9 at L1, 7 at L0, from `LANGS`); a request above is capped silently.
2. Strings and comments survive byte for byte (`test_strings_and_comments_survive`); outline line numbers point at the original lines (`test_outline_numbers_point_at_the_original_lines`).
3. A verification failure steps down one flag at a time in the order spaces, join, indent, blank and says `fallback[join:ast]` on stderr.
4. Read-only: no file is written except `-o` (`test_read_only`).

## Requirements *(mandatory)*

### Group A

- **FR-A1**: `ai-gent-core::passstore` provides `prefix()`, `store()`, `check()`, `has()`, `read()`, `write()` with the Python semantics and error text; `StoreError` messages never hold a value.
- **FR-A2**: `read` runs `pass show ENTRY` with stdin closed and a 90 s timeout; `write` runs `pass insert --multiline ENTRY` with the value on stdin and a 60 s timeout; neither puts a value in argv or the environment.
- **FR-A3**: Redaction: a process-wide registry of read secrets (the Python `SECRETS` list) applied to every string that leaves the process (stdout, stderr, logs, errors, JSON).
- **FR-A4**: Windows: backend order `pass`, then `gopass`; entry file check handles `.gpg` and `.age`; store directory from `PASSWORD_STORE_DIR`, else `~/.password-store`, else gopass's own location [ASSUMPTION: unverified where gopass keeps its store on Windows]. Windows Credential Manager or DPAPI are **not** added without changing `shared/secrets.md` (open question 1).
- **FR-A5**: A canary test: every `repo` fixture runs with a distinctive fake token and the harness greps stdout, stderr, the batch `--out` file and the process list for it; only the single intended sink (the child's environment or the HTTP header on the stub) may contain it.

### Group B

- **FR-B1**: `plan`, `record`, `features` with the flags of `specsync.py` and the JSON of `build_plan`; exit codes 0, 1 (`Failed`), 2 (usage).
- **FR-B2**: Parsing as Python: stories from `### User Story N - Title (Priority: Pn)` (also the introspec list form), tasks `- [ ] T001 ...` with phase/story headings and `## Frontend|Backend|QA` layers, README rows, `tracker.md` read/write.
- **FR-B3**: `to_html` (Markdown to HTML subset for tracker bodies) behaves identically; a golden test covers every construct the Python supports.
- **FR-B4**: Digest `sha` as Python's `digest()` (algorithm to be read from the source at T-B1; fixtures compare).
- **FR-B5**: Reads only `specs/`; writes only `specs/NNN-*/tracker.md`.

### Group C

- **FR-C1**: Subcommands `detect, cli, api, project-status, batch, chain` and flags as `contracts/cli.md`.
- **FR-C2**: Exit codes `0` ok, `1` error, `2` usage, `3` refused (a write without `--write`, a blocked CLI verb, a host not on the allowlist).
- **FR-C3**: Platforms table (`PLATFORMS`): github `gh` `ai-gent/github/pat` env `GH_TOKEN`; azure-devops `az` `ai-gent/azure-devops/pat` env `AZURE_DEVOPS_EXT_PAT`; gitlab `glab` `ai-gent/gitlab/pat` env `GITLAB_TOKEN`; bitbucket no CLI `ai-gent/bitbucket/token`; forgejo no CLI `ai-gent/forgejo/pat`; jira `jira` `ai-gent/jira/token` env `JIRA_API_TOKEN`.
- **FR-C4**: HTTP client: blocking, rustls, 30 s default timeout [TO CONFIRM against `request()` in the source], redirects not followed across hosts, response bodies redacted before they are shown or stored.
- **FR-C5**: Write detection for `api` (method not GET/HEAD, GraphQL `mutation`, `step_is_write` for batch steps) and `cli` (`cli_is_write`), identical to Python on the 45 tests' cases.
- **FR-C6**: `batch` records results by name for later steps (`lookup`/`resolve`), prints the plan with `--dry-run` calling nothing, needs `--write` once if any step writes, stops at the first failure and still reports what completed.
- **FR-C7**: Guards updated in the same release (see cutover): the shell guard and the opencode rules recognise `ai-gent repo ... --write` as the command word and decide as before (ask; deny with a sign-off in the text).
- **FR-C8**: The CI band mod (`plugins/repo/hooks/ci.ts`, `ci-model.ts`, 023) calls `ai-gent repo` and degrades when absent.
- **FR-C9**: Windows: the platform CLIs are looked up as `gh.exe` etc.; the token goes into the child's environment block; no shell is involved (arguments passed as an array).
- **FR-C10**: No logging of request URLs with credentials in them; query values that look like tokens are redacted.

### Group D

- **FR-D1**: `ai-gent compact` flags as the Python `argparse` (`contracts/cli.md`), exit codes 0 ok, 1 usage/IO error, 2 missing dependency (kept; in the binary 2 is not used because there are no optional dependencies, see FR-D4).
- **FR-D2**: Levels 0-4 and flags `blank, indent, join, spaces` with per-language `allowed` sets, `join` rule (`free, go, js, kotlin, css`), `hazard` regexes and `protect` patterns copied as data from `langs.py` into a generated Rust table (a build-time conversion script `scripts/gen-compact-langs.py`, kept in the repo, verified equal in `check.sh`).
- **FR-D3**: Verification by token-sequence equality: the original and the compacted text are tokenised by the same tokenizer and compared with whitespace ignored (except where it is token content); with a grammar, additionally compare the AST leaf and structure sequence.
- **FR-D4**: Tokenizer strategy (decision in plan): a tree-sitter grammar per language supplies the tokens (leaf nodes); languages without a grammar are capped at L1 (blank lines and trailing whitespace only, which needs no tokenizer). The Pygments dependency disappears; so does the "needs Pygments" exit 2.
- **FR-D5**: Language coverage is data: `--list-languages` prints the live table with a column saying which tokenizer backs each language, and a language whose grammar was dropped in a build shows its reduced level.
- **FR-D6**: Windows: reads files as UTF-8 with BOM tolerance, keeps `\r\n` inside strings and comments, and normalises directory walking order (sorted) so output is identical across OSes.

## Success Criteria *(mandatory)*

- **SC-001**: B: all 8 specsync tests and the live-corpus fixture are shared fixtures green on both implementations before `specsync.py` is deleted.
- **SC-002**: C: all 45 reporun tests are fixtures green on both, including the canary test (FR-A5), before `reporun.py` is deleted; `scripts/test-reporun-github.py` is ported or re-pointed at the binary and run once by the owner before the cutover (it is not part of CI).
- **SC-003**: A: `PassTest` and `EntryTest` fixtures green; the Windows gopass path verified on a Windows machine with a real store (manual, recorded in the tracker).
- **SC-004**: D: for every language kept at its current level, `test_every_fixture_at_every_level` passes with zero fallbacks; languages dropped to L1 are listed in the release notes.
- **SC-005**: D: on the 17 fixtures and on this repository's own source files, the compact output of the Rust tool is no larger than 100.5% of the Python tool's for the same language and level (token saving not worse); measured, not assumed.
- **SC-006**: After cutover the greps in the plan are empty for `reporun\.py`, `specsync\.py`, `compact\.py`, `passstore\.py`; `scripts/test-repo-guard.sh` and `opencode/guard/test/rules.test.mjs` pass with the new command word.

## Edge Cases

- A token entry has Windows line endings (`\r\n`): the first line is trimmed.
- `pass` asks for a passphrase via a GUI pinentry: the 90 s timeout applies; the error suggests unlocking in a terminal.
- A remote URL with credentials embedded (`https://user:tok@host/...`): the userinfo is stripped before any display or log.
- Jira Server vs Cloud (`url:` and `email:` notes decide); Azure `org:` note.
- API pagination over a `Link` header with a cross-host `next` URL: refused.
- `batch` file referencing a result that failed or does not exist: error before any write.
- Compact: a file in a language with a grammar that fails to parse the original: output at L1 only, `fallback[parse]` on stderr.
- Compact: binary or non-UTF-8 files: skipped with a note, exit 0 for the rest.
- Compact: a directory walk hits a symlink loop: not followed.

## Out of Scope

- A native replacement of `gh`, `az`, `glab` or `jira` (the runner still calls them), OAuth flows, token creation, a keychain backend (open question 1), `ai-gent secrets` (open question 3), formatting or rewriting files in compact (`fmt` list in `langs.py` is unused in the read-only copy), any change to the skills' behaviour.

## Open Questions

1. **Windows secret store**: add Windows Credential Manager as a documented backend, or require `pass`/`gopass` there? Recommended: no; support `pass` and `gopass` only, keep the contract unchanged, and document WSL as the supported route if gopass proves incompatible. A new backend is a contract change for the owner.
2. **gopass compatibility** with a pass-created store (same `.gpg` layout, `show -o` semantics) is [ASSUMPTION], not verified. Needs a Windows run (T006).
3. **A user-facing `ai-gent secrets`** (check/list names) versus keeping `secrets:pass` as the guide. Recommended: not now.
4. **Compact fate**: port, port with a reduced language set, or leave `compact` out of 3.0.0 and keep it as an optional skill that needs the Python script. The last conflicts with D8/D16 (no Python at runtime), so the real choice is port-reduced or drop the skill. Recommended: port with the reduced set (grammar-backed languages only; others L1), decide after the Phase D0 spike, which gives pass/fail numbers.
5. **Compact tokens**: is a 0.5% larger output acceptable (SC-005) when the saving moves from Pygments-style lexing to tree-sitter tokens? Recommended: yes up to 1%.
6. **HTTP crate and TLS roots** (reqwest vs ureq, platform roots vs bundled) are 032's dependency policy; this spec only needs blocking HTTPS with a custom host allowlist.
7. **Which release** each group cuts over in: recommended with 035/036 at 3.0.0, groups in order A, B, C, D (D may slip to a later 3.x).

## Sources (web research, retrieved 2026-11-01)

| # | Claim | URL | Confidence | Label |
|---|---|---|---|---|
| S1 | `tree-sitter-language-pack` Rust crate lists 372 languages (latest 1.22.1, 2026-10-12), MIT; parsers are pre-compiled but fetched on first use and cached, not bundled; docs.rs failed to build the latest version; the page does not list the languages | https://docs.rs/crate/tree-sitter-language-pack | Medium (one source, no language list) | [RESEARCHED: S1] |
| S4 | gopass supports Linux, macOS and Windows (winget: Git.Git, GnuPG.Gpg4win, gopass.gopass); most users need gpg and git, but `gopass setup --crypto age` uses age; the page does not say it reads a pass-format store | https://www.gopass.pw/llms.txt | Medium | [RESEARCHED: S4] |

Individual tree-sitter grammar crates for most languages in the compact table (rust, java, c, cpp, c-sharp, go, javascript, typescript, python, ruby, php, css, json, bash, html, yaml, toml) exist on crates.io [KNOWN]; coverage for `glsl, hlsl, solidity, verilog, zig, dart, kotlin, scss, objc, cuda, groovy, elixir, swift, scala, lua, haskell, sql, xml` as Rust crates was **not verified** here [ASSUMPTION]; T060 checks each.
