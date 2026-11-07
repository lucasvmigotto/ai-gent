# Feature Specification: `dbrun` production port to Rust (`ai-gent db`)

**Feature Branch**: `docs/spec-043-dbrun-port` (specification only; the work lands on its own branches, see `plan.md`)

**Created**: 2026-11-07

**Status**: Draft (for the owner's review; nothing here is approved until the owner says so)

**Input**: eight owner decisions from an owner-run questioning session on 2026-11-07, quoted below as "relayed" (they reached this draft through the supervising agent, not from a file the owner wrote), and twelve answers the owner gave in chat on 2026-11-07 to the draft's 14 open points, recorded below in the `shared/asking.md` rounds format; `specs/038-db-runner-spike/{spec,plan,tasks,results,measurements}.md` (the spike, its drivers in `rust/spikes/dbrun/` and the promoted pieces in `rust/crates/ai-gent-dbrun/`); `specs/039-release-3-contract/` (D26/D27, now amended); `plugins/db/` (`scripts/dbrun.py`, `references/runner.md`, `hooks/guard.sh`, the four skills).

**Depends on**: 032 (workspace, release), 037 (pass-store module, `ai-gent repo` patterns for secrets and redaction), 038 (the spike and its review findings), 039 (the release contract this spec amends). Read by: 039 (gates), the db skills.

## Owner decisions on record (relayed, 2026-11-07)

1. 3.0.0 is held until `dbrun` is ported to Rust. This reverses D26/D27 of spec 039, which kept `dbrun` Python and made its port the last 3.x step.
2. All five networked engines in Rust (SQLite, PostgreSQL, MySQL/MariaDB, SQL Server, Oracle thin). No Python left in the db plugin.
3. The port gaps are closed before 3.0.0: TLS verification (profile fields for a CA file or the system roots), the Oracle old-verifier case, native backup and restore.
4. Oracle: thin mode is the default. Thick mode through the `oracle` crate is optional, for a user who installs Instant Client themselves; a path environment variable points at the library; client libraries are never shipped. T008 of spec 038 becomes a real task.
5. This spec (043) is the production port: one worker per engine and per shared-core task, each output reviewed by a different agent, landing by pull requests into `dev`.
6. The 035 and 036 cutovers (their T060-T064) ship together with `dbrun` at 3.0.0 as one breaking release (B1-B12 of spec 039).
7. 2.28 releases as soon as its gates pass, ahead of 3.0.0, carrying the update notice and the guard launcher.
8. Owner-side preparation (gate evidence tables, the rollback rehearsal script T051, the `develop` rename plan, the platform-probe checklist) is deliberately not written here.

The eight decisions above are still relayed: they have not been re-confirmed from a file the owner wrote. The twelve below were given by the owner directly in chat in three question rounds on 2026-11-07 (the dates in `plan.md` "Decisions" are that date). Relayed decision 6 (cutovers ship together) is refined, not contradicted, by chat decision 11.

(SQLite is counted among the five engines in decision 2 as the relayed list gives it; MySQL and MariaDB share one driver, so "five engines" are five drivers: SQLite, PostgreSQL, MySQL/MariaDB, SQL Server, Oracle.)

## Owner decisions given in chat (2026-11-07, three rounds)

Each line is `Decided: <choice> — <reason>`; the requirement that carries it is in brackets. They settle all but a few details of the draft's 14 open points (see `plan.md` "Decisions" and "Still open").

1. Decided: the command word is `ai-gent db`; the db skills and the db guard call it — spike 038's recommendation. [FR-001]
2. Decided: backup and restore are driven by Rust through the container exec with the same engine tools (`pg_dump`, `mysqldump`, `BACKUP DATABASE`, ...), no Python and no Rust logical dump — the tools are proven and a Rust dump risks types, routines and grants. [FR-018]
3. Decided: profiles verify the server certificate by default (system roots or a named CA file); a profile proven local on loopback may opt out with an explicit `unverified`; `unverified` is refused on a production profile — safe by default, with a door for throwaway local databases. [FR-015]
4. Decided: keep the `tiberius-ng` fork (checksum pinned; plain `tiberius` is the measured drop-in fallback); send a plan's statements as one batch so no count probe runs between them, reading the count once at the end or from DONE tokens if the fork exposes them; not vendored — it avoids the `@@ROWCOUNT` finding without patching the dependency. [FR-012, FR-021]
5. Decided: each engine call runs in a short-lived child process of `ai-gent db`, so a panic or crash becomes a clean failure JSON; `panic = "abort"` stays — it contains the Oracle thin `unwrap` risk and gives uniform deadlines. [FR-014]
6. Decided: Oracle thin refuses an account without a 12c verifier with a clear message naming the cause and pointing to thick mode; TLS through the crate's PEM wallet or a CA file, with no system-roots promise; no 11g/12.1 image is pulled now — the cost of implementing old verifiers and of another image is not justified for a beta driver. [FR-016, FR-025]
7. Decided: thick mode reuses `AI_GENT_ORACLE_CLIENT_LIB`, is a separate optional crate or feature built only on targets where a client exists, is never in the shipped package, and thin stays the default everywhere. [FR-026]
8. Decided: the Rust values are the contract and are documented as intended 3.0.0 changes: DDL row count `0` (not `-1`) and `NUMBER` typed from its text; they go into the contract, the parity corpus and the migration guide. [FR-022]
9. Decided: a profile that publishes no port keeps the container exec path (the engine's client in the container, as `session` does now); a profile with a published port uses the Rust drivers — no client image or namespace join is rebuilt for the native path. [FR-020]
10. Decided: the platforms are all six release targets; an engine driver that cannot build on a target is waived for that target (3.0.0 ships it without that engine, with a clear refusal and a line in the compatibility matrix) rather than blocking the release. [FR-029, FR-030, FR-035]
11. Decided: held cutovers (035, 036, 037, 043) merge to `dev` as they are ready but stay inert for packages until the single cutover PR deletes the Python scripts; 2.28 releases from `dev` with the cutover edits not applied — nothing waits on a long-lived branch. [FR-036]
12. Decided: 038's `decision.md` is drafted by task T001 of 043 from 038's results and these answers; the owner approves it in the PR before any engine group starts. [FR-037]

## Context and ground rules

- What exists today [OBSERVED: `plugins/db/scripts/dbrun.py` and the `rust/` tree on `origin/dev`, `specs/038-db-runner-spike/results.md`]:
  - `dbrun.py` (about 1850 lines) is stdlib Python on the host: profiles, classification, masking, redaction, plans, backups, the command log. The drivers run in a child, `dbrun.py _exec`, started with `uv run --no-project --with <pinned driver>` on the host or inside a digest-pinned client container that joins the database container's network namespace. Credentials travel on the child's stdin only.
  - `rust/crates/ai-gent-dbrun` already holds the safety core (scanner, splitter, classifier, cost rules, mask, redact; a golden file generated from the Python), the `Connector` trait, the process connector (drives `dbrun.py _exec`), the SQLite connector and `core::plan` (plan records, tamper check, apply, script). Cargo feature: `sqlite` only. Not there yet: any networked engine, a CLI, profiles, the command log, backup/restore of a real engine.
  - `rust/spikes/dbrun` holds throwaway drivers: `pg.rs` (`postgres` 0.19.14), `mysql.rs` (`mysql` crate, MySQL and MariaDB), `mssql.rs` (`tiberius-ng` 0.13.1, rustls), `oracle_thin.rs` (`oracledb` `=26.0.0-beta.5`). Each passed the engines script through an adapter binary (17 of 17 checks for the first three, 14 of 14 for Oracle). Spike only: not shipped, not in the package.
  - Spike measurements: all four drivers plus SQLite build to 5.58 MB (x86_64 gnu, `opt-level = "z"`, LTO, strip); cold start 0.6-7 ms against 46 ms for a warm `uv run`. Not run: musl for sqlite, mysql and mssql (failed for want of a musl C compiler), aarch64, Windows, macOS.
- The safety logic is the valuable part and must not regress: classifier, cost rules, masking, redaction, the local proof, plans/apply/restore, the command log written before a statement runs, `script` never executing, credentials never in argv or output. `sqlparser` stays rejected as the classifier (038 FR-010).
- "No Python left" is a requirement of the final state, not of each intermediate pull request. While the port is under way the process connector may keep an engine working through `dbrun.py _exec`; it is deleted in the cutover (FR-020).
- The real-engine suite `scripts/test-dbrun-engines.py` (038 T003 gave it `--runner`) is the acceptance harness for every engine and stays a dev-only Python script (D16) pointed at the Rust binary.
- Credentials: `shared/secrets.md` applies to every task. Tests use throwaway container passwords only; no worker reads a real credential.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - The db plugin runs without Python or uv (Priority: P1)

A person installs the release package, adds a profile and runs `db:inspect` against PostgreSQL, MySQL, SQL Server, Oracle or SQLite. Nothing but the package binary (and, for a local database container, a container engine) is needed.

**Why this priority**: it is the premise of the Rust effort that 038 found unmet only for `db`, and the reason 3.0.0 is now held.

**Independent Test**: on a machine with neither `uv` nor Python, `ai-gent db test <profile>` and `ai-gent db query ...` succeed against each engine of the engines script; `ai-gent doctor` reports no Python or uv requirement for any plugin.

**Acceptance Scenarios**:
1. **Given** a package install and no `uv` or `python3` on `PATH`, **When** the engines script runs with `--runner` set to the binary for each engine, **Then** every engine passes all of its checks.
2. **Given** the 3.0.0 candidate package, **When** `grep -rEn 'dbrun\.py|uv run' plugins shared README.md AGENTS.md` runs, **Then** it finds only the migration guide and the changelog.

### User Story 2 - Same safety, byte for byte where it is observable (Priority: P1)

Everything a skill or a person can observe of `dbrun.py` (verdicts and reasons, exit codes, masked output, redacted errors, log entries, plan files, `--json` shapes) is the same from the Rust command, except where this spec lists a deliberate difference.

**Independent Test**: the parity corpus (FR-004) generated from the Python runs in `scripts/check.sh` and passes with an explicit, reviewed list of accepted differences.

**Acceptance Scenarios**:
1. **Given** the 171-string classifier corpus and the 16 core tests of `test_dbrun.py`, **Then** verdicts, reasons, splits, `is_ddl`, cost cases, mask and redact outputs match.
2. **Given** the 41 defined tests of `test_dbrun.py` (51 runs), **Then** each has a Rust equivalent or a recorded reason it is obsolete.
3. **Given** a write sent in read mode on every engine, **Then** the server refuses it where a server can (038 FR-011), and SQL Server relies on the classifier plus rollback, stated in the docs.

### User Story 3 - Remote connections are verified (Priority: P1)

A person connects to a remote PostgreSQL, MySQL, SQL Server or Oracle with TLS. The certificate is verified against the system roots or a CA file named in the profile; skipping verification is an explicit, logged, per-profile choice and never the default.

**Why this priority**: relayed decision 3. The spike used `NoTls` (PostgreSQL), no TLS (MySQL) and `trust_cert()` (SQL Server), all unacceptable outside a throwaway image (038 results E2, E3, E5).

**Independent Test**: for each engine, a server with a certificate signed by a test CA: connect succeeds with the CA file, fails without it with a redacted error naming the TLS problem, and succeeds with the explicit unverified opt-in only on a proven-local loopback profile (after a logged warning); `unverified` on a production profile is refused.

### User Story 4 - Backup and restore without the Python helper (Priority: P1)

`apply` takes a backup of a proven-local database before writing and `restore` brings it back, for every engine that has a backup method today (SQLite, PostgreSQL, MySQL/MariaDB, SQL Server), implemented natively (what "natively" means is FR-018).

**Independent Test**: the engines script's plan, apply, restore journey passes on each engine and the table created by the plan is gone after the restore.

### User Story 5 - Oracle works on thin by default and on thick when the user opts in (Priority: P2)

A person connects to Oracle with no client libraries. An account whose password verifier the thin driver cannot use gets a clear refusal naming the cause and pointing to thick mode, and, if the person installed Instant Client and set the path variable, the same request works through thick mode.

**Independent Test**: the engines script on `gvenzl/oracle-free:23-slim` passes on thin; an old-verifier account gets the refusal of T062; with a real Instant Client present, thick passes the same journeys; with none, the error says what to set.

### User Story 6 - One breaking release (Priority: P1)

The owner merges `dev` into `main` once: 3.0.0 carries the 035 and 036 cutovers, the `dbrun` cutover and everything in spec 039, and `scripts/release.py` plans the major.

**Independent Test**: spec 039's amended G30 gates are green and `python3 scripts/release.py --dry-run --json` prints `"version": "3.0.0"`.

## Requirements *(mandatory)*

### Command and contract

- **FR-001**: The command is `ai-gent db <subcommand>` with the subcommands, flags, defaults and exit codes of `dbrun.py` (`profiles`, `classify`, `test`, `query`, `plan`, `apply`, `restore`, `script`, `session`, `secrets migrate`, `log`; exit codes 0, 1, 2, 3 policy refusal, 4 connection failed). The command word is `ai-gent db` (chat decision 1, 2026-11-07); the guard patterns, fixtures and skills change to it in the cutover.
- **FR-002**: The connector contract is the Python `_exec` request and result, frozen as a versioned fixture set (`rust/fixtures/db-exec/`): request with engine, connection facts, mode, statements, timeout, row cap, expected counts; result one JSON line; failed stages `request`, `connect`, `execute`, `verify`. A native connector returns the same shapes; a new field is additive and tagged.
- **FR-003**: Output formats `table`, `csv`, `json`, `--explain`, `--reveal`, `--max-rows` (default 100) and `--timeout` (default 30; plan and apply 120) behave as today; lock waits time out after 5 seconds.
- **FR-004**: A parity corpus generated by running the real `dbrun.py` (a committed generator and golden, in the style of `rust/spikes/dbrun/gen_golden.py`) covers: classify verdict and reason, split, `is_ddl`, `catalog_only`, cost rules, mask, redact, network arguments, CLI exit codes and `--json` output for `profiles`, `classify` and `query`, the plan record format and the log entry format. The 038 corpus covered reads only; this one adds writes, plan/apply/restore, errors and refusals.
- **FR-005**: A list of accepted differences (`specs/043-dbrun-port/differences.md`, created by the shared-core worker) records every place the Rust output intentionally differs (for example row counts, error text carrying an engine error number, encryption negotiated where Python did not). A difference not on the list fails the parity run.

### Safety (must not regress; 038 "Must not regress" 1-8 apply in full)

- **FR-006**: Local is proven, never declared (a compose project in this repository for containers; SQLite files inside the repository and not symlinks). Everything else is production, staging or development and read-only.
- **FR-007**: Read mode is refused by the server where a server can: SQLite (`mode=ro` plus an authorizer), PostgreSQL (`default_transaction_read_only=on` and `BEGIN READ ONLY`), MySQL/MariaDB (ERROR 1792), Oracle (read-only transaction). SQL Server has no server-side read-only mode and relies on the classifier plus rollback, said plainly in `plugins/db/references/engines/sqlserver.md`.
- **FR-008**: Redaction replaces host, user and password with `<host>`, `<user>`, `<password>` in every error and log line, including a password or user that a profile gives as a number (a review finding on `oracle_thin.rs`, fixed there; the shared redactor covers it for every engine, with a test).
- **FR-009**: Connection fields that structure a connect string or descriptor (`(`, `)`, `=`, a quote, a backslash, whitespace) are refused in `host`, `port`, `service`, `database` and `sid` before any descriptor is built (the Oracle review finding), as one shared validator used by every engine that builds a string from profile fields.
- **FR-010**: Row counts come from the server, never from parsing statement text (PostgreSQL review finding 1). `max_rows` applies in read and write mode and `truncated` is true only in read mode, as Python (PostgreSQL review finding 2).
- **FR-011**: Every statement and every control batch (session setup, identity, transaction, showplan, count probe) runs under the request's timeout (SQL Server review finding). A missing timeout means 30 s; a negative, non-finite, non-numeric or oversized timeout is a `connect`-stage JSON error, never a panic (MySQL review finding).
- **FR-012**: The count a plan verifies is the server's own count for that statement and is not visible to the next statement. SQL Server must not send a count probe between a plan's statements (the `@@ROWCOUNT` finding): a plan's statements are sent as one batch, and the count is read once at the end or from DONE tokens if the fork exposes them (chat decision 4, 2026-11-07). `tiberius-ng` exposes DONE counts only through its RPC path (which loses the column names of an empty result) and not through its batch path; the worker proves which read is available and records it, but the one-batch shape is decided.
- **FR-013**: The command log entry is written before a statement runs and again with the result; plans carry the tamper check; `apply` backs up before writing; `script` never executes (038 must-not-regress 6).
- **FR-014**: A panic inside a connector never aborts the process silently. The release profile stays `panic = "abort"`, under which a panic cannot be caught (the Oracle thin login indexes server fields with `unwrap`; its `catch_unwind` does nothing under abort, 038 results E6 review). Decided (chat decision 5, 2026-11-07): each engine call runs in a short-lived child process of `ai-gent db` (`ai-gent db _exec`: request on stdin, one JSON line out, a deadline, a kill), so a panic or crash becomes a failure JSON at stage `connect` or `execute`. It mirrors today's `_exec` contract. Unwinding builds and forking `oracledb` are not pursued.

### TLS

- **FR-015**: Profiles gain TLS fields for every networked engine: a mode (verify against the system roots; verify against a CA file; unverified), a CA file path, and a server-name override for engines whose client supports one. The Python runner's behaviour (PostgreSQL `sslmode=prefer` through libpq, no explicit Oracle TLS, pymssql without encryption) is a baseline to improve on, not to copy. Decided (chat decision 3, 2026-11-07): the default is to verify the server certificate (system roots, or the named CA file), with a hard, redacted failure naming the profile field to set; this is an intended 3.0.0 behaviour change for profiles that worked unencrypted and goes in the migration guide. A profile proven local on loopback may opt out with an explicit `unverified`; `unverified` is refused on a profile classed production. Unverified mode is only ever an explicit profile value and is logged on every connection. Still open: the exact field names and file keys (T007 proposes them for the owner's review in its PR).
- **FR-016**: TLS is rustls with the system root store (`rustls-native-certs`, already enabled in the spike). No OpenSSL, no `native-tls`. Oracle thin is the exception (chat decision 6, 2026-11-07): `oracledb` ships the `webpki-roots` bundle and supports a PEM wallet only, so Oracle TLS goes through the crate's PEM wallet or a CA file and the docs make no system-roots promise for Oracle.
- **FR-017**: Each engine has a test against a server whose certificate is signed by a throwaway CA generated in the test: verified with the CA file, rejected without it, unverified opt-in. The spike never ran a TLS handshake on PostgreSQL, MySQL or Oracle and ran SQL Server only against a self-signed image (038 "Not verified").

### Backup and restore

- **FR-018**: Backup before `apply` and `restore` are implemented without Python for SQLite (the backup API or a file copy under a lock), PostgreSQL, MySQL/MariaDB and SQL Server. "Native" means no Python, not no external tool (chat decision 2, 2026-11-07): Rust drives the same engine tools through the container exec (`pg_dump`, `mysqldump`, `BACKUP DATABASE` and the restores), needing a container engine, which proven-local databases already need. There is no Rust logical dump. Still open: whether Oracle gets a backup (Data Pump); until the owner decides, `apply` stays refused on Oracle, as today.
- **FR-019**: A restore recreates the whole database as today, so objects a plan created disappear; system databases are refused for plan and apply (SQL Server `master`, `msdb`, ...; MySQL `mysql`, `sys`, ...). `BACKUP DATABASE` and `RESTORE DATABASE` on SQL Server need autocommit, so the SQL Server connector offers an autocommit path (the spike's adapter always ran a transaction).

### Engines

- **FR-020**: When the last engine is native, the process connector, `dbrun.py`, `test_dbrun.py` (replaced by Rust tests and the parity corpus), `plugins/db/scripts/passstore.py` (replaced by 037's pass-store module) and the `uv` and client-image machinery (`CLIENT_IMAGE`, `uv run --with`, driver pins) are deleted in the cutover. Decided (chat decision 9, 2026-11-07): a profile with a published port uses the Rust drivers over TCP; a profile that publishes no port keeps the container exec path (the engine's own client inside the database container, as `session` does now), so no client image and no namespace join of a separate container are rebuilt. `CLIENT_IMAGE` and `uv run --with` are deleted; the container exec path uses the clients already in the database container. `session` stays and means that path.
- **FR-021**: Drivers from the spike: SQLite `rusqlite` bundled; PostgreSQL `postgres` 0.19.14; MySQL and MariaDB the `mysql` crate (one client; the timeout variable is chosen after `SELECT VERSION()`: `max_execution_time` for MySQL, `max_statement_time` for MariaDB); SQL Server `tiberius-ng` 0.13.1 over rustls; Oracle thin `oracledb` `=26.0.0-beta.5` (pure Rust, beta, Oracle's own repository); Oracle thick `oracle` 0.6.3 (ODPI-C). Versions are pinned exactly and bumped only after the engines script passes, as the Python pins were. Decided (chat decision 4, 2026-11-07): keep the `tiberius-ng` fork with its checksum pinned (a single-maintainer fork on a credential path; plain `tiberius` 0.13.0 answered identically and is the measured drop-in fallback); not vendored. Still open: whether `mssql` becomes its own crate (the D28 candidate). Oracle thin is a beta: the owner accepted thin as the default (relayed 4); the risk stays in the plan.
- **FR-022**: Value rendering matches the Python driver where it is observable and every deliberate difference is on the FR-005 list. Items from the spike reviews: MySQL fractional datetimes padded to six digits as `isoformat`, TIME as `str(timedelta)`; SQL Server `datetimeoffset` as wall time with its offset, `money` as a four-decimal string, `sql_variant` typed (pymssql returns raw bytes); the DDL row count (`0` in the spike where psycopg and pymssql report `-1`); Oracle `NUMBER` typed from its text (an integer-valued `NUMBER(10,2)` prints `5` where python-oracledb probably gives `5.0`, and `1E-130` stays a string). Decided (chat decision 8, 2026-11-07): the Rust values are the contract. The DDL row count is `0` (not `-1`) and `NUMBER` is typed from its text; both are listed in `differences.md` as intended 3.0.0 changes, pinned in the connector contract (T002) and the parity corpus (T003), and stated in the migration guide (T097).
- **FR-023**: Error text carries the engine's error number where the engine gives one (SQL Server `MssqlError <number> (state, class)`, PostgreSQL through `as_db_error()`, the MySQL error code, Oracle `ORA-`), redacted per FR-008; skills that match error text are updated in the cutover.
- **FR-024**: The Oracle thin driver has no total login time bound (the transport timeout covers the TCP connect only) and sleeps one second after every failed connect unless the connect descriptor carries `RETRY_DELAY=0` (an Easy Connect `retry_delay=0` is ignored). The connector always sends a descriptor and puts the login under its own deadline (the child process of FR-014 provides one).

### Oracle

- **FR-025**: The thin driver cannot log in to an account whose only verifier is the 11G one (`ORA-01017` for an 11G-only account that python-oracledb thin and sqlplus accept; the crate implements only the 12c PBKDF2 path), and the 10G case (DPY-3015 in Python) was never reproduced. Closing it is part of 3.0.0 (relayed 3). Decided (chat decision 6, 2026-11-07): thin does not implement the older verifiers. An account without a 12c verifier gets a clear refusal that names the cause (the verifier) and points to thick mode with Instant Client. No 11g/12.1 image is pulled now, so the 10G case stays unreproduced and is recorded "unverified"; the 11G-only account of the spike (reproducible on the current image) tests the refusal text. This closes relayed 3's old-verifier gap by a clear refusal plus the thick path, which the owner chose.
- **FR-026**: Thick mode is a Cargo feature (`oracle-thick`) over the `oracle` crate. ODPI-C loads the client library at run time, so a build needs only a C compiler and the package ships no Oracle library, ever. The library is found through `AI_GENT_ORACLE_CLIENT_LIB`, the variable name Python used (chat decision 7, 2026-11-07). A missing library is a `connect`-stage error that names the variable and says to install Instant Client; never a panic or a raw DPI-1047 dump. Thick is a separate optional crate or feature, built only on targets where a client exists and never part of the shipped package; thin stays the default everywhere. Still open (small): the selection rule when the variable is set (assumed: thick is used only when the variable is set, with no automatic fallback after a thin verifier refusal, which only points to thick); T070 states it and the owner confirms in the PR.
- **FR-027**: Thick mode cannot be tested without a real Instant Client, which an agent cannot download in its sandbox (the spike's probe with the server home's libraries segfaulted, so those do not stand in for it). The thick test is an owner-run check recorded as gate evidence, not a CI job; CI covers the build and the missing-library path.
- **FR-028**: The thin driver's other recorded gaps stay recorded in `plugins/db/references/engines/oracle.md`: SID connections through to a login, RAC and DRCP, wallets other than PEM, Autonomous Database, PL/SQL OUT binds and DML `RETURNING` (Python fails the same way with DPY-4010), LOB contents, `TIMESTAMP WITH LOCAL TIME ZONE`, servers older than 26ai.

### Package, platforms, release

- **FR-029**: `ai-gent-dbrun` features: `sqlite`, `postgres`, `mysql`, `mssql`, `oracle` (thin), `oracle-thick`. The binary's default features per target come from the platform matrix. In the spike musl failed for sqlite, mysql and mssql (no musl C compiler), and aarch64, Windows and macOS were not run. The targets are all six release targets of spec 032 (chat decision 10, 2026-11-07). Waiver policy: an engine driver that cannot build on a target is waived for that target rather than blocking the release; 3.0.0 ships that target without that engine, `ai-gent db` refuses the engine there with a clear message naming the platform, and the compatibility matrix carries a line for it. A waiver is never silent: it is listed in `specs/043-dbrun-port/platforms.md` and the migration guide. SQLite is not waivable unless the owner says so (assumed, since all profiles of a local file rely on it).
- **FR-030**: Size budget: derived from existing measurements, not newly asked. Spec 032's `data-model.md` budget for `ai-gent` is at most 8 MB stripped per target (an [ASSUMPTION] there, archive at most 10 MB); spec 038 measured all four drivers plus SQLite at 5.58 MB on x86_64 gnu and the 032 core binary at 0.55 MB, so the sum (about 6.1 MB) fits with margin. 043 adopts the 032 budget; a target over it is measured and reported in `platforms.md`, and the owner decides case by case (a numeric budget is not left open). The sum is an estimate; the first combined build (T091) replaces it with a measured figure. Still open: whether the crate's own full-feature `regex` (needed for Unicode `\w`) is acceptable once the binary depends on the crate, to be answered by a measurement in T091.
- **FR-031**: Skills, references, the db guard (`plugins/db/hooks/guard.sh`, `opencode/guard/rules.mjs` while it exists, the Rust guard evaluator of 033), `shared/guard-fixtures/{db,secrets,containers}.json`, `scripts/check.sh`, the engines script and its CI job, `README.md`, `AGENTS.md` and the docs site switch to the new command in one cutover pull request together with the deletion of FR-020; `ai-gent doctor` drops the db `uv` check.
- **FR-032**: Spec 039 is amended (listed in `plan.md`, "Amendments to 039", and done in the same change set) so that 3.0.0 includes this port; 2.28 stays ahead of it (relayed 7).
- **FR-033**: Every task's output is reviewed by a different agent from the one that wrote it, and the review is recorded in the task's evidence line (reviewer, findings, fixed or recorded). Landing is by pull request into `dev`, one at a time, with the owner merging.
- **FR-034**: `ai-gent-dbrun` publishes after `ai-gent-core` and before `ai-gent` (D28). Trusted Publishing is configured by the owner; an agent never handles a token (038 T024).

- **FR-035**: Waiver policy and compatibility matrix (chat decision 10). Each of the six targets is built with every engine feature; a failing engine/target pair is recorded in `platforms.md` with the reason, waived for that target only, refused at run time with a message naming the engine and the platform, and listed in the docs-site compatibility matrix and the migration guide. A waiver needs no owner signature but appears in the release notes; a target that cannot build the binary at all, or SQLite, still blocks (assumed, see FR-029).
- **FR-036**: Held cutovers (chat decision 11). The 035, 036, 037 and 043 cutover tasks merge to `dev` as they are ready but stay inert for packages (they add the new command and tests and do not delete a Python script or change a skill's command word in the packaged files) until the single cutover pull request deletes the Python scripts and applies the skill, guard and allowlist edits. 2.28 releases from `dev` with the cutover edits not applied. No long-lived cutover branch exists.
- **FR-037**: 038's `decision.md` (chat decision 12). Task T001 drafts it from 038's results and the chat decisions; the owner approves it in that pull request, and no engine group (B-F) starts before that approval.

### Key entities

Profile (engine, connection facts, TLS fields, secret reference), request (the `_exec` JSON), result (one JSON line), plan record, backup, command-log entry, parity corpus, accepted-differences list.

## Edge cases

- A profile written for the Python runner has no TLS fields: the default is now verify (FR-015, chat decision 3), so a remote profile that worked unencrypted or with an unverifiable certificate fails with an error naming the field to set; the migration guide says what changes. A proven-local loopback profile may set `unverified`; a production profile may not.
- A password that is all digits, or contains a quote or a newline: redaction and descriptor validation cover it (FR-008, FR-009).
- A server that refuses encryption (an older SQL Server, a TLS-less development database): the connection fails with a redacted error that names the profile field to set; the Python runner used to connect to SQL Server unencrypted. For a proven-local container this is where an explicit `unverified` applies.
- MySQL's `max_execution_time` covers `SELECT` only; a write running past the timeout is bounded by the socket timeout plus `lock_wait_timeout`, as in Python. The spike did not exercise it; a test is added.
- SQL Server `SET NOCOUNT ON` issued by a user statement, several result sets in one request, `hierarchyid`, spatial and CLR types: untested in the spike; tests are added or the limit is recorded.
- MySQL 5.7, Percona, MariaDB older than 11.4, Azure SQL, SQL Server 2019 and older, named instances, Windows and Azure AD authentication, Oracle RAC: outside the acceptance set (038 FR-007); each is listed as unsupported-until-tested in the engine references.
- An engine waived on a target (FR-035): `ai-gent db` on that target refuses profiles of that engine with a message naming the engine and platform; the other engines work.
- A thick-mode request on a target where the optional crate was not built: the same clear refusal, naming `AI_GENT_ORACLE_CLIENT_LIB`.
- Unix-socket connections: the spike's adapter used TCP only; what `dbrun.py` does for sockets is checked in T010 and kept or dropped on the differences list.
- A second `apply` on the same plan: the plan record's state prevents it, as in Python; a test pins it.
- Windows: the namespace join of `network_args` has Linux semantics; on Windows a profile with no published port uses the container exec path of FR-020 (the engine's client in the container), which needs no namespace join.
- The weekly engines job pulls about 5.5 GB of images: one engine at a time, the disk floor applies, containers go by ID or label and never a prune (`shared/containers.md` 9a).

## Success Criteria *(mandatory)*

- **SC-001**: `scripts/test-dbrun-engines.py --runner <ai-gent db>` passes every check for sqlite, postgresql, mysql, mariadb, sqlserver and oracle (thin), on a machine with no `uv` and no Python on `PATH` for the binary under test.
- **SC-002**: The parity corpus is green with only listed differences; each of the 41 `test_dbrun.py` tests maps to a Rust test or an obsolescence note in `differences.md`.
- **SC-003**: Per engine, the TLS tests (FR-017) pass against a throwaway CA; `trust_cert` and `NoTls` appear in no production code path (grep).
- **SC-004**: Plan, apply and restore pass on every engine that has a backup, without Python; the `pg_dump`, `mysqldump` and `BACKUP DATABASE` calls go through the container exec as FR-018 decided, and no Python or Rust logical dump exists.
- **SC-005**: `grep -rEn 'dbrun\.py|uv run|CLIENT_IMAGE' plugins shared scripts opencode README.md AGENTS.md docs/site/src` finds only the migration guide, the changelog and specs.
- **SC-006**: The release package carries `ai-gent db` for every target in the platform matrix; `scripts/package.py --check` and `scripts/test-package.sh` are green; size within FR-030 (the spec 032 budget) per target not waived.
- **SC-007**: The owner-run checks have evidence: thick Oracle with a real Instant Client and the clear refusal of an old-verifier account pointing to thick, and any platform CI cannot build (or its recorded waiver).
- **SC-008**: Spec 039's amended G30 gates are green and the dry run prints 3.0.0.

## Out of scope

- New engines, a TUI, a connection pool, async execution, a general SQL validator (the crate README says so).
- Shipping any Oracle client library.
- Windows authentication, Azure AD tokens, SQL Server named instances, Autonomous Database wallets beyond PEM.
- The owner-prepared items of relayed decision 8.
- Changing the db skills' behaviour beyond the command word and the TLS fields; changing `shared/secrets.md`.

## Open points

The 14 points of the first draft are decided (the `## Owner decisions given in chat` section; `plan.md` "Decisions"). What remains open is listed in `plan.md` "Still open" and marked inline: TLS field names, Oracle backup, the `mssql` crate split, the thick selection rule, the `regex` size question. No [NEEDS CLARIFICATION] marker is kept for a settled point.
