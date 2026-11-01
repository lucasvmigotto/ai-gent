# Feature Specification: Database runner spike (does `dbrun` move to Rust, and how)

**Feature Branch**: `docs/specs-rust-tools` (specification only)

**Created**: 2026-11-01

**Status**: Draft

**Type**: **a time-boxed spike, not a port.** This spec produces a decision, not a binary. It authorises experiments and a written verdict; it does not authorise replacing `dbrun.py`.

**Input**: `docs/product/rust-decisions.md` (D9 lists `db` as "later"; D19: "`dbrun` is last or never: its drivers come from `uv run --with` or a container, so a Rust port needs native drivers or a container shell-out. A spike decides."), `specs/008-db-runner/` (the behavioural baseline), `plugins/db/scripts/dbrun.py` (1844 lines), `plugins/db/scripts/test_dbrun.py` (647 lines; the suite reports 51 tests, 41 defined, the rest are inherited runs in subclasses), `scripts/test-dbrun-engines.py` (351 lines; real engines, weekly in `.github/workflows/scheduled.yml`), `plugins/db/references/{runner,safety,engines/*}.md`.

**Depends on**: 032 (workspace), 037 (the pass-store module and the repo-runner patterns for secrets, redaction and guards).

## Context and ground rules

- The reason `dbrun` is special [OBSERVED: `driver_process`, lines 1088-1135]: the host process is stdlib Python and does classification, masking, logging, plans and backups; the **database drivers run in a child process** `python dbrun.py _exec`, started with `uv run --no-project --with <pinned driver>` either on the host (remote targets) or inside a **client container** (`CLIENT_IMAGE`, an uv + Python 3.12 image pinned by digest) that joins the local database container's network namespace. Credentials travel only on the child's stdin. SQLite runs in-process with the stdlib.
- Drivers pinned today: `psycopg[binary]==3.3.6` (PostgreSQL), `PyMySQL==1.2.3` (MySQL and MariaDB), `pymssql==2.4.4` (SQL Server), `oracledb==26.0.1` (Oracle, thin mode; thick mode via `AI_GENT_ORACLE_CLIENT_LIB`). Engines in `ENGINES`: postgresql, mysql, mariadb, sqlserver, oracle, sqlite.
- Without `uv` and without a container engine, `dbrun` already fails with an install message (`EXIT_CONNECT` 4). So today the db plugin **requires uv or Podman/Docker**; a plain "install the release package" user without either cannot use it. That is the one place where the premise of the whole Rust effort ("a user installs once and needs neither uv nor Python") is not met.
- The safety logic is the valuable part and must not regress: the statement scanner and classifier, cost rules, masking, redaction, the local proof, plans/apply/restore and the command log. None of it depends on a driver.
- The decision gates other work: if the answer is "not yet", the guards, skills and `check.sh` blocks for db stay as they are and `ai-gent doctor` reports `uv` (or a container engine) as a requirement of the db plugin.

## User Scenarios & Testing *(mandatory)*

The "users" here are the owner (who decides) and the builder who runs the experiments.

### User Story 1 - A written decision with evidence (Priority: P1)

The owner reads one page, `specs/038-db-runner-spike/decision.md`, that picks one of four options with measured evidence and a recommended release.

**Why this priority**: D19 asks for exactly this; everything else feeds it.

**Independent Test**: `decision.md` exists, follows `contracts/decision-template.md`, every experiment E1-E8 has a result (pass, fail or not run with a reason), and no section contains "TBD".

**Acceptance Scenarios**:
1. **Given** all experiments ran, **Then** the decision names an option and, for each engine, native / container / stay.
2. **Given** an experiment could not run (no Windows machine, no Oracle image), **Then** it is marked "not run" with the reason, and the decision says what it does not cover.

### User Story 2 - Know what each option costs per engine (Priority: P1)

For each of the six engines the spike records: the Rust driver candidate (or the CLI inside an image), whether it runs without a native library, TLS support, timeout and statement-limit support, how result types map to the JSON the runner emits, and the effort in tasks.

**Independent Test**: `engine-matrix.md` has six rows and no empty cells.

**Acceptance Scenarios**:
1. **Given** PostgreSQL, **Then** the row compares a native Rust client with `psql` in the client image, on the checks of E2.
2. **Given** SQL Server and Oracle, **Then** the row states whether any maintained pure-Rust driver exists today and what it needs (sqlx dropped MSSQL support pending a rewrite and does not list Oracle [RESEARCHED: S3]; the status of dedicated SQL Server and Oracle crates was **not verified** [ASSUMPTION]; E5 and E6 check it).

### User Story 3 - Prove the safety logic is portable first (Priority: P1)

Before any driver question, the classification and masking logic is shown to port with identical results.

**Independent Test**: experiment E1 (the safety core ported as a driver-free Rust library) passes the classifier, cost, mask, redact and network tests as shared fixtures.

