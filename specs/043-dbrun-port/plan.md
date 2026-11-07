# Implementation Plan: `dbrun` production port to Rust

**Spec**: `spec.md` | **Date**: 2026-11-07 | **Branch**: `docs/spec-043-dbrun-port` (this draft; each task below has its own branch) | **Status**: Draft for the owner's review

## Summary

Promote the spike drivers in `rust/spikes/dbrun/` into `rust/crates/ai-gent-dbrun/`, finish the shared core there (CLI, profiles, plans, log, backup/restore, TLS fields), close the three gaps the owner named (TLS verification, the Oracle old-verifier case, native backup/restore), add optional Oracle thick mode, then cut the skills, guard, package and docs over to `ai-gent db` and delete `dbrun.py` and `uv`, all at the release that ships the port, in the 3.x line, not inside 3.0.0 (amended 2026-11-07 per owner decision (chat): 3.0.0 not held for dbrun; 3.0.0 ships with `dbrun.py` still Python). Work is split into groups that one worker each can take; every output is reviewed by a different agent and lands by pull request into `dev`.

## Technical context

**Language**: Rust (workspace in `rust/`, toolchain pinned by `rust/rust-toolchain.toml`); Python only in dev tools (the parity generator, the engines script).
**Crate**: `ai-gent-dbrun` (D28) with features `sqlite`, `postgres`, `mysql`, `mssql`, `oracle`, `oracle-thick`; `process` is always compiled until the cutover removes it. `ai-gent-core` never depends on a driver. The `ai-gent` binary depends on `ai-gent-dbrun` with default features per target.
**Drivers**: see spec FR-021. TLS: rustls only (FR-016).
**Testing**: Rust unit and contract tests (`cargo test -p ai-gent-dbrun --features <engine>`), the golden/parity corpus generated from `dbrun.py` (committed generator), `scripts/test-dbrun-engines.py --runner` per engine (weekly in `scheduled.yml`, and run by the worker before its pull request), guard tests (`scripts/test-db-guard.sh`, `scripts/test-secrets-guard.sh`, `shared/guard-fixtures/*.json`), `scripts/check.sh`.

## Constraint check

| Constraint | Result |
|---|---|
| D8 no Python fallback | this spec removes the single declared exception at the release that ships the port; D26/D27 stand for 3.0.0 (amended 2026-11-07 per owner decision (chat): 3.0.0 not held for dbrun; the relayed reversal is itself reversed) |
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
| A shared core | T001-T014 | 038 crate pieces already in `dev` | the contract, parity corpus, redaction, TLS model, plan/apply/restore/log, profiles, CLI |
| B SQLite | T020-T023 | T002, T004 | in-process; the reference connector |
| C PostgreSQL | T030-T034 | T002, T004, T007 | review findings 1 and 2, TLS |
| D MySQL and MariaDB | T040-T044 | T002, T004, T007 | datetimes/TIME, timeout, TLS |
| E SQL Server | T050-T055 | T002, T004, T007, T008 | count probe, control-batch timeouts, TLS, autocommit backup |
| F Oracle thin | T060-T065 | T002, T004, T007, T012 | numeric password redaction, descriptor check, `panic = "abort"`, old verifier |
| G Oracle thick | T070-T072 | F | the 038 T008 task made real; owner-run test |
| H Cutover | T080-T084 | A-G | skills, guard, fixtures, `dbrun.py` removal |
| I Package and uv | T090-T093 | H | allowlist, doctor, uv removal, platform matrix |
| J Docs | T095-T098 | H | references, README, docs site, migration guide |

Parallelism: A's T002, T004 and T007 unblock B-F; B-F run in parallel worktrees after them (up to the slot cap), landing one at a time in dependency order (`agents:parallel`). Each engine worker needs one engine container at a time on the machine; the disk floor applies, so engine groups that need images are scheduled so no two pull at once unless disk allows. G waits for F. H starts only when every engine passes the engines script natively. Cutover work in H-J merges to `dev` as it is ready but stays inert for packages until the single cutover pull request (Sequencing, FR-036); T001's `decision.md` is approved by the owner before B-F start.

## Sequencing with the rest of the series (relayed 6 and 7)

amended 2026-11-07 per owner decision (chat): 3.0.0 not held for dbrun: the block below and the 'Why' paragraph are superseded by the 'Amended sequencing' paragraph that follows them. Struck text is kept for the record.

