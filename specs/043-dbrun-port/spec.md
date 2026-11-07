# Feature Specification: `dbrun` production port to Rust (`ai-gent db`)

**Feature Branch**: `docs/spec-043-dbrun-port` (specification only; the work lands on its own branches, see `plan.md`)

**Created**: 2026-11-07

**Status**: Draft (for the owner's review; nothing here is approved until the owner says so)

**Input**: eight owner decisions from an owner-run questioning session on 2026-11-07, quoted below as "relayed" (they reached this draft through the supervising agent, not from a file the owner wrote); `specs/038-db-runner-spike/{spec,plan,tasks,results,measurements}.md` (the spike, its drivers in `rust/spikes/dbrun/` and the promoted pieces in `rust/crates/ai-gent-dbrun/`); `specs/039-release-3-contract/` (D26/D27, now amended); `plugins/db/` (`scripts/dbrun.py`, `references/runner.md`, `hooks/guard.sh`, the four skills).

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

(SQLite is counted among the five engines in decision 2 as the relayed list gives it; MySQL and MariaDB share one driver, so "five engines" are five drivers: SQLite, PostgreSQL, MySQL/MariaDB, SQL Server, Oracle.)

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

**Independent Test**: for each engine, a server with a certificate signed by a test CA: connect succeeds with the CA file, fails without it with a redacted error naming the TLS problem, and succeeds with the explicit unverified opt-in only after a logged warning.

### User Story 4 - Backup and restore without the Python helper (Priority: P1)

`apply` takes a backup of a proven-local database before writing and `restore` brings it back, for every engine that has a backup method today (SQLite, PostgreSQL, MySQL/MariaDB, SQL Server), implemented natively (what "natively" means is FR-018).

**Independent Test**: the engines script's plan, apply, restore journey passes on each engine and the table created by the plan is gone after the restore.

### User Story 5 - Oracle works on thin by default and on thick when the user opts in (Priority: P2)

A person connects to Oracle with no client libraries. An account whose password verifier the thin driver cannot use gets a working path or a clear message, and, if the person installed Instant Client and set the path variable, the same request works through thick mode.

**Independent Test**: the engines script on `gvenzl/oracle-free:23-slim` passes on thin; an old-verifier account passes by the means chosen in T062; with a real Instant Client present, thick passes the same journeys; with none, the error says what to set.

### User Story 6 - One breaking release (Priority: P1)

The owner merges `dev` into `main` once: 3.0.0 carries the 035 and 036 cutovers, the `dbrun` cutover and everything in spec 039, and `scripts/release.py` plans the major.

**Independent Test**: spec 039's amended G30 gates are green and `python3 scripts/release.py --dry-run --json` prints `"version": "3.0.0"`.

## Requirements *(mandatory)*

### Command and contract

- **FR-001**: The command is `ai-gent db <subcommand>` with the subcommands, flags, defaults and exit codes of `dbrun.py` (`profiles`, `classify`, `test`, `query`, `plan`, `apply`, `restore`, `script`, `session`, `secrets migrate`, `log`; exit codes 0, 1, 2, 3 policy refusal, 4 connection failed). [NEEDS CLARIFICATION: the command word. 038 open question 2 recommended `ai-gent db` and D9 says "later `db`"; the relayed decisions do not restate it. The guard patterns, fixtures and skills all change with it in the cutover.]
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
- **FR-012**: The count a plan verifies is the server's own count for that statement and is not visible to the next statement. SQL Server must not send a count probe between a plan's statements (the `@@ROWCOUNT` finding): read the count from the DONE token, or send each plan's statements as one batch. [NEEDS CLARIFICATION: `tiberius-ng` exposes DONE counts only through its RPC path (which loses the column names of an empty result) and not at all through its batch path. Whether the answer is an upstream patch, a fork, the one-batch approach or an RPC-path read is for the SQL Server worker to prove and the owner to accept.]
- **FR-013**: The command log entry is written before a statement runs and again with the result; plans carry the tamper check; `apply` backs up before writing; `script` never executes (038 must-not-regress 6).
- **FR-014**: A panic inside a connector never aborts the process silently. The release profile is `panic = "abort"`, under which a panic cannot be caught; the Oracle thin login, which indexes server fields with `unwrap`, was wrapped in a `catch_unwind` that does nothing under abort (038 results E6 review). [NEEDS CLARIFICATION: choose one. (a) Run each networked connector in a child process of the same binary (`ai-gent db _exec`: request on stdin, one JSON line out, a crash becomes a `connect` error); it mirrors today's `_exec` contract and also gives uniform deadlines and kills. (b) Build with unwinding (size, and the policy of the other subcommands). (c) Fix or fork `oracledb` so it returns errors. The draft prefers (a) for every engine; the owner decides.]

### TLS

- **FR-015**: Profiles gain TLS fields for every networked engine: a mode (verify against the system roots; verify against a CA file; unverified), a CA file path, and a server-name override for engines whose client supports one. The Python runner's behaviour (PostgreSQL `sslmode=prefer` through libpq, no explicit Oracle TLS, pymssql without encryption) is a baseline to improve on, not to copy. [NEEDS CLARIFICATION: field names and file keys; whether the default for a remote target is `verify` with a hard failure (a behaviour change for profiles that worked unencrypted); the default for a proven-local container with a self-signed certificate; and whether `unverified` is refused for a profile classed production, which is this draft's proposal.] Unverified mode is allowed only as an explicit profile value and is logged on every connection.
- **FR-016**: TLS is rustls with the system root store (`rustls-native-certs`, already enabled in the spike). No OpenSSL, no `native-tls`. [NEEDS CLARIFICATION: `oracledb` ships the `webpki-roots` bundle and supports a PEM wallet only; whether "system roots" can be honoured for Oracle without patching it.]
- **FR-017**: Each engine has a test against a server whose certificate is signed by a throwaway CA generated in the test: verified with the CA file, rejected without it, unverified opt-in. The spike never ran a TLS handshake on PostgreSQL, MySQL or Oracle and ran SQL Server only against a self-signed image (038 "Not verified").

### Backup and restore

- **FR-018**: Backup before `apply` and `restore` are implemented without Python for SQLite (the backup API or a file copy under a lock), PostgreSQL, MySQL/MariaDB and SQL Server. [NEEDS CLARIFICATION: what "native" means for the owner. Today the dumps are `pg_dump` and `mysqldump` run inside the database container, and `BACKUP DATABASE` goes through a separate autocommit script. Options: (a) Rust orchestrates the same container-exec commands (needs a container engine, which local databases already need), (b) a logical dump and reload written in Rust over the driver (no external tool, but a correctness risk for types, routines, sequences and grants), (c) a mix per engine. Also whether Oracle gets a backup (Data Pump) or `apply` stays refused on Oracle, as today.]
- **FR-019**: A restore recreates the whole database as today, so objects a plan created disappear; system databases are refused for plan and apply (SQL Server `master`, `msdb`, ...; MySQL `mysql`, `sys`, ...). `BACKUP DATABASE` and `RESTORE DATABASE` on SQL Server need autocommit, so the SQL Server connector offers an autocommit path (the spike's adapter always ran a transaction).

### Engines

- **FR-020**: When the last engine is native, the process connector, `dbrun.py`, `test_dbrun.py` (replaced by Rust tests and the parity corpus), `plugins/db/scripts/passstore.py` (replaced by 037's pass-store module) and the `uv` and client-image machinery (`CLIENT_IMAGE`, `uv run --with`, driver pins) are deleted in the cutover. [NEEDS CLARIFICATION: how a native binary on the host reaches a local database container that publishes no port. `dbrun.py` joins the container's network namespace with a client container; the spike always connected over TCP to a published port (`127.0.0.1:154xx`) set by the harness. Options: require published ports for proven-local containers, resolve the container's address through the container engine, or run the binary inside the namespace (an image carrying the binary, which brings a client image back). This decides whether any client image remains and what `session` means; `session` may become unnecessary.]
- **FR-021**: Drivers from the spike: SQLite `rusqlite` bundled; PostgreSQL `postgres` 0.19.14; MySQL and MariaDB the `mysql` crate (one client; the timeout variable is chosen after `SELECT VERSION()`: `max_execution_time` for MySQL, `max_statement_time` for MariaDB); SQL Server `tiberius-ng` 0.13.1 over rustls; Oracle thin `oracledb` `=26.0.0-beta.5` (pure Rust, beta, Oracle's own repository); Oracle thick `oracle` 0.6.3 (ODPI-C). Versions are pinned exactly and bumped only after the engines script passes, as the Python pins were. [NEEDS CLARIFICATION: `tiberius-ng` is a single-maintainer fork on a credential path, and the spike recorded its health from local sources only; plain `tiberius` 0.13.0 answered identically. The owner decides fork, plain or vendoring, and whether `mssql` becomes its own crate (the D28 candidate for the first split). Oracle thin is a beta: the owner accepted thin as the default (relayed 4); the risk stays in the plan.]
- **FR-022**: Value rendering matches the Python driver where it is observable and every deliberate difference is on the FR-005 list. Items from the spike reviews: MySQL fractional datetimes padded to six digits as `isoformat`, TIME as `str(timedelta)`; SQL Server `datetimeoffset` as wall time with its offset, `money` as a four-decimal string, `sql_variant` typed (pymssql returns raw bytes); the DDL row count (`0` in the spike where psycopg and pymssql report `-1`); Oracle `NUMBER` typed from its text (an integer-valued `NUMBER(10,2)` prints `5` where python-oracledb probably gives `5.0`, and `1E-130` stays a string). [NEEDS CLARIFICATION: for the DDL count and for `NUMBER`, keep the Python value or adopt the server's and amend FR-005.]
- **FR-023**: Error text carries the engine's error number where the engine gives one (SQL Server `MssqlError <number> (state, class)`, PostgreSQL through `as_db_error()`, the MySQL error code, Oracle `ORA-`), redacted per FR-008; skills that match error text are updated in the cutover.
- **FR-024**: The Oracle thin driver has no total login time bound (the transport timeout covers the TCP connect only) and sleeps one second after every failed connect unless the connect descriptor carries `RETRY_DELAY=0` (an Easy Connect `retry_delay=0` is ignored). The connector always sends a descriptor and puts the login under its own deadline (the child process of FR-014 provides one).

### Oracle

- **FR-025**: The thin driver cannot log in to an account whose only verifier is the 11G one (`ORA-01017` for an 11G-only account that python-oracledb thin and sqlplus accept; the crate implements only the 12c PBKDF2 path), and the 10G case (DPY-3015 in Python) was never reproduced. Closing it is part of 3.0.0 (relayed 3). [NEEDS CLARIFICATION: the means. (a) implement the 11G (SHA-1) and 10G verifiers in our connector or upstream in `oracledb`; (b) route such accounts to thick mode when Instant Client is present and fail with a clear message when it is not; (c) both. Testing needs an Oracle 11g or 12.1 server image to reproduce a 10G verifier, because the 26ai image refuses to create one (`ORA-02153`); the owner decides whether and which image may be pulled for a test.]
- **FR-026**: Thick mode is a Cargo feature (`oracle-thick`) over the `oracle` crate. ODPI-C loads the client library at run time, so a build needs only a C compiler and the package ships no Oracle library, ever. The library is found through a path environment variable (today's Python name is `AI_GENT_ORACLE_CLIENT_LIB`). A missing library is a `connect`-stage error that names the variable and says to install Instant Client; never a panic or a raw DPI-1047 dump. [NEEDS CLARIFICATION: keep that variable name; whether thick is compiled into every release target or only some (a C compiler on the Windows and musl legs was not tested; the spike's `oracle` probe built to 957 KB on Linux); and the selection rule: thin unless the variable is set, or also automatic when thin fails with the verifier error.]
- **FR-027**: Thick mode cannot be tested without a real Instant Client, which an agent cannot download in its sandbox (the spike's probe with the server home's libraries segfaulted, so those do not stand in for it). The thick test is an owner-run check recorded as gate evidence, not a CI job; CI covers the build and the missing-library path.
- **FR-028**: The thin driver's other recorded gaps stay recorded in `plugins/db/references/engines/oracle.md`: SID connections through to a login, RAC and DRCP, wallets other than PEM, Autonomous Database, PL/SQL OUT binds and DML `RETURNING` (Python fails the same way with DPY-4010), LOB contents, `TIMESTAMP WITH LOCAL TIME ZONE`, servers older than 26ai.

### Package, platforms, release

- **FR-029**: `ai-gent-dbrun` features: `sqlite`, `postgres`, `mysql`, `mssql`, `oracle` (thin), `oracle-thick`. The binary's default features per target come from the platform matrix. In the spike musl failed for sqlite, mysql and mssql (no musl C compiler), and aarch64, Windows and macOS were not run. A target that cannot build an engine is a release blocker or an owner-signed waiver, never a silent omission. [NEEDS CLARIFICATION: the target list and the waiver policy; this is the platform-probe work the owner reserved (relayed 8).]
- **FR-030**: Size budget: the spike's all-drivers figure is about 5.6 MB on Linux x86_64 gnu. [NEEDS CLARIFICATION: the budget the owner accepts, and whether the crate's own full-feature `regex` (needed for Unicode `\w`) is acceptable once the binary depends on the crate.]
- **FR-031**: Skills, references, the db guard (`plugins/db/hooks/guard.sh`, `opencode/guard/rules.mjs` while it exists, the Rust guard evaluator of 033), `shared/guard-fixtures/{db,secrets,containers}.json`, `scripts/check.sh`, the engines script and its CI job, `README.md`, `AGENTS.md` and the docs site switch to the new command in one cutover pull request together with the deletion of FR-020; `ai-gent doctor` drops the db `uv` check.
- **FR-032**: Spec 039 is amended (listed in `plan.md`, "Amendments to 039", and done in the same change set) so that 3.0.0 includes this port; 2.28 stays ahead of it (relayed 7).
- **FR-033**: Every task's output is reviewed by a different agent from the one that wrote it, and the review is recorded in the task's evidence line (reviewer, findings, fixed or recorded). Landing is by pull request into `dev`, one at a time, with the owner merging.
- **FR-034**: `ai-gent-dbrun` publishes after `ai-gent-core` and before `ai-gent` (D28). Trusted Publishing is configured by the owner; an agent never handles a token (038 T024).

### Key entities

Profile (engine, connection facts, TLS fields, secret reference), request (the `_exec` JSON), result (one JSON line), plan record, backup, command-log entry, parity corpus, accepted-differences list.

## Edge cases

- A profile written for the Python runner has no TLS fields: the default for existing profiles is open (FR-015); the migration guide says what changes.
- A password that is all digits, or contains a quote or a newline: redaction and descriptor validation cover it (FR-008, FR-009).
- A server that refuses encryption (an older SQL Server, a TLS-less development database): the connection fails with a redacted error that names the profile field to set; the Python runner used to connect to SQL Server unencrypted.
- MySQL's `max_execution_time` covers `SELECT` only; a write running past the timeout is bounded by the socket timeout plus `lock_wait_timeout`, as in Python. The spike did not exercise it; a test is added.
- SQL Server `SET NOCOUNT ON` issued by a user statement, several result sets in one request, `hierarchyid`, spatial and CLR types: untested in the spike; tests are added or the limit is recorded.
- MySQL 5.7, Percona, MariaDB older than 11.4, Azure SQL, SQL Server 2019 and older, named instances, Windows and Azure AD authentication, Oracle RAC: outside the acceptance set (038 FR-007); each is listed as unsupported-until-tested in the engine references.
- Unix-socket connections: the spike's adapter used TCP only; what `dbrun.py` does for sockets is checked in T010 and kept or dropped on the differences list.
- A second `apply` on the same plan: the plan record's state prevents it, as in Python; a test pins it.
- Windows: the namespace join of `network_args` has Linux semantics; the answer to FR-020 decides what Windows does for proven-local containers.
- The weekly engines job pulls about 5.5 GB of images: one engine at a time, the disk floor applies, containers go by ID or label and never a prune (`shared/containers.md` 9a).

## Success Criteria *(mandatory)*

- **SC-001**: `scripts/test-dbrun-engines.py --runner <ai-gent db>` passes every check for sqlite, postgresql, mysql, mariadb, sqlserver and oracle (thin), on a machine with no `uv` and no Python on `PATH` for the binary under test.
- **SC-002**: The parity corpus is green with only listed differences; each of the 41 `test_dbrun.py` tests maps to a Rust test or an obsolescence note in `differences.md`.
- **SC-003**: Per engine, the TLS tests (FR-017) pass against a throwaway CA; `trust_cert` and `NoTls` appear in no production code path (grep).
- **SC-004**: Plan, apply and restore pass on every engine that has a backup, without Python; the cutover grep finds no `pg_dump` or `mysqldump` shell-out unless FR-018 chose container-exec.
- **SC-005**: `grep -rEn 'dbrun\.py|uv run|CLIENT_IMAGE' plugins shared scripts opencode README.md AGENTS.md docs/site/src` finds only the migration guide, the changelog and specs.
- **SC-006**: The release package carries `ai-gent db` for every target in the platform matrix; `scripts/package.py --check` and `scripts/test-package.sh` are green; size within FR-030.
- **SC-007**: The owner-run checks have evidence: thick Oracle with a real Instant Client, the old-verifier account, and any platform CI cannot build.
- **SC-008**: Spec 039's amended G30 gates are green and the dry run prints 3.0.0.

## Out of scope

- New engines, a TUI, a connection pool, async execution, a general SQL validator (the crate README says so).
- Shipping any Oracle client library.
- Windows authentication, Azure AD tokens, SQL Server named instances, Autonomous Database wallets beyond PEM.
- The owner-prepared items of relayed decision 8.
- Changing the db skills' behaviour beyond the command word and the TLS fields; changing `shared/secrets.md`.

## Open points

Every [NEEDS CLARIFICATION] above is collected in `plan.md` "Open points", each with the task it blocks.
