# Tasks: Repo, spec, pass-store and compact tools

Format: `- [ ] Tnnn [P] [Story] description. Files. Verify. Fixture.` Groups in value-and-risk order: A, B, C, D. Executor Sonnet builder, Reviewer Opus inline (C security review: Opus reviewer, fresh context). `rust/` paths follow D4 and are assumed until 032 lands.

## Phase 0: Shared harness

- [ ] T001 Reuse the 035 fixture harness (`specs/035-token-tools/contracts/parity-fixtures.md`); add the portable fake-tool binary (`pass`, `gh`, `az`, `glab`, `jira` stand-ins keyed by file name, state in JSON, records argv and environment names but not values). Files: `rust/ai-gent/src/bin/fake_tool.rs`. Verify: `cargo test -p ai-gent --features test-support fake_tool`. Fixture: harness for A and C.
- [ ] T002 [P] Fixture maps: list the tests of `test_reporun.py` (45), `test_specsync.py` (8), `EntryTest` + `PassTest`, `test_compact.py` (9) into `specs/037-repo-spec-pass-compact-tools/fixture-map.md`. Verify: `python3 -m unittest` for each prints the counts. Fixture: all.

## Group A: pass-store (Phase A)

- [ ] T003 [US1] `passstore::{prefix, store, check, has, read, write}` with `ENTRY_RE`, timeouts, error text. Files: `rust/ai-gent-core/src/passstore/{mod,pass}.rs`. Verify: `cargo test passstore`. Fixture: `EntryTest::test_entry_rules`, `::test_notes`, `PassTest::test_bad_or_unreadable_entry`, `::test_prefix_override`.
- [ ] T004 [US1] Redaction registry and the secret-holding wrapper type (redacting `Debug`/`Display`). Files: `passstore/redact.rs`. Verify: `cargo test passstore::redact`; a test fails if a secret wrapper derives `Debug`. Fixture: `RedactTest` (test_dbrun.py lines 189-211) behaviour for repo.
- [ ] T005 [US2] Gopass backend and backend selection (pass first); entry file check for `.gpg` and `.age`. Files: `passstore/gopass.rs`. Verify: `cargo test passstore::gopass` with fake executables. Fixture: new `passstore-gopass-*`.
- [ ] T006 [US2] Manual Windows gate (open question 2): on a Windows machine with Git, Gpg4win and gopass, verify read of an entry created with `pass`, the flag that prints only the first line, and the store location; record in `tracker.md`. Verify: tracker entry. Fixture: n/a. Blocked until a Windows machine is available.

## Group B: spec (Phase B)

- [ ] T010 [US3] Parse: `parse_spec`, `listed_stories`, `parse_tasks`, `readme_row`, `status_word`, `feature_state`, `tasks_state`; read the `digest()` algorithm. Files: `rust/ai-gent-core/src/spec/{parse,readme,digest}.rs`. Verify: `cargo test spec::parse`. Fixture: `test_items_and_story_assignment`, `test_states`, `test_all_tasks_checked_is_review`, `test_introspec_story_list`, `test_status_with_notes_after_it`, `test_retired_from_notes`.
- [ ] T011 [P] [B1] `to_html` Markdown subset with a construct-by-construct golden test. Files: `spec/html.rs`. Verify: `cargo test spec::html`. Fixture: golden cases from `specsync.py` lines 230-284.
- [ ] T012 [US3] `build_plan`, `plan`, `record`, `features`, tracker read/write; clap and `--json`. Files: `spec/{plan,tracker}.rs`, `rust/ai-gent/src/cmd/spec.rs`. Verify: parity group `spec-*`. Fixture: `test_tracker_round_trip_and_actions`, `test_orphans_reported`.
- [ ] T013 [US3] Live-corpus fixture: bless `plan` and `features` over this repository's `specs/` at a pinned commit. Files: `rust/fixtures/spec/live-corpus/`. Verify: parity runner green on both. Fixture: live-corpus.
- [ ] T014 [US3] Cutover B (plan list): edit `plugins/repo/skills/sync/SKILL.md`, `plugins/repo/references/sync.md`, `plugins/project/hooks/{pipeline-model.ts, pipeline.test.tsx, pipeline-fixtures.ts}`, `plugins/project/scripts/test_scripts.py`, `scripts/check.sh`; delete `specsync.py`, `test_specsync.py`. Verify: `git grep -n 'specsync\.py' -- . ':!CHANGELOG.md' ':!specs' ':!docs/product'` empty; `scripts/check.sh`. Gated on SC-001.

## Group C: repo (Phase C)

