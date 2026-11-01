# Implementation Plan: dbrun spike

**Branch**: `docs/specs-rust-tools` | **Date**: 2026-11-01 | **Spec**: `spec.md`

## Summary

Run eight experiments (`contracts/experiments.md`) in a throwaway `rust/spikes/dbrun/` crate, record results, and write `decision.md` with the template. No change to `plugins/db/`.

## Technical Context

**Language**: Rust for prototypes; Python for the existing acceptance harness. **Candidates** (to be evaluated, not chosen): a PostgreSQL client crate; a MySQL/MariaDB client crate, or `sqlx` for the four databases it supports [RESEARCHED: S3]; `rusqlite` with a bundled SQLite; SQL Server and Oracle crates not yet identified [ASSUMPTION]. **Container shell-out**: the existing `CLIENT_IMAGE` pattern, with each engine's CLI or a small helper.
**Testing**: shared fixtures for E1; `scripts/test-dbrun-engines.py` gets a `--runner` argument (default: `dbrun.py`) for E2-E6, so the same checks drive any candidate.

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
  src/drivers/{pg,mysql,sqlite,mssql,oracle}.rs   E2-E6 prototypes behind one trait
  src/bin/adapter.rs       speaks dbrun's CLI subset so scripts/test-dbrun-engines.py can drive it
specs/038-db-runner-spike/{results.md, engine-matrix.md, decision.md}
```

## Parity (shared fixtures, D10) for the safety core

The 41 defined tests (51 runs, because subclasses of `Sandbox` and `PassTest` re-run inherited tests) split as: ClassifierTest 8, CostTest 2, MaskTest 3, RedactTest 2, NetworkTest 1 (E1 scope, 16 tests); ProfilesTest 4, PassTest 5, TomlTest 7, MigrateAllTest 1 (profile and secret loading; need 037's pass-store module); SqliteEndToEndTest 8 (E4 and the plan/apply/restore flow). Python builds profiles in a `Sandbox` with a fake `pass`; the fixture format of 035 `contracts/parity-fixtures.md` applies. If the verdict is "move", all 41 become fixtures, plus the real-engine script as the integration gate (it already runs weekly).

## Cutover (only if the verdict is not S)

D7 applies: one release, references switched, `dbrun.py` deleted. Edit list from `git grep 'dbrun\.py'` (21 lines, 12 files, 2026-11-01): `AGENTS.md` (1), `README.md` (1), `opencode/guard/test/rules.test.mjs` (5), `plugins/db/hooks/guard.sh` (1), `plugins/db/references/engines/oracle.md` (2), `.../postgresql.md` (1), `plugins/db/references/runner.md` (2), `plugins/db/scripts/dbrun.py` (1), `plugins/db/scripts/test_dbrun.py` (1), `scripts/check.sh` (1), `scripts/test-db-guard.sh` (4), `scripts/test-secrets-guard.sh` (1). The bare word `dbrun` appears in 43 files and 324 lines (prose; the skills in `plugins/db/skills/*`, the guard messages in `opencode/guard/rules.mjs` lines 147-218, `plugins/db/hooks/guard.sh`, `plugins/secrets/hooks/guard.sh` line 48). If the command word stays "dbrun" (a wrapper), none of those guard patterns change.

## Risks

| Risk | Mitigation |
|---|---|
| Driver crates for SQL Server and Oracle immature or needing native libraries | hybrid option H keeps a container route |
| Behaviour drift in the safety core | E1 first, fixtures from the Python tests |
| Binary size and build time grow with drivers | E8 measures; drivers behind cargo features so lean builds stay small |
| The spike never ends | FR-002 time box |
| The engines script pulls 5.5 GB | one engine at a time with `--engine`; the disk floor is respected |

## Phased approach

- **Phase 1**: E1 (safety core) and the engine matrix survey (no containers needed).
- **Phase 2**: E4, E2, E3 (sqlite, PostgreSQL, MySQL/MariaDB).
- **Phase 3**: E5, E6 (SQL Server, Oracle), the container-route comparison.
- **Phase 4**: E7, E8 (Windows, size), then `decision.md` and the owner's sign-off.