```
2.27  032 foundation + 033 shadow                       (G27)
2.28  update notice + guard launcher, as soon as G28 passes; subcommands ship beside old tools; announces 3.0.0 as a major
      ----- no longer waits for, or includes, any 3.0.0 cutover -----
3.0.0 one breaking release (B1-B12 of 039), G30 amended:
        035 cutover (T060-T064)  token tools
        036 cutover (T060-T064)  worktree tool
        037 cutovers (T068 compact, the repo/spec/pass cutover tasks)
        ~~043 port + cutover (this spec)   dbrun.py and uv gone~~   (amended 2026-11-07 per owner decision (chat): 3.0.0 not held for dbrun: not in 3.0.0; dbrun.py stays Python, uv needed only by the db plugin)
        install.ps1, git mode removed, tiers, docs v2
```

Why: the previous plan made `dbrun` the last 3.x step so 3.0.0 could ship with Python for one plugin. The owner reversed it (relayed 1): one release that removes every Python tool, so a user migrates once and "uv and Python are not needed" is true without an exception. The cost is that 3.0.0 now waits on the slowest group (Oracle: a beta driver, a 10G verifier case that stays unreproduced without another server image, and an owner-run thick test). Between 2.28 and 3.0.0, `dev` and `main` keep 2.x semantics: the 035/036/037 cutover tasks and the 043 cutover merge to `dev` as ready, inert for packages, and take effect together in the single cutover pull request. Decided (chat, 2026-11-07, point 13): the cutover tasks merge to `dev` as they are ready but stay inert for packages (no Python script deleted, no packaged skill or allowlist edit applied) until the single cutover pull request; 2.28 releases from `dev` with the cutover edits not applied. No long-lived branch.

**Amended sequencing (amended 2026-11-07 per owner decision (chat): 3.0.0 not held for dbrun).** 2.28 and 3.0.0 are as spec 039 has them (D26/D27 stand): 3.0.0 carries the 035, 036 and 037 cutovers, `install.ps1` and the rest, with `dbrun.py` still Python as the one documented exception; the README says so plainly (only the db plugin needs `uv` and its Python packages). This spec's groups A-G (shared core, the five engines, optional thick) run in the 3.x line, engine by engine, unblocked by 3.0.0 scheduling but still gated on the owner's approval of `decision.md` (T001, FR-037). Groups H (cutover) and I (package, `uv` removal) happen at the release that ships the port, when SC-001 to SC-007 pass; that release is planned by `scripts/release.py` from the commit types and carries one `BREAKING CHANGE:` footer for the `dbrun` move. Group J follows H. Until then `dev` and `main` keep `dbrun.py` working and the `ai-gent db` code merges to `dev` inert for packages (FR-036).

## Risks

| Risk | Impact | Mitigation |
|---|---|---|
| Oracle thin is beta (`=26.0.0-beta.5`), narrower than python-oracledb thin (12c verifier only), no total login deadline, panics possible | the port's cutover blocked (amended 2026-11-07 per owner decision (chat): 3.0.0 not held for dbrun; was "3.0.0 blocked"), or a crash on a credential path | FR-014 child-process containment, FR-024 deadline, FR-025 clear refusal pointing to thick, exact pin, thick as the documented escape for users who install Instant Client |
| The old-verifier case cannot be reproduced on the 26ai image | an untested claim | the owner chose no 11g/12.1 image now (2026-11-07): the 10G case stays recorded "unverified"; the 11G-only account tests the refusal text |
| `tiberius-ng` is a one-maintainer fork | supply-chain and maintenance risk on a credential path | fork kept with its checksum pinned (decided 2026-11-07), plain `tiberius` as the measured fallback; the `mssql` crate split stays open |
| TLS defaults break profiles that worked unencrypted | users locked out after upgrade | FR-015 default decided (verify, 2026-11-07); migration guide row; clear error naming the field; explicit `unverified` only on proven-local loopback profiles |
| Native backup/restore is less proven than `pg_dump`/`mysqldump` | data loss on restore of a local dev database | FR-018 decided (the same engine tools through the container exec, 2026-11-07); engines script restore journey per engine; backups are of proven-local databases only |
| Parity corpus covers reads only today | behaviour drift in writes and plans | T003 extends it to writes, plans, errors, CLI shapes; accepted differences reviewed |
| musl/aarch64/Windows/macOS builds of the engines were never run | a target without `db` | T091 matrix before the cutover; per-target waiver of the failing engine per FR-029/FR-035 |
| Thick mode untestable in CI | silent breakage | owner-run evidence (FR-027); CI builds the feature and runs the missing-library path |
| The slowest engine holds the port's release (amended 2026-11-07 per owner decision (chat): 3.0.0 not held for dbrun: no longer 3.0.0) | the port's release slips; 3.0.0 does not | engine groups run in parallel; the owner can later split the release by a new decision (not assumed here) |
| 51 Python test runs have no one-to-one Rust names | missed behaviour | SC-002 mapping table in `differences.md` |
| The container-exec path (profiles with no published port) diverges from the driver path | two code paths to keep equal | one parity corpus exercises both (T014); decided 2026-11-07 |

