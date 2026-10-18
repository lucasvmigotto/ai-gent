Reconstructed by project:introspec on 2026-10-18 from 384c771

# Feature Specification: Audited database runner (dbrun)

**Feature Branch**: `docs/introspec-site-runners` (reconstructed; the code already exists on `main`)

**Created**: 2026-10-18

**Status**: Draft

**Input**: Reverse-engineered from `plugins/db/scripts/dbrun.py` (1837 lines), `plugins/db/scripts/test_dbrun.py` and `scripts/test-dbrun-engines.py`. The `db:*` skills' Markdown is out of scope; `plugins/db/references/safety.md` and `runner.md` are the rules this code enforces.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Know which databases exist and what they really are (Priority: P1)

An agent lists connection profiles (from TOML/.env files and running compose containers of this repository) and classifies one: `local` only when proven, otherwise the declared class, defaulting to `production`.

**Why this priority**: every other command's permissions depend on the class.

**Independent Test**: `test_declared_local_without_proof_is_production`, `test_container_or_pod`, `test_listing_never_shows_secrets`, `test_families_and_precedence`.

**Acceptance Scenarios**:

1. **Given** `AI_GENT_DB_CONNECTIONS` or `connections.toml` then `.env` in `${XDG_CONFIG_HOME}/ai-gent/db`, **When** `dbrun profiles` runs, **Then** profiles are listed with engine, class and secret source, never a value [OBSERVED: plugins/db/scripts/dbrun.py:151-161,1181-1201].
2. **Given** a running container started by this repository's compose project with a known engine image, **When** profiles are listed, **Then** it appears as `local:<service>` [OBSERVED: dbrun.py:407-422].
3. **Given** a profile declared `local` whose container isn't proven (no engine, remote `DOCKER_HOST`/`CONTAINER_HOST`, container missing, stopped, a proxy image, or another repository's compose project), **When** classified, **Then** it is `production` and not writable, with the reason as evidence [OBSERVED: dbrun.py:471-526,454-468].
4. **Given** a SQLite file inside this repository and not a symlink, **When** classified, **Then** it is local and writable; outside, read-only [OBSERVED: dbrun.py:477-486].
5. **Given** a name defined twice, **When** profiles load, **Then** dbrun fails with "defined twice" [OBSERVED: dbrun.py:425-431].

---

### User Story 2 - Read safely, anywhere (Priority: P1)

`dbrun query` runs only read-only statements, in a read-only session with statement and lock timeouts, a row cap and masking of personal data, after logging the statements and the reason.

**Why this priority**: the only operation allowed on remote databases.

**Independent Test**: `test_reads`, `test_writes`, `test_backslash_trick_refused`, `test_star_and_counts_on_user_tables`, `test_read_masks_and_logs`, `test_row_cap`, `test_remote_refusals`, `test_remote_read_is_read_only_even_past_the_classifier`.

**Acceptance Scenarios**:

1. **Given** any statement not starting with a read keyword, or containing a write word, a locking clause, `INTO`, a sequence advance, a side-effect function or an Oracle `DBMS_*`/`UTL_*` package, **When** queried, **Then** it is refused (exit 3) — unknown means write [OBSERVED: dbrun.py:660-693,1250-1254].
2. **Given** a non-local target, **When** a statement uses `SELECT *` or an unfiltered `COUNT(` on a user table, **Then** it is refused unless `--allow-full-count`; catalog queries are exempt [OBSERVED: dbrun.py:713-727].
3. **Given** a read, **When** it runs, **Then** the session is read-only (`default_transaction_read_only`, `SET SESSION TRANSACTION READ ONLY`, SQLite `mode=ro` + `query_only`) with `statement_timeout`/equivalent and a 5 s lock timeout, and is rolled back [OBSERVED: dbrun.py:45,905-978,1022-1068].
4. **Given** columns named like secrets, emails, documents, phones, cards, birth dates, addresses, IPs or names, **When** results print, **Then** values are masked (secrets always, even with `--reveal`) and the masked columns are listed [OBSERVED: dbrun.py:730-836; test_reveal_keeps_secrets_redacted].
5. **Given** more rows than `--max-rows` (default 100), **When** results print, **Then** they are truncated and say so [OBSERVED: dbrun.py:1766,1285-1287].
6. **Given** `--explain`, **When** the target is Oracle and remote, **Then** refused (EXPLAIN PLAN writes PLAN_TABLE); elsewhere a plain plan, never `EXPLAIN ANALYZE` [OBSERVED: dbrun.py:676-680,1256-1257].

---

### User Story 3 - Change a proven local database with a backup (Priority: P1)

On a proven local database, `plan` records the statements and expected row counts; after the user confirms, `apply --confirmed` re-proves "local", backs up, runs everything in one transaction and commits only if every expected count matches; `restore --confirmed` puts the backup back.

**Independent Test**: `test_plan_apply_restore`, `test_apply_rolls_back_on_count_mismatch`, `test_tampered_plan_refused`, `test_system_databases_refused_for_writes`; `scripts/test-dbrun-engines.py` against real engines (not run here, not in CI).

**Acceptance Scenarios**:

