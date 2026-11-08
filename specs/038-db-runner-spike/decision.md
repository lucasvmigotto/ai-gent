# Decision: `dbrun` and Rust

Status: DRAFT, awaiting the owner's approval

Drafted 2026-11-07 from the repository evidence on `origin/dev` (`results.md`, `measurements.md`, `engine-matrix.md`, `tasks.md`), the spec 043 draft (branch `docs/spec-043-dbrun-port`) and the owner's chat decisions of 2026-11-07 as relayed to the drafting agent. Nothing here is approved until the owner says so. Where the repository does not hold a fact it says "not found" or carries [NEEDS CLARIFICATION]. The spike's own evidence lines are the builders' runs: `results.md` itself says there is no raw log in the repository for the Oracle figures.

- **Date / author / machines used**: 2026-11-07; drafted by an agent for the owner; the spike's experiments ran on one machine: Linux x86_64 (Debian, glibc, Intel i7-1165G7, 8 threads), rustc 1.98.1, gcc 14.2.0, Podman 5.4.2 (`results.md` "E8" and each engine section). The research report `docs/product/dbrun-spike-report.md` (2026-11-01) was a second machine, in a sandbox.
- **Option chosen**: **N (native all)**, proposed, with the phasing of H-exec during the port: the process connector (`dbrun.py _exec`) keeps an engine working until its native connector passes, and is deleted in the cutover. Why N and not H-exec, C, S or "not yet": see "Verdict and why" below.
- **Per engine**: postgresql: native (`postgres` 0.19.14); mysql: native (`mysql` 28.0.4); mariadb: native (same binary); sqlserver: native (`tiberius-ng` 0.13.1, fork-dependent, owner decision pending on fork versus plain, see below); oracle: native thin (`oracledb` `=26.0.0-beta.5`, a beta), thick optional (`oracle` 0.6.3, never run to a result); sqlite: native (`rusqlite`, bundled).
- **Evidence**: E1 to E8 below, linked to `results.md`.
- **User-visible requirement after the change** (one sentence): the db plugin needs nothing but the `ai-gent` binary (and a container engine only for a database that runs in a local container), with no `uv` and no Python.
- **Command word**: `ai-gent db`.
- **Release**: ~~3.0.0, held until the port is done (the owner's decision, relayed)~~ amended 2026-11-07 per owner decision (chat): 3.0.0 not held for dbrun: 3.0.0 ships with `dbrun.py` still Python (spec 039 D26/D27); the port ships in the 3.x line, when spec 043's own gates pass, as the last 3.x step. The owner decides; this draft does not set a date.
- **Not run** (carried over): see "Not run".
- **Crate layout** (D28): confirmed as written in `spec.md` "Crate layout" (library `ai-gent-dbrun` with features `sqlite`, `postgres`, `mysql`, `mssql`, `oracle`; `ai-gent-core` depends on no driver). The crate exists (`rust/crates/ai-gent-dbrun/`, T015 and T025 done: `cargo publish --dry-run` packaged and verified 12 files). The `ai-gent-dbrun` Trusted Publishing step (T024) is the owner's and is pending (not found as done). Whether `mssql` becomes its own crate and whether `ai-gent-dbrun` depends on `ai-gent-core` (spec.md open question 6): [NEEDS CLARIFICATION: not decided in the repository or in the relayed chat].
- **Must-not-regress checklist**: see "Must-not-regress checklist".
- **If the answer is "not yet"**: see that section, kept as the fallback.
- **Owner sign-off**: pending (amended 2026-11-07 per owner decision (chat): 3.0.0 not held for dbrun: the approval covers a verdict that ships after 3.0.0; status stays DRAFT until the owner approves).

## Verdict and why

Proposed verdict: **N**, native drivers for all five networked engines plus SQLite, ~~ahead of 3.0.0, which matches the owner's relayed decision~~ in the 3.x line, after 3.0.0 (amended 2026-11-07 per owner decision (chat): 3.0.0 not held for dbrun). The evidence supports it with these conditions, each of which the spike left open and spec 043 turns into work:

1. Four engines ran the engines script end to end through the Rust adapter: PostgreSQL 17 (17 of 17 checks), MySQL 8.4 (17 of 17), MariaDB 11.4 (17 of 17), SQL Server 2022 (17 of 17, three runs), and Oracle 26ai Free (14 of 14, two runs). SQLite passed the 8 `SqliteEndToEndTest` journeys natively. The safety core ported with zero diffs against goldens generated from the Python (E1).
2. The spike used no verified TLS on any engine (PostgreSQL and MySQL none; SQL Server `trust_cert()`; Oracle never handshaken). The verdict is conditional on closing that before the port's cutover (amended 2026-11-07 per owner decision (chat): 3.0.0 not held for dbrun; was "before 3.0.0") (owner decision on TLS below).
3. Oracle thin is narrower than python-oracledb thin (no 11G or 10G verifier path) and is a beta. The verdict is conditional on the owner's chosen answer (message pointing at thick mode, below) and on thick mode being built as an optional feature.
4. SQL Server needs a fork-dependent crate and the count probe fixed (one batch per plan, below).

Why not the others: S leaves the premise ("a user installs once and needs neither uv nor Python") unmet for the db plugin, which `spec.md` names as the one place it fails. C and H need a client container per request; E5 and E6 show native drivers work for SQL Server and Oracle thin without one. H-exec is kept only as the order of work, not as the end state. "Not yet" is kept as a fallback section.

## Evidence per engine

Numbers are copied from `results.md` and `measurements.md`; "not found" means the repository has no figure.

### E1: safety core port (all engines)

- Pass (T001). The report's figure: about 330 lines of Rust, zero diffs on 1026 classify verdicts, 1026 splits, 1026 `is_ddl`, 4104 cost cases, mask, redact, network (reads only; goldens from the Python). In the repo: the standalone `cargo test --offline` passes 4 golden tests (16 Python source tests, exact reasons) and the Python unittest passes 51 runs; the two lookaround workarounds (Oracle `DBMS_` exclusion, redaction boundaries) are recorded in the spike README. The goldens cover reads only; writes, plan records, log entries and CLI shapes are not in the corpus (spec 043 FR-004 adds them).
- `sqlparser` 0.63 as classifier: 163 of 1026 disagree, 119 say READ where dbrun says WRITE. Rejected (FR-010).

### SQLite (E4), native

- Pass (T004). 8 `SqliteEndToEndTest` journeys natively, plus independent `mode=ro` and `query_only` refusal checks (server-side refusal shown). Via the adapter the engines script passes 8 journeys.
- Size: + sqlite 2.65 MB on `dbq` (the report said 2.63); cold build 26.8 s, warm 6.1 s; needs a C compiler (bundled SQLite). Cold start median 0.7 ms (refuse path), `SELECT 1` 6.3 ms median, 7.1 ms p95, against warm `uv run --with PyMySQL` 46.2 ms on the same machine.
- Cold start of the report's own machine for sqlite: 48 to 65 ms (a different machine and method; not comparable).

### PostgreSQL (E2), native

- Pass (T005): 17 of 17 checks, 29 s wall (25 s on a second run), plan, apply, restore and the remote-declared checks included. Server-side refusal `25006`; statement timeout refused at 1.04 s; lock timeout (5 s) refused at 5.05 s; redacted `28P01`.
- Adapter release size: 388,984 bytes bridge only, 957,984 bytes with `pg` (+569,000 bytes, +0.57 MB); `dbq` + postgres 2.09 MB, cold build 24.6 s. One request about 0.04 s wall. Runtime libraries: `libgcc_s`, `libc`.
- Python driver on the same checks: not run (a driver fetch needs the network).
- TLS: none (`NoTls`). The Python/libpq default is `sslmode=prefer`, so against a TLS-offering server the prototype sends credentials and data unencrypted where psycopg would encrypt, and it cannot connect to `hostssl`-only servers. Not run.

### MySQL and MariaDB (E3), native, one binary

- Pass (T006): MySQL 8.4.13 17 of 17, 32 s; MariaDB 11.4.13 17 of 17, 40 s. Server-side refusal ERROR 1792 (25006) on both. Timeouts: MySQL `max_execution_time` ERROR 3024 after 1.006 s; MariaDB `max_statement_time` ERROR 1969 after 1.007 s.
- Adapter release size: 388,824 bytes bridge only, 2,044,048 bytes with `mysql` (+1,655,224 bytes, +1.66 MB). It links the system `libz.so.1`; the binary does not start on a host without it. One request against MariaDB 11.4: median 3.8 ms, p95 4.1 ms. `dbq` + mysql 1.98 MB.
- First connect to MySQL 8.4 (debug build, one run) 0.156 s. The release binary was not timed against MySQL (the container was gone).
- TLS: none (`minimal` features). Not run.

### SQL Server (E5), native via `tiberius-ng`

- Pass (T007): 17 of 17 in three runs; 70 s, 35 s, 42 s. `BACKUP`/`RESTORE` go through a separate autocommit path, so those two checks do not exercise the Rust driver.
- Adapter release size: 387,600 bytes bridge only, 2,599,528 bytes with `mssql` (+2,211,928 bytes, +2.21 MB); `dbq` + mssql 3.72 MB, cold build 38.1 s. Needs `aws-lc-sys` (C compiler, cmake crate in the tree). One request: median 65.4 ms, p95 104.2 ms (Python `pymssql`: median 104 ms). Timeout refused at 1.04 s; lock timeout at 5.05 s; 100,000-row read with `max_rows: 5` 0.43 s.
- No server-side read-only mode on SQL Server: the server ran read-mode writes (`UPDATE`, `INSERT`, `DELETE`, `CREATE TABLE`, `DROP TABLE` returned `ok: true`) and only the rollback undid them. The classifier plus a never-committed transaction are the protection. The Python runner behaved the same.
- Fork health (local sources only): `tiberius-ng` 0.13.1 released 2026-08-30 (0.13.0 on 2026-08-29), MIT OR Apache-2.0, rust-version 1.88, one active maintainer, a fork of `prisma/tiberius` whose README says it is no longer maintained; its changelog says "No further releases are planned in the immediate term"; `src/` differs from plain `tiberius` 0.13.0 by about 14,500 diff lines. Plain `tiberius` 0.13.0 built from the same source with a one-line dependency change and answered 6 requests identically (size and time not measured; not run through the whole engines script). Not found: GitHub issue and PR counts, last commit date, stars, download counts.
- TLS: rustls with `trust_cert()` for the self-signed test image only; `encrypt_option` TRUE for the adapter, FALSE for pymssql. No CA-signed certificate test, no TDS 8.0 strict encryption. `sqlcmd` rejected on interface, not built.

### Oracle thin (E6b), native via `oracledb`

- Partial (T014): 14 of 14 checks in two runs (58 s and 59 s); the 10G verifier case not reproduced; TLS never handshaken. Server `gvenzl/oracle-free:23-slim` (Oracle 26ai Free), image 1.97 GB.
- Adapter release size 2,780,000 bytes (2.78 MB) with `--features oracle` only; runtime libraries `libgcc_s`, `libc`; release build 43.8 s cold. One request: Rust median 39.8 ms, p95 67.5 ms; Python `uv run --offline --with oracledb` median 337.5 ms, p95 786 ms. 100,000-row read with `max_rows 5`: Rust 63.6 ms, Python 482 ms. Timeout 1 s refused at 2.1 s (Python the same: break plus a second wait).
- Server-side read-only: `ORA-01456` in a read-only transaction. DDL commits implicitly, so the classifier is the only barrier for DDL, as in Python.
- 11G-only account: `sqlplus` and python-oracledb thin log in; the Rust driver gets `ORA-01017`. The crate implements only the 12c PBKDF2 verifier. A 10G-only account: not measured (a panic is plausible).
- TLS: rustls with `webpki-roots`, PEM wallet only, no option found to skip verification; `tcps://` against a plain listener fails in 1.0 s; no handshake run.

### Oracle thick (E6a), `oracle` crate: NOT RUN TO A RESULT (T008 open)

- The crate builds with a C compiler only (probe 957 KB). Without a client library it fails at the first connect with `DPI-1047`. Instant Client was not fetched (no network); the server home's libraries (174 MB, `libclntsh.so.23.1`) made the probe die with SIGSEGV. No login, query or timing was obtained. Only the "library absent" half of the expectation is confirmed.

### E7: Windows (T010): not run

- No Windows machine or runner was used. The target standard libraries are installed on the Linux host but there is no MSVC `link.exe`, clang or mingw. The namespace join of `network_args` has Linux semantics.

### E8: build matrix (T009)

| Driver set | Size (MB) | Cold build | Toolchain need |
|---|---|---|---|
| core | 1.56 | 10.0 s | none |
| + sqlite | 2.65 | 26.8 s | C compiler |
| + postgres | 2.09 | 24.6 s | none (NoTls) |
| + mysql | 1.98 | 20.8 s | C compiler; links system `libz.so.1` |
| + mssql | 3.72 | 38.1 s | aws-lc-rs (C and assembly) |
| all four (sqlite, pg, mysql, mssql) | 5.58 | 63.3 s | as above |

- `ring` is in no set; `aws-lc-rs` comes with mssql. `openssl-sys` and `native-tls` are in no set.
- Other targets: `x86_64-unknown-linux-musl` built for core (1.66 MB) and + postgres (2.22 MB); it failed for sqlite, mysql and mssql (no `x86_64-linux-musl-gcc`). aarch64 Linux, Windows and macOS: not run (no cross C compiler, linker or host).
- Cold start of `dbq`, refuse path: 0.6 to 2.1 ms median across the sets; `SELECT 1` on sqlite 6.3 to 7.1 ms. Baselines on the same machine: `python3 -I -c pass` 7.4 / 8.5 ms; warm `uv run --with PyMySQL` 46.2 / 48.0 ms.
- The size cost is small; the build toolchain (a C compiler per target) is the constraint.

## Review findings each driver needed

Independent reviews found these; every item is in `results.md`. "Fixed" means fixed in the spike; "recorded" means left open and carried into spec 043.

| Finding | Engine | State | Where it goes in 043 |
|---|---|---|---|
| Row count taken from parsing statement text (a commented `UPDATE` reported -1 and a correct change rolled back); `max_rows` not applied in write mode | PostgreSQL | fixed (count from the server's `CommandComplete`; cap in both modes, `truncated` only in read mode) | FR-010 |
| Count probe visible to the next statement: `SELECT ROWCOUNT_BIG()` between statements leaves `@@ROWCOUNT` at 1 | SQL Server | recorded, not fixable in the driver | FR-012, T051 |
| Control batches (session setup, identity, transaction, showplan) had no timeout | SQL Server | fixed (run under the request timeout) | FR-011 |
| Panic on a negative, huge or non-numeric `timeout` | MySQL | fixed (`timeout_secs()`, `connect`-stage error) | FR-011 |
| Fractional datetimes and TIME rendered differently from PyMySQL | MySQL | fixed (`iso_datetime()`, `py_timedelta()`; only the pure conversion tested, not a live fractional column) | FR-022 |
| A numeric `password` or `user` not redacted from error text | Oracle | fixed | FR-008, T004 |
| Connection fields holding descriptor characters could add an address | Oracle | fixed (refused) | FR-009 |
| Login has no total time bound | Oracle | recorded | FR-024 |
| `catch_unwind` around login does nothing under `panic = "abort"`; a panic after login is not caught even in a debug build | Oracle | recorded | FR-014, T012, T064 |
| 1 s sleep after every failed connect; `retry_delay=0` in Easy Connect ignored (works as `(RETRY_DELAY=0)` in a descriptor) | Oracle | worked around (always send a descriptor) | FR-024 |
| `NUMBER` typed from its text (an integer-valued `NUMBER(10,2)` prints `5`; `1E-130` stays a string) | Oracle | recorded | FR-022 |
| `TIMESTAMP WITH TIME ZONE` returned as UTC without the offset (Python gives wall time); `datetimeoffset` the same on SQL Server | Oracle, SQL Server | not fixed | FR-022 |
| DDL row count 0 where psycopg and pymssql report -1 | PostgreSQL, SQL Server | recorded | FR-022 |
| `sql_variant` typed (pymssql gives raw bytes); `money` beyond about 9e11 loses digits | SQL Server | not fixed | FR-022 |
| Descriptor field check (the connect-descriptor validator) | Oracle | see the numeric/field items above; made shared in 043 | FR-009 |

## Not run

Carried over from the spike, each still "not run" unless a line says otherwise. A live Codex path is not part of this decision.

- Oracle 10G and 11G servers: the 10G verifier cannot be created on 26ai (`ORA-02153`), so DPY-3015 was not reproduced. An 11G-only account was reproduced (thin Rust fails).
- TLS handshakes on all engines: none run against a real TLS server (PostgreSQL and MySQL built without TLS; SQL Server only against the self-signed image with `trust_cert()`; Oracle by source reading and a negative probe).
- T008, Oracle thick mode with a real Instant Client: not run to a result.
- Backup and restore through Rust: `pg_dump` and `mysqldump` run inside the container as Python shell-outs; SQL Server `BACKUP`/`RESTORE` stay a Python autocommit shell-out. The engines script's restore checks passed with those parts still Python.
- Windows and macOS builds (E7); aarch64 (Linux); musl for sqlite, mysql and mssql (failed).
- The 5-engine parity corpus: the existing golden is reads only; a corpus with writes, plans, logs and CLI shapes does not exist yet.
- The plan, apply and script commands over all five engines through a Rust CLI: the engines script ran them through the adapter, but there is no Rust CLI yet (`rust/crates/ai-gent-dbrun` holds the core, the trait, the process connector and the SQLite connector with `core::plan`; networked connectors, profiles, the command log and a CLI are not found there).
- Also not run: Unix-socket connections; MySQL 5.7, Percona, MariaDB older than 11.4; SQL Server 2019 and older, Azure SQL, named instances; `sqlcmd` in a client image; the Python psycopg driver on the same PostgreSQL checks; a write past the timeout on MySQL; `SET NOCOUNT ON` set by a user statement on SQL Server; several result sets in one request.

## Must-not-regress checklist

The list is `spec.md` "Must not regress" 1 to 8. The tests that prove each item for the promoted code are not all in place; the owner signs each item when its test exists. Status as found in the repository on 2026-11-07:

| # | Item | Proof now | Open |
|---|---|---|---|
| 1 | Classification (local only when proven) | Python tests `ProfilesTest`, `SqliteEndToEndTest` remote cases; the engines script `published port is not local` and `remote refusals` pass through the adapter on all five engines | Rust profiles and the proof are spec 043 T010 |
| 2 | Statement scanning | E1 golden (4 tests, 1026 verdicts) | writes not in the golden |
| 3 | Cost rules | E1 golden (4104 cost cases) | none found |
| 4 | Masking | E1 golden; engines script `read is masked` and `--reveal shows values` pass on every engine | none found |
| 5 | Redaction | E1 golden; redaction shown on PostgreSQL, MySQL, Oracle; numeric password finding fixed on Oracle | host redaction not provable on loopback; shared redactor with a numeric test is 043 T004 |
| 6 | Command log before a statement, plan tamper check, backups before write, row cap, timeouts, `script` never executing | `SqliteEndToEndTest` 8 journeys in the crate (T018); timeouts shown on each engine | the command log and a native backup are not yet in Rust |
| 7 | Secrets: pass first, credentials only on stdin | not found for the Rust path | 043 T010 |
| 8 | Guards | not touched in the spike (SC-006: no file under `plugins/db/` changed) | change in the cutover with the command word |

## Cutover edits (the verdict is not S)

D7: one release, references switched, `dbrun.py` deleted only in the release that ships the verdict (T026). From `git grep 'dbrun\.py'` (21 lines in 12 files on 2026-11-01), the `plan.md` list, and spec 043 groups H and I:

- Skills and references: `plugins/db/skills/*/SKILL.md`; `plugins/db/references/{runner,safety,connections,secrets,containers}.md` and `engines/{oracle,postgresql,mysql,sqlserver}.md` (TLS fields, SQL Server's missing read-only mode, Oracle thin and thick text, the unsupported-until-tested list); `README.md` (1), `AGENTS.md` (1).
- Guards: `plugins/db/hooks/guard.sh` (1), `plugins/secrets/hooks/guard.sh` (the `dbrun` mention), `opencode/guard/rules.mjs` (lines 147 to 218 while it exists), `opencode/guard/test/rules.test.mjs` (5), `scripts/test-db-guard.sh` (4), `scripts/test-secrets-guard.sh` (1), `shared/guard-ux.ts` and its copies, `shared/guard-fixtures/{db,secrets,containers}.json`.
- Tests and scripts: the `scripts/check.sh` unit-test block (1), `scripts/consistency.py`, `scripts/package.py`, `scripts/test-dbrun-engines.py` (Python runner path removed after the cutover; the weekly job in `scheduled.yml` pointed at the Rust binary).
- Deletions: `plugins/db/scripts/dbrun.py`, `test_dbrun.py`, `passstore.py`, the process connector, `CLIENT_IMAGE` and the driver pins.
- Evals: `plugins/db/evals/` (the `behavior-raw-client` scaffold and any case naming the old command).
- `ai-gent doctor` drops the db `uv` check. The bare word `dbrun` appears in 43 files and 324 lines on 2026-11-01 (prose); the cutover re-runs the grep.
- Release: the cutover is its own release in the 3.x line (amended 2026-11-07 per owner decision (chat): 3.0.0 not held for dbrun; the 035, 036 and 037 cutovers ship with 3.0.0); `CHANGELOG.md` `## Unreleased` and one `BREAKING CHANGE:` footer. ~~Spec 039 is amended (open question 5 there changes to "no"; G30-16 and the README sentence change with it; the roadmap line becomes a 3.0.0 gate).~~ amended 2026-11-07 per owner decision (chat): 3.0.0 not held for dbrun: spec 039 is not changed for this; `uv` stays a prerequisite for the db plugin only until the port ships, and the cutover is a release of its own in the 3.x line.
- Verification: `grep -rEn 'dbrun\.py|uv run|CLIENT_IMAGE' plugins shared scripts opencode README.md AGENTS.md docs/site/src` finds only the migration guide, the changelog and specs; `python3 scripts/package.py --check`; `python3 scripts/release.py --dry-run --json` plans the release that carries the cutover (amended 2026-11-07 per owner decision (chat): 3.0.0 not held for dbrun; was 3.0.0).

## If the answer is "not yet"

Kept as the fallback if the owner withdraws N.

- **What stays**: `plugins/db/scripts/dbrun.py`, `test_dbrun.py`, the `passstore.py` link, the guard patterns, the `check.sh` block, the weekly engines run.
- **What is documented**: the db plugin requires `uv` or Podman/Docker; `ai-gent doctor` reports it; spec 039's README sentence stays ("only the db plugin needs uv and Python").
- **Revisit**: when `oracledb` leaves beta and the TLS, 11G-verifier and count-probe gaps above have results. Date: [NEEDS CLARIFICATION: not set by the owner].

## Decisions given by the owner in chat on 2026-11-07

These are the owner's own words as relayed to the drafting agent, in the form `Decided: <choice> — <reason>`. Reasons are taken from the repository evidence, not invented; a reason the repository does not give is marked.

- **Command word.** Decided: `ai-gent db` — one binary; `spec.md` open question 2 recommended it and D9 says "later `db`"; the guard patterns change in the same release.
- **Backup and restore.** Decided: Rust drives the same engine tools (`pg_dump`, `mysqldump`, the SQL Server `BACKUP`/`RESTORE`) through the container exec — the dumps are already container shell-outs and do not depend on the driver (`spec.md`, `results.md` E2). This is spec 043 FR-018 option (a). Whether Oracle gets a backup: [NEEDS CLARIFICATION].
- **TLS.** Decided: verify by default (system roots or a CA file); loopback-proven profiles may opt out explicitly; the opt-out is refused on production profiles — the spike's `NoTls`, no TLS and `trust_cert()` are unacceptable outside a throwaway image (`results.md` E2, E3, E5).
- **SQL Server.** Decided: keep `tiberius-ng`, and send a plan as one batch (no count probe between statements) — plain `tiberius` answered identically, so the fallback stays one dependency line; the batch avoids the `@@ROWCOUNT` finding that the driver cannot fix. Vendoring and a separate `mssql` crate are not decided.
- **Panic containment.** Decided: a short-lived child process per engine call — `panic = "abort"` cannot be caught, so `catch_unwind` around the Oracle login does nothing (`results.md` E6 review); this is spec 043 FR-014 option (a).
- **Oracle thin.** Decided: thin refuses accounts without a 12c verifier with a message pointing at thick mode; TLS via a PEM wallet or a CA file — the crate implements only the 12c verifier (`results.md` E6b) and supports PEM wallets only.
- **Oracle thick.** Decided: optional, reusing `AI_GENT_ORACLE_CLIENT_LIB`, a separate optional crate never in the package — client libraries cannot be shipped (`results.md` E6a).
- **Value contract.** Decided: Rust values are the contract (the DDL row count is 0; `NUMBER` is typed from its text) — they are what the spike's drivers produce. The differences from Python go on the accepted-differences list (043 FR-005).
- **Profiles with no published port.** Decided: they keep the container exec path — the spike always connected over TCP to a published port; the namespace join of `dbrun.py` stays for the rest (043 FR-020).
- **Targets.** Decided: all six release targets, with a per-target engine waiver — an engine that cannot build on a target is an owner-signed waiver, never a silent omission (043 FR-029). The target list is [NEEDS CLARIFICATION: not found in the repository].
- **Held cutovers.** Decided: they merge to `dev` inert until one cutover PR — one breaking release (amended 2026-11-07 per owner decision (chat): 3.0.0 not held for dbrun: for 035, 036 and 037 that is 3.0.0; the `dbrun` cutover is its own later PR).

Earlier decisions, marked as relayed (they reached the drafting agent through the supervisor, not from a file the owner wrote; spec 043 "Owner decisions on record" lists them):

- **Relayed, reversed 2026-11-07**: ~~3.0.0 is held for the `dbrun` port (reverses D26 and D27 of spec 039)~~ amended 2026-11-07 per owner decision (chat): 3.0.0 not held for dbrun: the owner decided in chat "lets dont hold dbrun for 3.0.0"; D26 and D27 stand.
- **Relayed**: all five networked engines in Rust, SQLite included; no Python left in the db plugin.
- **Relayed**: thick Oracle is optional; thin is the default; client libraries are never shipped.
- **Relayed**: spec 043 is the production port, one worker per engine and per shared-core task, each output reviewed by a different agent, landing by pull request into `dev`.

## Open points

- [NEEDS CLARIFICATION] The sign-off itself: this file is a draft; the owner decides, and T012 records it.
- [NEEDS CLARIFICATION] `tiberius-ng` versus plain `tiberius` versus vendoring (the owner chose `tiberius-ng`; health beyond local sources is not retrieved).
- [NEEDS CLARIFICATION] `ai-gent-dbrun` depends on `ai-gent-core`? (spec.md open question 6); whether `mssql` is the first split.
- [NEEDS CLARIFICATION] Trusted Publishing for `ai-gent-dbrun` (T024), the owner's step.
- [NEEDS CLARIFICATION] The 10G verifier: needs an Oracle 11g or 12.1 image; the owner decides whether it may be pulled.
- [NEEDS CLARIFICATION] The size budget (043 FR-030), the six-target list and waiver terms, and whether Oracle gets a backup.
- [NEEDS CLARIFICATION] E7 (Windows) and aarch64 need the owner's machines or a CI runner.
- The thick-mode check with a real Instant Client is an owner-run item (043 FR-027, T072).
