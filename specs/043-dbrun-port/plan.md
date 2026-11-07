# Implementation Plan: `dbrun` production port to Rust

**Spec**: `spec.md` | **Date**: 2026-11-07 | **Branch**: `docs/spec-043-dbrun-port` (this draft; each task below has its own branch) | **Status**: Draft for the owner's review

## Summary

Promote the spike drivers in `rust/spikes/dbrun/` into `rust/crates/ai-gent-dbrun/`, finish the shared core there (CLI, profiles, plans, log, backup/restore, TLS fields), close the three gaps the owner named (TLS verification, the Oracle old-verifier case, native backup/restore), add optional Oracle thick mode, then cut the skills, guard, package and docs over to `ai-gent db` and delete `dbrun.py` and `uv`, all inside the one 3.0.0 release. Work is split into groups that one worker each can take; every output is reviewed by a different agent and lands by pull request into `dev`.

## Technical context

**Language**: Rust (workspace in `rust/`, toolchain pinned by `rust/rust-toolchain.toml`); Python only in dev tools (the parity generator, the engines script).
**Crate**: `ai-gent-dbrun` (D28) with features `sqlite`, `postgres`, `mysql`, `mssql`, `oracle`, `oracle-thick`; `process` is always compiled until the cutover removes it. `ai-gent-core` never depends on a driver. The `ai-gent` binary depends on `ai-gent-dbrun` with default features per target.
**Drivers**: see spec FR-021. TLS: rustls only (FR-016).
**Testing**: Rust unit and contract tests (`cargo test -p ai-gent-dbrun --features <engine>`), the golden/parity corpus generated from `dbrun.py` (committed generator), `scripts/test-dbrun-engines.py --runner` per engine (weekly in `scheduled.yml`, and run by the worker before its pull request), guard tests (`scripts/test-db-guard.sh`, `scripts/test-secrets-guard.sh`, `shared/guard-fixtures/*.json`), `scripts/check.sh`.

## Constraint check

| Constraint | Result |
|---|---|
| D8 no Python fallback | this spec removes the single declared exception (D26/D27 reversed by relayed decision 1) |
| D16 dev tools stay Python | the engines script and the parity generator stay Python, dev-only |
| D28 crate layout | kept; the `mssql` split stays a candidate (FR-021) |
| `shared/secrets.md` | throwaway container passwords only in tests; pass first; credentials never in argv or output; the child process of FR-014 receives them on stdin |
| `shared/containers.md` 9a | engine containers by ID or label, one engine at a time, no prune |
| CLAUDE.md "propose first" | this draft is the proposal; no skill, hook or plugin file changes until the owner approves it |
| Guards stay in step | `plugins/db/hooks/guard.sh`, `opencode/guard/rules.mjs` (parity test) and the Rust evaluator change in the cutover task only, together |
| `scripts/package.py` allowlist | any file the installed skills need at runtime is added; `--check` stays green (T090) |

## Work breakdown and workers

One worker per task group below. A task is one pull request into `dev` unless it says otherwise. The reviewer is always a different agent from the author; the review's findings, fixed or recorded, go into the task's evidence line in `tasks.md`, as 038 did in `results.md`.

| Group | Tasks | Needs first | Notes |
|---|---|---|---|
| A shared core | T001-T013 | 038 crate pieces already in `dev` | the contract, parity corpus, redaction, TLS model, plan/apply/restore/log, profiles, CLI |
| B SQLite | T020-T023 | T002, T004 | in-process; the reference connector |
| C PostgreSQL | T030-T034 | T002, T004, T007 | review findings 1 and 2, TLS |
| D MySQL and MariaDB | T040-T044 | T002, T004, T007 | datetimes/TIME, timeout, TLS |
| E SQL Server | T050-T055 | T002, T004, T007, T008 | count probe, control-batch timeouts, TLS, autocommit backup |
| F Oracle thin | T060-T065 | T002, T004, T007, T012 | numeric password redaction, descriptor check, `panic = "abort"`, old verifier |
| G Oracle thick | T070-T072 | F | the 038 T008 task made real; owner-run test |
| H Cutover | T080-T084 | A-G | skills, guard, fixtures, `dbrun.py` removal |
| I Package and uv | T090-T093 | H | allowlist, doctor, uv removal, platform matrix |
| J Docs | T095-T098 | H | references, README, docs site, migration guide |

Parallelism: A's T002, T004 and T007 unblock B-F; B-F run in parallel worktrees after them (up to the slot cap), landing one at a time in dependency order (`agents:parallel`). Each engine worker needs one engine container at a time on the machine; the disk floor applies, so engine groups that need images are scheduled so no two pull at once unless disk allows. G waits for F. H starts only when every engine passes the engines script natively. Nothing in H-J lands on `dev` before the cutover branch is ready, because `dev` ships 2.x releases until the owner decides otherwise (see Sequencing).

## Sequencing with the rest of the series (relayed 6 and 7)

```
2.27  032 foundation + 033 shadow                       (G27)
2.28  update notice + guard launcher, as soon as G28 passes; subcommands ship beside old tools; announces 3.0.0 as a major
      ----- no longer waits for, or includes, any 3.0.0 cutover -----
3.0.0 one breaking release (B1-B12 of 039), G30 amended:
        035 cutover (T060-T064)  token tools
        036 cutover (T060-T064)  worktree tool
        037 cutovers (T068 compact, the repo/spec/pass cutover tasks)
        043 port + cutover (this spec)   dbrun.py and uv gone
        install.ps1, git mode removed, tiers, docs v2
```