**Acceptance Scenarios**:
1. **Given** the 8 `ClassifierTest`, 2 `CostTest`, 3 `MaskTest`, 2 `RedactTest` and 1 `NetworkTest` tests, **Then** their cases run as fixtures against the Rust core with identical outputs.
2. **Given** the engine-specific scanning rules (backslash quoting for MySQL/MariaDB, bracket identifiers for SQL Server, `parse_modes`), **Then** `test_engine_specific` and `test_backslash_trick_refused` pass.

### User Story 4 - A "not yet" answer is a first-class outcome (Priority: P2)

If the verdict is "not yet", the spec lists exactly what stays, what changes (documentation, `doctor`), and when to revisit.

**Independent Test**: the "If the answer is not yet" section of `decision.md` is filled even when another option is chosen (it is the fallback plan).

## Requirements *(mandatory)*

- **FR-001**: The spike answers: (a) can the safety core move to Rust with no behaviour change; (b) which option is right: N (native drivers for all engines), H (hybrid: native for sqlite, PostgreSQL, MySQL and MariaDB; a container client for SQL Server and Oracle), C (container shell-out for every networked engine, native sqlite) or S (stay a Python script that needs `uv`); (c) in which release; (d) what the package ships.
- **FR-002**: Time box: the spike closes after E1-E8 or after 5 builder sessions, whichever comes first; unfinished experiments are reported as not run.
- **FR-003**: No production code replaces `dbrun.py` during the spike. Prototype code lives under `rust/spikes/dbrun/` (not shipped, excluded from the package allowlist, removed or kept per the decision). Prototypes never connect to a real remote database: only throwaway containers and a temp sqlite file (the `scripts/test-dbrun-engines.py` rule).
- **FR-004**: Any prototype that reads a credential follows `shared/secrets.md`: a throwaway container's fixture password only (the engines script's `Dbrun_Test_1`), never a real credential, never in argv, never printed.
- **FR-005**: Every experiment states its pass/fail rule before it runs, in `contracts/experiments.md`.
- **FR-006**: The verdict uses `contracts/decision-template.md`; the owner's sign-off is a field in it.
- **FR-007**: The engines in `scripts/test-dbrun-engines.py` are the acceptance set for any "yes": postgresql (`docker.io/library/postgres:17`, 512m), mysql (`mysql:8.4`, 1g), mariadb (`mariadb:11.4`, 512m), sqlserver (`mcr.microsoft.com/mssql/server:2022-latest`, 2g), oracle (`gvenzl/oracle-free:23-slim`, 2g). One engine at a time, container and volumes removed before the next, by ID. The script's checks: the local proof, masked and revealed reads, `--explain`, plan then apply then restore (the new table must be gone after the restore), a count-mismatch rollback, the system-database refusal; then the same server as a remote target declared `development`: the read-only refusals and a change script that is never executed.
- **FR-008**: The disk and memory rules of the engines script apply (a free-space warning; peak memory is one engine's cap, at most 2 GB); the spike does not override the disk floor.
- **FR-009**: Behaviour that must not regress is the checklist below; the decision signs off each item.

### Must not regress

1. **Classification** (`classify`, lines 478-535): local only when proven (a compose project in this repository for containers; sqlite files inside the repository and not symlinks); everything else is production, staging or development with `writable: false`. Tests: `ProfilesTest::test_declared_local_without_proof_is_production`, `SqliteEndToEndTest::test_remote_refusals`, `::test_remote_read_is_read_only_even_past_the_classifier`.
2. **Statement scanning** (`scan`, `split_statements`, `classify_statement`, `is_ddl`, `catalog_only`, lines 568-720): reads versus writes, `EXPLAIN` forms, comments and quoting per engine, the MySQL backslash trick, SQL Server brackets, multi-statement splitting. Tests: all of `ClassifierTest` (reads, writes, engine_specific, system_databases_refused_for_writes, explain_forms, split, backslash_trick_refused, ddl).
3. **Cost rules** (`cost_rules`): `SELECT *` and unfiltered counts on user tables refused on remote targets unless `--allow-full-count`; catalog and local exempt. Tests: `CostTest`.
4. **Masking** (`mask_value`, `mask_free_text`, `mask_result`, `luhn`, `keep_last`): column-name rules, Luhn-checked card numbers, `--reveal` keeps secrets redacted. Tests: `MaskTest`.
5. **Redaction** of host, user and password in any error or log. Tests: `RedactTest`, `ProfilesTest::test_errors_are_redacted`, `::test_listing_never_shows_secrets`.
6. **Command log written before a statement runs** (`log_start`, `log_result`), plans with a tamper check (`test_tampered_plan_refused`), backups taken by `apply` before writing, row cap, timeouts, `script` never executing. Tests: `SqliteEndToEndTest` (8).
7. **Secrets**: pass first, `pass:<entry>` references, family entries, `secrets migrate`; profiles from the config directory; credentials only on the driver's stdin. Tests: `PassTest`, `TomlTest`, `MigrateAllTest`.
8. **Guards**: `plugins/db/hooks/guard.sh` and `opencode/guard/rules.mjs` (lines 147-218: only dbrun reads credential files; direct client calls with write SQL are denied or asked), `scripts/test-db-guard.sh` (4 mentions of `dbrun.py`), `scripts/test-secrets-guard.sh` (1), `opencode/guard/test/rules.test.mjs` (5). If the command word changes (`ai-gent db`), the patterns change with it in the same release; if it stays, they do not.

## Success Criteria *(mandatory)*

- **SC-001**: `decision.md` exists, follows the template, covers all six engines and E1-E8, and carries the owner's sign-off or an explicit "pending".
- **SC-002**: E1 passes: the 16 classifier, cost, mask, redact and network tests (and the engine-specific scan cases) are green as shared fixtures on a Rust core library.
- **SC-003**: For every engine marked "native" or "container" in the verdict, a prototype ran the engines script's checks for that engine and passed, or the verdict marks the engine "unproven".
- **SC-004**: The verdict states the user-visible requirement after the change in one sentence ("needs nothing", "needs Podman or Docker for SQL Server and Oracle", "needs uv") and lists the doc pages that say otherwise.
- **SC-005**: The size cost is measured: release binary size with and without the chosen drivers (a number, not an estimate).
- **SC-006**: No file under `plugins/db/` changes during the spike.

## Edge Cases

- Oracle thin versus thick mode: `oracledb` is thin by default (no client libraries); a Rust option that needs Oracle Instant Client reintroduces a native dependency the package cannot ship [ASSUMPTION about the Rust Oracle crates; E6 checks].
- SQL Server TLS: older servers negotiate TLS 1.0-1.2; some pure-Rust TLS stacks refuse legacy settings. E5 uses the `mssql/server:2022-latest` image and notes the encryption mode.
- MariaDB versus MySQL timeouts: `max_statement_time` (MariaDB) versus `max_execution_time` (MySQL) are set per session after `SELECT VERSION()`; any new client must do the same.
- Read-only enforcement is per engine in the Python (`default_transaction_read_only=on` for PostgreSQL, read-only transactions elsewhere); each prototype must prove a write is refused by the server, not only by the scanner (`test_remote_read_is_read_only_even_past_the_classifier`).
- Windows: containers need Docker Desktop or a Podman machine; the client container joining another container's network namespace (`network_args`) has Linux semantics; E7 tests it or marks it not run.
- The pinned client image digest and driver pins are security-relevant (credentials go to them); any new pin follows the same "bump only after the engines script passes" rule.

## Out of Scope

- Implementing the port; changing any skill, reference or guard; removing `uv` from other tools; a TUI; new engines; a connection pool; async execution.
- Keeping Python alive "just for db" as a silent fallback: if the answer is S, the requirement is stated openly in the db plugin docs and `ai-gent doctor`, not hidden.

## Open Questions

1. **Is "db needs uv (or a container engine)" acceptable in 3.0.0?** D19 says last or never. Recommended: yes for 3.0.0; revisit after the spike. It is the honest state today.
2. **Command word if db moves**: `ai-gent db` (D9 says "later `db`"). Recommended: yes, one binary; the guard patterns change in the same release.
3. **Hybrid (H) as the default hypothesis.** Recommended to test first: native for sqlite and PostgreSQL (and MySQL/MariaDB if one client handles both server families and the per-session timeouts), a container client for SQL Server and Oracle, because the Rust ecosystem for those two is the least certain [ASSUMPTION; S3 shows sqlx dropped MSSQL and lists no Oracle].
4. **Who runs E7 and E8** (Windows and aarch64 container behaviour)? Needs the owner's machines.
5. **Engines script ownership**: keep `scripts/test-dbrun-engines.py` as the acceptance harness after a port (Python stays a dev-only tool per D16), pointed at the new binary. Recommended: yes.

## Sources (web research, retrieved 2026-11-01)

| # | Claim | URL | Confidence | Label |
|---|---|---|---|---|
| S3 | `sqlx` 0.9.0 supports PostgreSQL, MySQL, MariaDB and SQLite; MSSQL support was removed pending a rewrite (it existed before 0.7); Oracle is not mentioned; works with tokio; TLS via native-tls or rustls (TLS 1.2 and 1.3 for rustls) | https://docs.rs/crate/sqlx/latest | Medium (one source) | [RESEARCHED: S3] |

A second search returned only secondary articles about `tokio-postgres`, `mysql_async` and `tiberius`; none was fetched, so nothing about those crates, or about an Oracle crate, is claimed here beyond [ASSUMPTION] markers. Source numbers S1-S4 are shared across specs 035-040 (S1 and S4 are in 037, S2 in 040).