1. **Given** a non-writable class, **When** `plan`, `apply` or `restore` runs, **Then** it is refused with the evidence and a pointer to `dbrun script` [OBSERVED: dbrun.py:1303-1307].
2. **Given** a system database (`master`, `mysql`, `template1`, …), **When** `plan` or `apply` runs, **Then** refused [OBSERVED: dbrun.py:1311-1324].
3. **Given** DDL on an engine dbrun can't back up (Oracle), **When** planning, **Then** refused [OBSERVED: dbrun.py:1310,1335-1338].
4. **Given** a plan, **When** it is recorded, **Then** it is written `0600`, exclusive-create, with a SHA-256 of its statements; `apply` refuses a plan made for another profile or whose statements no longer match the hash [OBSERVED: dbrun.py:1339-1352,1371-1382].
5. **Given** `apply` without `--confirmed`, **When** run, **Then** refused [OBSERVED: dbrun.py:1463-1464].
6. **Given** an expected count mismatch or an error, **When** applying, **Then** the transaction rolls back; on MySQL/MariaDB DDL the output says DDL committed and how to restore [OBSERVED: dbrun.py:1052-1060,1482-1487].

---

### User Story 4 - Hand a change to a person for any other database (Priority: P2)

`dbrun script` writes a change script for a person to review and run — transaction wrapper, pre-check, verification and rollback sections — and never executes it.

**Independent Test**: `test_script_never_executes`.

**Acceptance Scenarios**:

1. **Given** an `UPDATE` or `DELETE` without `WHERE`, **When** scripting, **Then** refused [OBSERVED: dbrun.py:1550-1553].
2. **Given** an existing `--out` file, **When** scripting, **Then** refused [OBSERVED: dbrun.py:1547-1549].
3. **Given** valid statements, **When** scripting, **Then** the file is written and the log says "not executed"; no password is read [OBSERVED: dbrun.py:1541-1589].

---

### User Story 5 - Every action leaves a trail (Priority: P2)

Before a statement runs, an entry is appended to a per-day, per-repository Markdown log (`0600`) with action, profile, class, evidence, reason and statements; the result is appended after. `dbrun log --tail N` shows it.

**Acceptance Scenarios**:

1. **Given** any query, plan, apply, restore, script or session action, **When** it starts, **Then** `log_start` writes "status: started" first [OBSERVED: dbrun.py:856-889].
2. **Given** log text, **When** it is written, **Then** known secrets, host, user and password values are redacted [OBSERVED: dbrun.py:838-854,866; test_host_user_password_redacted].

---

### User Story 6 - Credentials stay out of the agent's reach (Priority: P2)

Passwords come from pass first (`pass:` references, then `ai-gent/db/<family>`, then `ai-gent/db/<profile>`), a plain-text file last; `secrets migrate` moves plain-text passwords into pass; `profiles --export-env` writes a `.env` with `pass:` references. Drivers run in a child process that gets credentials only on stdin.

**Acceptance Scenarios**:

1. **Given** a profile without a password, **When** loaded, **Then** the family entry is tried before the profile entry [OBSERVED: dbrun.py:318-333; test_family_default_entry].
2. **Given** `secrets migrate <profile>` or `--all`, **When** run, **Then** values move into pass and the file lines become references, never shown [OBSERVED: dbrun.py:1622-1723; test_migrate].
3. **Given** a remote target, **When** the driver runs, **Then** it's `uv run --with <driver>` on the host; for a local container, a client container joins the database container's network namespace (by `ns:` path inside a podman pod) with the script mounted read-only [OBSERVED: dbrun.py:529-538,1081-1127].

---

### User Story 7 - Warm session for an investigation (Priority: P3)

`dbrun session start|stop|status` keeps a client container running so repeated queries skip the startup.

**Acceptance Scenarios**:

1. **Given** `session start`, **When** run, **Then** a container `sleep infinity` with the same network and mounts starts and later queries reuse it; `stop` removes it [OBSERVED: dbrun.py:1591-1619].

### Edge Cases

- `restore` re-proves "local" but, unlike `apply`, doesn't call `require_user_database` [OBSERVED: dbrun.py:1501-1506 vs 1467-1468] — harmless today since a system database can't get a plan, but asymmetric.
- The plan hash lives in the same file as the statements; it catches accidental edits, not a deliberate rewrite of both [INFERRED: dbrun.py:1346,1380 → integrity, not authenticity; the file is `0600` in the user's state dir].
- `script`'s "no WHERE" check looks for the word anywhere in the statement, so a `WHERE` in a subquery satisfies it [INFERRED: dbrun.py:1551-1553; confirm with `DELETE FROM t USING (SELECT … WHERE …) s`].
- The client image defaults to the tag `ghcr.io/astral-sh/uv:python3.12-bookworm-slim` (no digest) and drivers are unpinned (`psycopg[binary]`, `PyMySQL`, `pymssql`, `oracledb`) [OBSERVED: dbrun.py:36-44].
- A started session container has no lifetime limit [OBSERVED: dbrun.py:1611-1612].

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Unknown targets MUST be treated as production; `local` MUST be proven per run.
- **FR-002**: Remote targets MUST only be read, in a read-only session, with timeouts.
- **FR-003**: Writes MUST need a recorded plan, the user's confirmation (`--confirmed`), a fresh local proof and a backup where the engine allows.
- **FR-004**: Results MUST be masked by column name and free-text patterns (email, CPF, CNPJ, Luhn-valid card numbers); secrets MUST stay masked under `--reveal`.
- **FR-005**: Every action MUST be logged before it runs, redacted.
- **FR-006**: Exit codes: 0 ok, 1 error, 2 usage, 3 refused, 4 connection [OBSERVED: dbrun.py:32].

### Key Entities

See `data-model.md`.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: `python3 -m unittest plugins/db/scripts/test_dbrun.py` passes [OBSERVED: `scripts/check.sh --quick` 2026-10-18 → "dbrun (Ran 51 tests)" ok].
- **SC-002**: Zero writes ever reach a non-local target.

## Assumptions

- [ASSUMPTION: `scripts/test-dbrun-engines.py` passes against the five server engines; it wasn't run in this reconstruction and isn't in CI.]