Why: the previous plan made `dbrun` the last 3.x step so 3.0.0 could ship with Python for one plugin. The owner reversed it (relayed 1): one release that removes every Python tool, so a user migrates once and "uv and Python are not needed" is true without an exception. The cost is that 3.0.0 now waits on the slowest group (Oracle: a beta driver, an old verifier that cannot be reproduced without another server image, and an owner-run thick test). Between 2.28 and 3.0.0, `dev` and `main` keep 2.x semantics: the 035/036/037 cutover tasks and the 043 cutover wait on their own branch and are released as one. [NEEDS CLARIFICATION: where those cutover commits wait (a long-lived branch off `dev`, which the owner's `develop` rename plan may touch, or `dev` itself held back from `main` promotions); this is the owner-side plan of relayed 8, not written here.]

## Risks

| Risk | Impact | Mitigation |
|---|---|---|
| Oracle thin is beta (`=26.0.0-beta.5`), narrower than python-oracledb thin (12c verifier only), no total login deadline, panics possible | 3.0.0 blocked, or a crash on a credential path | FR-014 containment, FR-024 deadline, FR-025 means chosen by the owner, exact pin, thick as the documented escape for users who install Instant Client |
| The old-verifier case cannot be reproduced on the 26ai image | an untested claim | needs an 11g/12.1 image (owner decision) or a unit test against a recorded handshake transcript; recorded as unverified otherwise |
| `tiberius-ng` is a one-maintainer fork | supply-chain and maintenance risk on a credential path | decision in FR-021; plain `tiberius` as fallback; consider the `mssql` crate split |
| TLS defaults break profiles that worked unencrypted | users locked out after upgrade | FR-015 default decided before T007; migration guide row; clear error naming the field |
| Native backup/restore is less proven than `pg_dump`/`mysqldump` | data loss on restore of a local dev database | FR-018 decided first; engines script restore journey per engine; backups are of proven-local databases only |
| Parity corpus covers reads only today | behaviour drift in writes and plans | T003 extends it to writes, plans, errors, CLI shapes; accepted differences reviewed |
| musl/aarch64/Windows/macOS builds of the engines were never run | a target without `db` | T091 matrix before the cutover; waiver or blocker per FR-029 |
| Thick mode untestable in CI | silent breakage | owner-run evidence (FR-027); CI builds the feature and runs the missing-library path |
| The slowest engine holds the whole release | 3.0.0 slips | engine groups run in parallel; the owner can later split the release by a new decision (not assumed here) |
| 51 Python test runs have no one-to-one Rust names | missed behaviour | SC-002 mapping table in `differences.md` |
| Local container reachability (FR-020) is open | design churn in every engine group | resolved in T010/T011 before the engine groups finish |

## Open points (all [NEEDS CLARIFICATION] items, with what each blocks)

| # | Point | Spec | Blocks |
|---|---|---|---|
| 1 | Command word `ai-gent db` | FR-001 | T011, T080 |
| 2 | Where a native binary reaches a local container with no published port; fate of `session` | FR-020 | T010, T011, engine groups' local runs |
| 3 | TLS: field names, default for existing profiles, local default, `unverified` refused on production | FR-015 | T007 |
| 4 | System roots for Oracle thin (`webpki-roots` in `oracledb`) | FR-016 | T063 |
| 5 | Meaning of "native" backup/restore; Oracle backup | FR-018 | T008, T020, T033, T043, T055, T065 |
| 6 | SQL Server count: DONE token, upstream patch, fork or one-batch | FR-012 | T051 |
| 7 | Panic containment: child process, unwinding build or upstream fix | FR-014 | T012, T060 |
| 8 | `tiberius-ng` fork vs plain vs vendored; `mssql` crate split | FR-021 | T050 |
| 9 | DDL row count and Oracle `NUMBER` contract values | FR-022 | T005, T031, T053, T061 |
| 10 | Old-verifier means and the test image for 11g/12.1 | FR-025 | T062 |
| 11 | Thick mode: variable name, targets, selection rule | FR-026 | T070, T071 |
| 12 | Platform list, waiver policy, size budget | FR-029, FR-030 | T091 |
| 13 | Where the held cutovers wait before 3.0.0 | Sequencing | owner plan |
| 14 | Whether 038's `decision.md` is written by T001 or by the owner from the relayed decisions | spec header | T001 |

## Amendments to 039 (made in this change set; each carries "amended 2026-11-07 per owner decision (relayed)")

1. `spec.md` context (the D26/D27 line) and FR-005.
2. `spec.md` open question 5 (is `uv` still a prerequisite).
3. `spec.md` and `plan.md`: the 2.28-before-3.0.0 sequencing and the one-release cutover note (plan "Sequencing" diagram).
4. `plan.md` risks: the row "A tool port misses its parity gate" now says `dbrun` blocks 3.0.0.
5. `contracts/breaking-changes.md`: B7 (the set now includes `dbrun.py`), B11 (no `uv` or Python at all), A7 wording.
6. `contracts/release-gates.md`: G30-2 (adds `dbrun.py` to the grep), G30-16 (the README sentence flips), a new G30-18 for spec 043's success criteria.
7. `contracts/migration-guide.md`: the `dbrun.py` row.
8. `tasks.md`: T049a wording, T062a (the roadmap note becomes a pointer to this spec).
9. `specs/README.md`: row 043 and the notes for 038/039.

Not amended, on purpose: 038's own text is left as the record of the spike, except for one pointer line under its Phase 5 header and one under T008 saying that 043 supersedes them.

## Verification

Per task: the commands in `tasks.md`. Per group: the engines script for that engine through the new binary. For the whole spec: `scripts/check.sh` (full, once, before the cutover pull request), `scripts/package.py --check`, `scripts/test-package.sh`, `scripts/test-migrate.sh`, the guard tests, and spec 039's amended G30 gates. Reviews are recorded per task.
