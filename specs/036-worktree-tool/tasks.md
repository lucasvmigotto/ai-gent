# Tasks: Worktree tool (`ai-gent wt`)

Format: `- [ ] Tnnn [P] [USn] description. Files. Verify. Fixture.` Executor Sonnet builder, Reviewer Opus inline. `rust/` paths follow D4 (names assumed until 032 lands).

## Phase 0: Harness

- [ ] T001 Fake engine binary. A Rust port of `FAKE_ENGINE` in `plugins/agents/scripts/test_wt.py` (lines 29-100: `ps`, `images`, `volume ls/inspect/rm`, `rm`, `rmi`, `info`, `system df`, `ps -a --external`; fails on any `prune`; records calls). Files: `rust/ai-gent/src/bin/fake_engine.rs`, `rust/ai-gent/Cargo.toml` (feature `test-support`). Verify: `cargo test -p ai-gent --features test-support fake_engine`. Fixture: harness for all.
- [ ] T002 [P] Fixture map. List the 16 tests into `specs/036-worktree-tool/fixture-map.md`. Verify: `python3 -m unittest plugins/agents/scripts/test_wt.py` prints `Ran 16 tests`; map has 16 rows. Fixture: all.
- [ ] T003 Python oracle runner `scripts/test-parity-wt.py` (reuses 035 T003's harness). Verify: `--self-test`. Fixture: harness.

## Phase 1: Plan (US4)

- [ ] T010 [P] [US4] `readme.rs`: `readme_rows`, `level`, `plan` incl. JSON shape. Files: `rust/ai-gent-core/src/wt/readme.rs`. Verify: `cargo test wt::readme`. Fixture: `test_plan`, `test_plan_reads_the_worktree_it_runs_in`.
- [ ] T011 [US4] `ai-gent wt plan` text and `--json`. Files: `rust/ai-gent/src/cmd/wt.rs`. Verify: parity group `plan-*`. Fixture: same.

## Phase 2: Add and list (US1)

- [ ] T020 [P] [US1] `git.rs`: runner, `root`, `here`, `chain_dev`, `exclude`, porcelain parser incl. Windows drive paths. Verify: `cargo test wt::git`. Fixture: `test_from_inside_a_worktree`.
- [ ] T021 [P] [US1] `ports.rs` and `Settings`. Verify: `cargo test wt::ports`. Fixture: `test_offsets_never_reuse_a_live_one`, `test_add_reads_settings_and_caps_volumes` (settings part).
- [ ] T022 [US1] `add` without engine, `list`, text and JSON. Verify: parity groups `add-*`, `list-*`. Fixture: `test_add_list_remove`, `test_refusals`, `test_list_json_keeps_the_keys_the_worktree_board_reads` (also compares `plugins/agents/hooks/worktrees-fixtures.ts`).

## Phase 3: Engine, disk floor, remove (US2, US3)

- [ ] T030 [P] [US2] `engine.rs`: selection, `ctr`, `ids`, `owned`, `release`. Verify: `cargo test wt::engine` asserting no `prune` and label filters. Fixture: SC-003 check.
- [ ] T031 [P] [US2] `disk.rs`: `store_usage` (GraphRoot, DockerRootDir, fallback), `preflight`, floor rules, test hook for usage. Verify: `cargo test wt::disk`. Fixture: `test_add_prints_labels_and_checks_disk`.
- [ ] T032 [US2] `add` with engine: volume cap, floor, `system df` on stderr. Verify: parity `add-disk-*`. Fixture: `test_add_reads_settings_and_caps_volumes`.
- [ ] T033 [US3] `remove`: clean check, `git cherry`, release, recount, `-d`/`-D`, prune, port offset. Verify: parity `remove-*`. Fixture: `test_remove_after_a_rebase_merge`, `test_remove_releases_the_tracks_containers_and_volumes`, `test_remove_keeps_the_worktree_when_a_volume_stays`.

## Phase 4: Resources and finish (US5)

- [ ] T040 [P] [US5] `procs.rs`: builds running on Linux (`/proc`), macOS and Windows (process list), test hook. Verify: `cargo test wt::procs`. Fixture: `test_finish_leaves_buildah_containers_while_a_build_runs`.
- [ ] T041 [US5] `baseline`, `resources`, `finish`. Verify: parity `resources-*`, `finish-*`, `no-engine`. Fixture: `test_resources_reports_orphans_and_never_removes`, `test_finish_removes_dangling_and_new_buildah_containers_only`, `test_without_an_engine_containers_are_skipped`.

## Phase 5: Platforms

- [ ] T050 [P] Windows fixtures: `core.symlinks=false` warning, long path warning, backslash and drive-letter porcelain, branch with `/`. Verify: Windows CI job. Fixture: new `win-*`.
- [ ] T051 [P] macOS run: porcelain, `/private/var` temp paths, process list. Verify: macOS CI job. Fixture: parity groups.
- [ ] T052 Manual check against real podman: add, list, remove with a real labelled volume (owner machine; records output in tracker). Verify: tracker entry. Fixture: n/a.

## Phase 6: Cutover (gated on SC-001..SC-004, owner approval of the file list)

- [ ] T060 Edit skills, references, `shared/containers.md`, README (plan list 1). Verify: `scripts/check.sh --quick`.
- [ ] T061 Switch mods, hooks, types and their tests/fixtures; `leftovers.sh` mention (plan list 2). Verify: `claude plugin test` runs inside `scripts/check.sh`.
- [ ] T062 Edit `scripts/check.sh`, `scripts/consistency.py`, `scripts/test_consistency.py`, `plugins/project/scripts/test_scripts.py`. Verify: `scripts/check.sh`.
- [ ] T063 Delete `wt.py`, `test_wt.py`, `scripts/test-parity-wt.py`; package allowlist. Verify: `git grep -n 'wt\.py' -- . ':!CHANGELOG.md' ':!specs' ':!docs/product'` empty; `python3 scripts/package.py --check`.
- [ ] T064 `CHANGELOG.md` Unreleased note; README row stays Draft/Planned until released.
