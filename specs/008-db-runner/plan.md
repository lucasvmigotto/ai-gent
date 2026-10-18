Reconstructed by project:introspec on 2026-10-18 from 384c771

# Implementation Plan: Audited database runner (dbrun)

**Branch**: `docs/introspec-site-runners` | **Date**: 2026-10-18 | **Spec**: `spec.md`

## Summary

One stdlib-only Python file on the host does everything except talk to the database: profiles, classification, statement policy, masking, logging, plans and backups. A hidden `_exec` subcommand of the same file runs in a child process (host `uv run --with <driver>` or a client container) and receives the request — credentials included — as JSON on stdin [OBSERVED: plugins/db/scripts/dbrun.py:1-13,1081-1127,1735-1739].

## Technical Context

**Language/Version**: Python 3.9+ stdlib (host); Python 3.12 in the client image.

**Primary Dependencies**: at run time only — `psycopg[binary]`, `PyMySQL`, `pymssql`, `oracledb` through `uv`; `podman` or `docker` for local containers and backups (`pg_dump`/`pg_restore`, `mysqldump`, SQL Server `BACKUP DATABASE`, SQLite backup API) [OBSERVED: dbrun.py:36-44,1404-1441].

**Storage**: see `data-model.md`.

**Testing**: `plugins/db/scripts/test_dbrun.py` (41 test methods, 51 tests with subtests; passed locally) in `scripts/check.sh` and CI; `scripts/test-dbrun-engines.py` against real PostgreSQL, MySQL, MariaDB, SQL Server and Oracle containers — manual, before pushing dbrun changes (CLAUDE.md).

**Target Platform**: Linux host with podman or docker; remote databases over the network.

**Project Type**: CLI.

**Constraints**: lock timeout 5 s, default statement timeout 30 s (120 s for apply), 100 rows [OBSERVED: dbrun.py:45,1759-1785].

## Constitution Check

N/A — no constitution. Checked against `plugins/db/references/safety.md` as the module docstring states [OBSERVED: dbrun.py:4-6]; the guard hook `plugins/db/hooks/guard.sh` keeps agents on this path (not reconstructed here).

## Project Structure

```text
plugins/db/scripts/
├── dbrun.py
├── passstore.py -> ../../../shared/passstore.py
└── test_dbrun.py
scripts/test-dbrun-engines.py
```
