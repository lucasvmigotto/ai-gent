# Tasks: dbrun spike

Format: `- [ ] Tnnn [P] [USn] description. Files. Verify. Experiment.` The deliverable is `decision.md`; code is throwaway. Executor Sonnet builder, Reviewer Opus inline.

## Phase 1: Core and survey

- [ ] T001 [US3] E1: port the driver-free safety core and turn the 16 tests into fixtures. Files: `rust/spikes/dbrun/src/core/`, `rust/fixtures/db-core/`. Verify: `cargo test -p dbrun-spike core` and `python3 -m unittest plugins/db/scripts/test_dbrun.py` still green. Experiment: E1.
- [ ] T002 [P] [US2] Survey Rust drivers for the six engines (crate, maintainer activity, native library needs, TLS, timeouts) into `engine-matrix.md`; web research per `shared/research.md`, claims labelled. Files: `specs/038-db-runner-spike/engine-matrix.md`. Verify: six rows, no empty cell. Experiment: input to E2-E6.
- [ ] T003 [P] Make `scripts/test-dbrun-engines.py` accept `--runner PATH` (default the Python `dbrun`), default behaviour unchanged. Files: `scripts/test-dbrun-engines.py`. Verify: `python3 scripts/test-dbrun-engines.py --engine postgresql` unchanged (owner approves the image pull). Experiment: harness.

## Phase 2: Native candidates

- [ ] T004 [US2] E4 sqlite prototype and the 8 `SqliteEndToEndTest` cases. Files: `rust/spikes/dbrun/src/drivers/sqlite.rs`. Verify: `cargo test -p dbrun-spike sqlite`. Experiment: E4.
- [ ] T005 [US2] E2 PostgreSQL prototype against `postgres:17` through the adapter. Files: `drivers/pg.rs`, `src/bin/adapter.rs`. Verify: `python3 scripts/test-dbrun-engines.py --runner target/debug/adapter --engine postgresql`. Experiment: E2.
- [ ] T006 [US2] E3 MySQL and MariaDB prototype. Files: `drivers/mysql.rs`. Verify: the same script with `--engine mysql` then `--engine mariadb`. Experiment: E3.

## Phase 3: Hard engines

- [ ] T007 [US2] E5 SQL Server: best candidate versus `sqlcmd` in a client image. Files: `drivers/mssql.rs`. Verify: engines script `--engine sqlserver` (2 GB cap, one engine at a time). Experiment: E5.
- [ ] T008 [US2] E6 Oracle: the same, thin versus Instant Client. Files: `drivers/oracle.rs`. Verify: engines script `--engine oracle`. Experiment: E6.

## Phase 4: Platforms, size, verdict

- [ ] T009 [P] [US2] E8: build matrix of driver sets, record binary sizes and build times (Linux x86_64 and aarch64). Files: `results.md`. Verify: numbers in the table. Experiment: E8.
- [ ] T010 [P] E7: Windows run of E1 and E4 and the container namespace join (the owner's Windows machine or a CI runner); mark not run if unavailable. Files: `results.md`. Verify: section filled. Experiment: E7.
- [ ] T011 [US1] Write `decision.md` from the template, list the cutover edits if the verdict is not S, fill "If the answer is not yet". Files: `specs/038-db-runner-spike/decision.md`. Verify: `grep -c TBD decision.md` is 0; SC-001 checklist. Experiment: all.
- [ ] T012 [US1] Record the owner's sign-off; update the row 038 in `specs/README.md`. Verify: `scripts/check.sh --quick`.
