# Implementation Plan: dbrun spike

**Branch**: `docs/specs-rust-tools` | **Date**: 2026-11-01 | **Spec**: `spec.md`

## Summary

Run the experiments (`contracts/experiments.md`; E1-E8, with E6 split into E6a and E6b) in a throwaway `rust/spikes/dbrun/` crate, record results, and write `decision.md` with the template. No change to `plugins/db/`. The research pass in `docs/product/dbrun-spike-report.md` already measured part of E1-E5 and E8; its numbers seed `measurements.md`, and the experiments close what it did not run. After a non-S verdict, the production port follows the crate layout below (D28).

## Technical Context

**Language**: Rust for prototypes; Python for the existing acceptance harness. **Candidates** (to be evaluated, not chosen): `tokio-postgres` 0.7.18 or sync `postgres` 0.19.14; `mysql` 28.0.4 or `mysql_async` 0.37.1 (one client for MySQL and MariaDB), or `sqlx` for the four databases it supports [RESEARCHED: S3, S9]; `rusqlite` 0.41.1 with `bundled`; SQL Server: `tiberius-ng` 0.13.1 (fork of `tiberius` 0.13.0, rustls) with a fork-health check; Oracle: the `oracle` crate 0.6.3 (needs Instant Client) and `oracledb` 26.0.0-beta.5 (pure Rust, beta) [RESEARCHED: S8, S9]. **Container shell-out**: the existing `CLIENT_IMAGE` pattern, with each engine's CLI (`psql`, `sqlcmd`, `sqlplus`) or a small helper. **Python `_exec` bridge**: the Rust binary spawns `dbrun.py _exec` with the same JSON request, so any engine without a native driver keeps working (option H-exec). `sqlparser` is rejected as the classifier (FR-010).
**Testing**: shared fixtures for E1, including a committed `golden.json` generated from the Python (the prototype's copy lived in a scratchpad); `scripts/test-dbrun-engines.py` gets a `--runner` argument (default: `dbrun.py`) for E2-E6, so the same checks drive any candidate. `--runner` comes before the PostgreSQL prototype (T003 before T005). Backups (`pg_dump`, `mysqldump`, `BACKUP DATABASE`) are container shell-outs that do not depend on the driver choice, so no experiment replaces them.

## Constraint check

| Constraint | Result |
|---|---|
| D19 "last or never; a spike decides" | this spec is the spike |
| D8 no Python fallback | if S is chosen, db is the single declared exception, documented and signed by the owner, not hidden |
| D16 dev tools stay Python | the engines script and `test_dbrun.py` stay until the verdict says otherwise |
| `shared/secrets.md` | prototypes use throwaway passwords only (FR-004) |
| `shared/containers.md` §9a | engine containers go by ID or label, removed one engine at a time; no prune |
| AGENTS.md | propose first: running the engines script pulls about 5.5 GB of images; the disk floor applies; the weekly CI already runs it |
| Guards | untouched during the spike (SC-006) |

## Module layout (spike only)

```
rust/spikes/dbrun/
  Cargo.toml               not in the workspace default members; not packaged
  src/core/                E1: scan, classify, cost, mask, redact (driver-free)
  src/drivers/{pg,mysql,sqlite,mssql,oracle}.rs   E2-E6 prototypes behind one trait (the shape of the future `Connector`)
  src/drivers/process.rs   the `_exec` bridge: spawns dbrun.py _exec, JSON request on stdin, one JSON line back
  src/bin/adapter.rs       speaks dbrun's CLI subset so scripts/test-dbrun-engines.py can drive it
specs/038-db-runner-spike/{results.md, engine-matrix.md, decision.md}
```

## Crate layout (D28)

Target layout once the owner signs a non-S verdict (the spike code is not it):

```
rust/crates/
  ai-gent-core/     lean: --json envelope, exit codes, paths, version; no database driver, ever
  ai-gent-dbrun/    safety core (scanner, splitter, classifier, cost rules, masking, redaction,
                    plan engine, logging) + `Connector` trait + connector modules behind features
    src/connector/{mod,process,sqlite,postgres,mysql,mssql,oracle}.rs
  ai-gent/          the binary; depends on ai-gent-dbrun with per-target default features
```

- Features: `sqlite`, `postgres`, `mysql`, `mssql`, `oracle`; the binary picks defaults per target (for example no `oracle` on Windows). `process` (the `_exec` bridge) is always compiled in, since it is the fallback for every engine without a native module.
- The `Connector` trait is derived from the Python `_exec` JSON contract (request: engine, connection facts, mode, statements, timeout, row cap; result: one JSON line), so the process connector is the reference implementation and a native connector must return the same shapes.
- No crate per connector in 3.0.0. A split needs (a) a different release cadence or licence, (b) an outside user wanting it alone, or (c) a dependency tree that makes the shared crate hard to build; the likeliest first split is a fork-dependent `mssql` or `oracle`, after the trait has been exercised by at least two native connectors.
- `ai-gent-dbrun` README: built for ai-gent's own policy, not a general SQL validator (sqlparser: 163 of 1026 disagreements, 119 unsafe).
- Versioning and release: independent SemVer (0.x until the API is deliberately stable, D23); published after `ai-gent-core` and before `ai-gent`; Trusted Publishing is the owner's to configure under the `crate-release` environment; `cargo publish --dry-run -p ai-gent-dbrun` verifies the metadata (spec 032 FR-018). Workspace, `release.py` and publish-order changes live in spec 032 (T021, T033, T033a, T040).

## Parity (shared fixtures, D10) for the safety core

The 41 defined tests (51 runs, because subclasses of `Sandbox` and `PassTest` re-run inherited tests) split as: ClassifierTest 8, CostTest 2, MaskTest 3, RedactTest 2, NetworkTest 1 (E1 scope, 16 tests); ProfilesTest 4, PassTest 5, TomlTest 7, MigrateAllTest 1 (profile and secret loading; need 037's pass-store module); SqliteEndToEndTest 8 (E4 and the plan/apply/restore flow). Python builds profiles in a `Sandbox` with a fake `pass`; the fixture format of 035 `contracts/parity-fixtures.md` applies. If the verdict is "move", all 41 become fixtures, plus the real-engine script as the integration gate (it already runs weekly).

## Cutover (only if the verdict is not S)

D7 applies: one release, references switched, `dbrun.py` deleted. Edit list from `git grep 'dbrun\.py'` (21 lines, 12 files, 2026-11-01): `AGENTS.md` (1), `README.md` (1), `opencode/guard/test/rules.test.mjs` (5), `plugins/db/hooks/guard.sh` (1), `plugins/db/references/engines/oracle.md` (2), `.../postgresql.md` (1), `plugins/db/references/runner.md` (2), `plugins/db/scripts/dbrun.py` (1), `plugins/db/scripts/test_dbrun.py` (1), `scripts/check.sh` (1), `scripts/test-db-guard.sh` (4), `scripts/test-secrets-guard.sh` (1). The bare word `dbrun` appears in 43 files and 324 lines (prose; the skills in `plugins/db/skills/*`, the guard messages in `opencode/guard/rules.mjs` lines 147-218, `plugins/db/hooks/guard.sh`, `plugins/secrets/hooks/guard.sh` line 48). If the command word stays "dbrun" (a wrapper), none of those guard patterns change.

## Phasing after a verdict (from the report; a recommendation)

1. Rust core and CLI parity with the Python `_exec` helper for every driver; `scripts/test-dbrun-engines.py --runner` against the new binary.
2. Native sqlite, then PostgreSQL, then MySQL and MariaDB.
3. Native SQL Server if `tiberius-ng` (or another crate) passes the engines script and the TLS checks on the 2022 image.
4. Oracle: re-evaluate `oracledb` once it leaves beta; otherwise the helper stays.
5. The Windows, aarch64 and musl matrix before shipping any phase.

Do not port if: the owner accepts "db needs uv" long term; the Windows or musl TLS builds prove painful; phase-1 plan, apply and restore parity fails the engines script; or `tiberius-ng` and `oracledb` both stay unfit, leaving only sqlite, PostgreSQL and MySQL native (then keep the script and document the requirement). Strongest case against: the Python is audited and works, the goldens covered reads only, type mapping, error text, timeouts, TLS defaults and plan/apply semantics were not exercised, and the gain is mostly packaging and startup on a credential-handling path.

## Risks

| Risk | Mitigation |
|---|---|
| Driver crates for SQL Server and Oracle immature or needing native libraries (`tiberius` maintenance doubts, `oracle` needs Instant Client, `oracledb` is beta) | option H keeps a container route, H-exec keeps the Python helper |
| The prototype's evidence is not in the repo and covered reads only | E1 regenerates goldens with a committed generator; plan, apply, restore and remote-declared checks stay on the "not run" list until run |
| Windows, musl, aarch64 builds (aws-lc-rs needs a C/C++ toolchain; bundled sqlite needs a C compiler) | E8 measures the build, E7 runs Windows; none ships before the matrix is green |
| Behaviour drift in the safety core | E1 first, fixtures from the Python tests |
| Binary size and build time grow with drivers | E8 measures; drivers behind cargo features so lean builds stay small |
| The spike never ends | FR-002 time box |
| The engines script pulls 5.5 GB | one engine at a time with `--engine`; the disk floor is respected |

## Phased approach

- **Phase 1**: E1 (safety core) and the engine matrix survey (no containers needed).
- **Phase 2**: E4, E2, E3 (sqlite, PostgreSQL, MySQL/MariaDB).
- **Phase 3**: E5, E6a, E6b (SQL Server, Oracle), the container-route comparison.
- **Phase 4**: E7, E8 (Windows, size), then `decision.md` and the owner's sign-off.