## Decisions (given by the owner in chat, 2026-11-07, three rounds; the 14 open points of the first draft)

Format as `shared/asking.md`: `Decided: <choice> — <reason>`. The eight earlier decisions stay "relayed" (spec header).

| # | Point | Decided | Reason | Date | Unblocks |
|---|---|---|---|---|---|
| 1 | Command word | `ai-gent db`; db skills and guard call it | 038's recommendation | 2026-11-07 | T011, T080 |
| 2 | Local container with no published port; `session` | no published port keeps the container exec path (engine client in the container); published port uses the Rust drivers; `session` stays | no client image or namespace join to rebuild | 2026-11-07 | T010, T011, T014 |
| 3 | TLS model | verify by default (system roots or named CA file); explicit `unverified` only on a proven-local loopback profile; refused on production | safe default with a door for throwaway databases | 2026-11-07 | T007, T032, T042, T054, T063 |
| 4 | System roots for Oracle thin | PEM wallet or CA file, no system-roots promise | `oracledb` ships webpki-roots only | 2026-11-07 | T063 |
| 5 | "Native" backup/restore | Rust drives the same tools through the container exec; no Python, no Rust dump | proven tools, no correctness risk | 2026-11-07 | T008, T020, T033, T043, T055 (Oracle: T065 still open) |
| 6 | SQL Server count | one batch per plan, count read once at the end or from DONE tokens if exposed | no probe between statements, no patch | 2026-11-07 | T051 |
| 7 | Panic containment | child process of `ai-gent db` per engine call; `panic = "abort"` stays | clean failure JSON, uniform deadline | 2026-11-07 | T012, T060, T064 |
| 8 | `tiberius-ng` | keep the fork, checksum pinned, not vendored; plain `tiberius` is the fallback | measured drop-in | 2026-11-07 | T050 (crate split: still open) |
| 9 | DDL row count and `NUMBER` | the Rust values are the contract (DDL `0`, `NUMBER` from text), documented as intended changes of the release that ships the port | simplest and server-true | 2026-11-07 | T005, T031, T053, T061, T097 |
| 10 | Old-verifier means | clear refusal naming the cause and pointing to thick; no 11g/12.1 image now | cost not justified for a beta driver | 2026-11-07 | T062 |
| 11 | Thick mode | `AI_GENT_ORACLE_CLIENT_LIB`; separate optional crate/feature, only where a client exists, never in the package; thin default | no client ever shipped | 2026-11-07 | T070, T071 (selection rule: still open) |
| 12 | Platforms, waiver, size | six release targets; an engine that cannot build is waived for that target with a refusal and a matrix line; size: spec 032 budget (8 MB binary, derived from 032/038 measurements) | do not block the release on one engine | 2026-11-07 | T091 |
| 13 | Held cutovers | merge to `dev` as ready, inert for packages until the single cutover PR; 2.28 releases from `dev` without the cutover edits | no long-lived branch | 2026-11-07 | T084 |
| 14 | 038 `decision.md` | T001 drafts it from 038's results and these answers; owner approves in the PR before any engine group starts | owner keeps the gate | 2026-11-07 | T001, groups B-F |

## Still open (genuinely; nothing else)

- TLS profile field names and file keys (T007 proposes, the owner reviews in its PR).
- Whether Oracle gets a backup (Data Pump) or `apply` stays refused there (T065; default: refused).
- Whether `mssql` becomes its own crate (D28 candidate).
- Oracle thick selection rule when the variable is set (assumed: thick only when set, no automatic fallback; T070).
- Whether the full-feature `regex` is acceptable for size (answered by a measurement in T091).
- Measured combined size per target: the 8 MB budget is derived (about 6.1 MB estimated), not yet measured together.

## Amendments to 039 (made in this change set; each carries "amended 2026-11-07 per owner decision (relayed)")

**Reverted, amended 2026-11-07 per owner decision (chat): 3.0.0 not held for dbrun.** The nine amendments below made 039 hold 3.0.0 for this port. They are reverted to the `origin/dev` wording of 039 (D26/D27, FR-005, open question 5, B7, B11, G30-2, G30-16, T049a, T062a, the migration-guide row, the plan risk row and diagram); what remains in 039 is the pointer that the owner decided the command word `ai-gent db` (A7 and the migration-guide row) and G30-18 rewritten to 'the dbrun port is tracked by spec 043, not a 3.0.0 gate'. `specs/README.md` is reworded the same way. The list below records what had been amended.

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