- [ ] T020 [US4] Platforms table, remote parsing (`parse_remote`, `detect_repo`, `git_config`), `detect` text and `--json`. Files: `rust/ai-gent-core/src/repo/{platforms,remote}.rs`, `cmd/repo.rs`. Verify: parity `repo-detect-*`. Fixture: `RemoteTest` (2), `EntryTest::test_detect_never_decrypts`.
- [ ] T021 [P] [C4] `chain` (branch chain detection, ambiguity asks). Files: `repo/chain.rs`. Verify: parity `repo-chain-*`. Fixture: `ChainTest` (3).
- [ ] T022 [US5] `cli_is_write`, blocked verbs, `prepare_cli`, run with the token in the child's environment only, redaction of output. Files: `repo/cli.rs`. Verify: parity `repo-cli-*` and the canary test. Fixture: `ClassifyTest::test_cli`, `CliTest` (3).
- [ ] T023 [US6] HTTP layer: blocking client, host allowlist, header builders per platform, Link pagination, error redaction. Files: `repo/{http,api}.rs`. Verify: `cargo test repo::http` against the Rust stub. Fixture: `ApiTest` (11), `ClassifyTest::test_api`.
- [ ] T024 [US6] `api` command: placeholders, `--data`, `--query`, `--paginate`, `--max-pages`, write detection. Files: `repo/api.rs`, `cmd/repo.rs`. Verify: parity `repo-api-*`. Fixture: `ApiTest`.
- [ ] T025 [US7] `project-status`: field/option matching (`_project_norm`, aliases), owner default from the remote. Files: `repo/project_status.rs`. Verify: parity `repo-projstatus-*`. Fixture: `ProjectStatusTest` (15).
- [ ] T026 [US7] `batch`: plan file, `lookup`/`resolve`, `--dry-run`, `--out`, one `--write`, stop at first failure. Files: `repo/batch.rs`. Verify: parity `repo-batch-*`. Fixture: `BatchTest` (6).
- [ ] T027 [C] Canary secret test over every `repo` fixture (FR-A5) and process-list check. Files: `rust/ai-gent/tests/canary_repo.rs`. Verify: `cargo test canary_repo`. Fixture: new `repo-canary-*`.
- [ ] T028 [C] Owner gate: run `scripts/test-reporun-github.py` re-pointed at `ai-gent repo` against a real GitHub repo; record the result in `tracker.md`. Verify: tracker entry. Needs the owner's token in pass; the assistant never handles it.
- [ ] T029 [C5] Cutover C (plan list): skills and references, guards (`guard.sh`, `rules.mjs`, `rules.test.mjs`, `test-repo-guard.sh`, `shared/guard-ux.ts` + `--sync-shared-code`, `guard-ux-hook.test.ts`), mods, evals, manifest, `scripts/check.sh`, `scripts/test-reporun-github.py`; delete `reporun.py`, `test_reporun.py`. Verify: `scripts/check.sh` (runs the guard parity test); `git grep -n 'reporun\.py' -- . ':!CHANGELOG.md' ':!specs' ':!docs/product'` empty. Gated on SC-002 and T028.

## Group D: compact (Phase D; D0 first)

- [ ] T060 [US8] Grammar table: for each of the 36 tree-sitter names, find the Rust crate, version, `tree-sitter` compatibility, license, size added to a release binary; write `specs/037-repo-spec-pass-compact-tools/grammar-table.md`. Verify: 36 rows, each with a verdict yes/no/unknown. Fixture: n/a.
- [ ] T061 [US8] Prototype: tokenizer from leaf nodes plus verifier for rust, java and typescript at L4 on the matching fixtures (`cache.rs`, `Inventory.java`, `service.ts`, `Widget.tsx`). Files: `rust/ai-gent-core/src/compact/{tokenize,verify,render}.rs`. Verify: `cargo test compact::proto`. Fixture: `test_every_fixture_at_every_level` for those files.
- [ ] T062 [US8] Spike verdict against the pass/fail rule (>= 18 of 24 L4 languages keep L4, prototype green); write it in `tracker.md`; if it fails stop and ask the owner (open question 4). Verify: tracker entry.
- [ ] T063 [P] [D1] Generated language table: `scripts/gen-compact-langs.py` converts `langs.py` into `rust/ai-gent-core/data/compact-langs.rs`; `scripts/consistency.py` check that it is current while `langs.py` exists. Verify: `python3 scripts/gen-compact-langs.py --check`. Fixture: `--list-languages` golden.
- [ ] T064 [US8] Join-rule family `free` (java, c, cpp, objc, cuda, csharp, rust, php, dart, glsl, hlsl, solidity, json), levels 1-4, outline, `-n`. Files: `compact/{render,outline}.rs`. Verify: parity `compact-free-*`. Fixture: `CompactTest` (4), the fixtures for those languages.
- [ ] T065 [US8] Families `css`, `js`, `go`, `kotlin`. Files: `compact/render.rs` (rules), per-family modules. Verify: parity `compact-css-*`, `-js-*`, `-go-*`, `-kt-*`. Fixture: `VerifierTest` (5), `site.css`, `theme.scss`, `app.mjs`, `nosemi.mjs`, `server.go`, `Shapes.kt`.
- [ ] T066 [US8] CLI: paths, directories (sorted walk), stdin with `-l`, `-o`, `-q`, `--no-verify`, `--no-ast`, `--list-languages`; read-only guarantee. Files: `cmd/compact.rs`, `compact/walk.rs`. Verify: parity `compact-cli-*`. Fixture: `test_read_only`.
- [ ] T067 [US8] SC-005 size comparison script over the 17 fixtures and this repository's sources. Files: `scripts/compare-compact.py`. Verify: prints per-language ratio; all <= 1.005. Fixture: n/a.
- [ ] T068 [US8] Cutover D (plan list). Verify: `git grep -n 'compact\.py' -- . ':!CHANGELOG.md' ':!specs' ':!docs/product'` empty; `scripts/check.sh`. Gated on SC-004, SC-005.
