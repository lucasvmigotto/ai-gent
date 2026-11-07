# Results: 038 dbrun spike

Measured results by experiment. Each section names the machine and the command. Numbers here are from this repository's spike (`rust/spikes/dbrun`), not from the research report.

## E8: build matrix of driver sets

Machine: Linux x86_64 (Debian, glibc, 8 threads, Intel i7-1165G7), rustc 1.98.1 (pinned by `rust/rust-toolchain.toml`), gcc 14.2.0, GNU ld 2.44. No cmake, no nasm, no clang, no pkg-config; perl present. Dependencies were already downloaded (`cargo fetch --locked`), so times exclude the network. Profile is the spike's own: `opt-level = "z"`, `lto = true`, `codegen-units = 1`, `strip = true`, `panic = "abort"`, so the sizes are already stripped. The `dbq` binary only (`--bin dbq`), one fresh `CARGO_TARGET_DIR` per set.

Command: `cargo build --release --locked --bin dbq --features <set>`. The Cargo feature for PostgreSQL is `pg`; the all set is `sqlite,pg,mysql,mssql` (the spike's `all` feature omits mssql).

### Size and build time, x86_64-unknown-linux-gnu

| Driver set | Size (bytes) | Size (MB) | Report's figure (MB) | Cold build | Warm rebuild (main.rs touched) |
|---|---|---|---|---|---|
| core only | 1563720 | 1.56 | 1.56 | 10.0 s | 5.8 s |
| + sqlite | 2651936 | 2.65 | 2.63 | 26.8 s | 6.1 s |
| + postgres | 2090792 | 2.09 | 2.09 | 24.6 s | 8.7 s |
| + mysql | 1979088 | 1.98 | 1.98 | 20.8 s | 8.0 s |
| + mssql | 3723304 | 3.72 | 3.72 | 38.1 s | 11.5 s |
| all (sqlite, pg, mysql, mssql) | 5582448 | 5.58 | 5-6 (estimate) | 63.3 s | 15.8 s |

Stripped size equals the size above because the profile sets `strip = true`. The "all drivers together" estimate of 5-6 MB is now measured: 5.58 MB. Sizes reproduce the report within 0.02 MB.

### Toolchain needs actually observed

| Driver set | TLS / crypto backend | C compiler | Other build tools | Runtime shared libraries (`ldd`) |
|---|---|---|---|---|
| core | none | none | none | libc, libgcc_s |
| + sqlite | none | yes: `libsqlite3-sys` (bundled SQLite) builds C through the `cc` crate; gcc 14.2 sufficed | none | libc, libgcc_s, libm |
| + postgres | none (the spike connects without TLS, `NoTls`) | none | none | libc, libgcc_s |
| + mysql | none (`minimal` feature, no TLS) | the `cc` crate runs for `libz-sys`, but it links the system zlib (`cargo:rustc-link-lib=z`) | none observed | adds `libz.so.1` (system zlib): the binary does not start on a host without it |
| + mssql | rustls with aws-lc-rs (`tiberius-ng` -> `tokio-rustls` -> `rustls` 0.23.46 -> `aws-lc-rs` 1.19.0 -> `aws-lc-sys` 0.47.0) | yes: `aws-lc-sys` compiles its C and assembly (`.o` files in the build dir) with gcc | the `cmake` crate is in the tree as a build dependency of `aws-lc-sys`, but the cmake binary is not installed and was not needed on this target; perl is present | libc, libgcc_s |
| all | same as mssql | sqlite plus mssql | none | libc, libgcc_s, libm, libz.so.1 |

`ring` is in the dependency graph of no set (`cargo tree --locked --features <set> -e normal,build -i ring` prints "nothing to print" for mssql and all, and "did not match any packages" for the rest). `aws-lc-rs` appears only through mssql (`tokio-rustls` and `rustls-webpki`). `openssl-sys` and `native-tls` appear in no set, so no set needs a system OpenSSL. A rustls build with the `ring` feature instead of the default `aws-lc-rs` was not tried.

### Cold start of `dbq`

Method: 55 runs of one process spawned from a Python loop (`subprocess.run`, first 5 dropped), median and p95 of the remaining 50, wall clock, same machine, files in page cache. Two paths: the refuse path (a write statement is classified and refused, nothing else runs, no driver touched) isolates process start and binary size; the sqlite path runs `SELECT 1` on a small database made by `mkdb.py`.

| Binary | Path | Median | p95 |
|---|---|---|---|
| core | refuse | 0.6 ms | 0.9 ms |
| + sqlite | refuse | 0.7 ms | 0.8 ms |
| + postgres | refuse | 0.6 ms | 0.8 ms |
| + mysql | refuse | 1.4 ms | 1.9 ms |
| + mssql | refuse | 1.8 ms | 2.1 ms |
| all | refuse | 2.1 ms | 2.4 ms |
| + sqlite | `SELECT 1` on sqlite | 6.3 ms | 7.1 ms |
| all | `SELECT 1` on sqlite | 7.1 ms | 7.8 ms |
| baseline: `python3 -I -c pass` (this machine) | | 7.4 ms | 8.5 ms |
| baseline: warm `uv run --with PyMySQL python -c "import pymysql"` (this machine) | | 46.2 ms | 48.0 ms |

The spec's baseline (stdlib start 24 ms, warm uv 176 ms) was taken on another machine; on this one the same two measure 7.4 ms and 46.2 ms, so compare ratios, not absolutes. Against the warm-uv baseline, `dbq` is about 7 times faster on a real sqlite read (6.3 ms versus 46.2 ms) and 20 times or more on the refuse path, on this machine; it is far below the recorded 176 ms too. These are start times with no network round trip: connecting to PostgreSQL, MySQL and SQL Server adds the engine's connect time (E2, E3, E5: 88, 160, 69 ms).

### Other targets

| Target | Result |
|---|---|
| x86_64-unknown-linux-musl, core | built, 1655600 bytes (1.66 MB), static-pie, 9.7 s cold; refuse-path start median 1.0 ms, p95 1.2 ms. Linked with the host gcc, no musl C compiler needed. |
| x86_64-unknown-linux-musl, + postgres | built, 2216896 bytes (2.22 MB), 16.2 s cold; the refusal path runs. |
| x86_64-unknown-linux-musl, + sqlite | failed: `libsqlite3-sys` build script, `failed to find tool "x86_64-linux-musl-gcc"`. Needs a musl C cross compiler (`musl-tools`), not installed. |
| x86_64-unknown-linux-musl, + mysql | failed: `libz-sys` build script, same missing `x86_64-linux-musl-gcc`. |
| x86_64-unknown-linux-musl, + mssql | failed: `aws-lc-sys` build script, same missing `x86_64-linux-musl-gcc`. The all set is therefore not buildable for musl here. |
| aarch64 (Linux gnu or musl) | not run: no aarch64 cross C compiler or linker installed (no `aarch64-linux-gnu-gcc`, no `aarch64-linux-musl-gcc`) and no aarch64 machine. The `aarch64-unknown-linux-musl` standard library is installed, but the build scripts of sqlite, mysql and mssql need a C compiler for it and linking needs a cross linker. No numbers recorded. |
| aarch64 cold start | not run: no aarch64 host. |
| Windows (x86_64-pc-windows-msvc, aarch64-pc-windows-msvc) | not run: the target standard libraries are installed, but there is no MSVC `link.exe`, no clang and no mingw on this Linux host. Left to E7 / T010. |
| macOS targets | not run: the standard libraries are installed, but there is no Apple SDK or cross linker. Outside the task's scope. |

Only the x86_64 musl target was run: `rustup target list --installed` shows gnu and musl for the default toolchain, and the pinned toolchain carries standard libraries for the others without the tools to link them.

### Reading for the decision

- The five single-engine sizes match the report, and all together is 5.58 MB.
- The C toolchain is the real cost, not Rust compile time: bundled SQLite and aws-lc-sys need a C compiler per target, the mysql set needs the system zlib, and cross builds need a cross C compiler. The pure-Rust sets (core, postgres without TLS) cross-compile with no extra tools.
- The mysql build is dynamically linked to `libz.so.1`, a runtime dependency the report did not mention; whether `libz-sys` can build zlib from source (its `static` feature) was not tried.
- PostgreSQL TLS and MySQL TLS are not run, so those two sets are without TLS; adding TLS may pull `rustls` and so `aws-lc-rs` or `ring` into them.

## E2: PostgreSQL prototype through the adapter (T005, 2026-11-07)

Machine as E8. Server: `docker.io/library/postgres:17` (PostgreSQL 17.11, already pulled; 512 MB cap, published on `127.0.0.1:15433`), started by `scripts/test-dbrun-engines.py` through podman. This host has no compose provider, so a stand-in `docker-compose` shim (`up -d` and `down -v` over `podman run` and `podman rm -f -v`, written by another worker for the same script, with the compose labels the profile discovery reads) was put first on `PATH` for the run. The script itself is unchanged.

### Choice

`postgres` 0.19.14 (sync API over `tokio-postgres` 0.7.18, pure Rust, no C compiler), already in the spike's `Cargo.toml` as the `pg` feature. Reasons: the adapter is one request per process, so a blocking API is the smaller code and needs no async runtime of our own (the crate runs a private Tokio runtime inside); `Error::as_db_error()` is available; session options go through `Config::options` exactly as libpq's `options`. Rejected: `sqlx` (async, a larger tree and feature surface for four calls); `tokio-postgres` directly (the same wire code, plus an async runtime and `block_on` in the adapter).

TLS: **none in the prototype** (`NoTls`). This is not what the Python does: psycopg/libpq default to `sslmode=prefer` (TLS if the server offers it, plain otherwise; `PGSSLMODE` can change it, and `dbrun.py` passes no `sslmode` of its own). With `NoTls` the client's default `SslMode::Prefer` can only fall back to plain text, so against a server that offers TLS the prototype sends credentials and data unencrypted where psycopg would encrypt them, and against `hostssl`-only or `sslmode=require` servers it fails to connect. The request JSON carries no `sslmode` field, so there is nothing to honour yet. The production decision (not made here): `tokio-postgres-rustls` 0.14.0 (third party, `engine-matrix.md`), an `sslmode` honoured from the profile, and a test against a TLS-enabled image. The local test image has no TLS, so rustls versus none is not distinguishable by this run; none is acceptable only for loopback. The E8 note stands: rustls brings `aws-lc-rs` or `ring` (a C compiler) into the PostgreSQL set.

### What was built

`rust/spikes/dbrun/src/drivers/pg.rs` (about 250 lines) and a `#[cfg(feature = "pg")]` branch in `src/bin/adapter.rs`: `postgresql` requests run natively, every other engine still goes to the Python `_exec`. Same session options as `dbrun.py connect()`: `statement_timeout` = request timeout, `lock_timeout=5000`, `idle_in_transaction_session_timeout=60000`, `default_transaction_read_only=on` in read mode, `application_name=ai-gent-dbrun`, `connect_timeout` 10 s. Read mode additionally opens `BEGIN READ ONLY` (Python relies on the session default alone), so `SET default_transaction_read_only=off` inside a request cannot lift it. Values come back through the simple query protocol as text and are converted by the column type (from a `prepare`): ints, floats and booleans as JSON, bytea as `<N bytes>`, timestamps as Python's `isoformat()`, everything else (numeric, uuid, json, dates, intervals) as the server's text. Errors: a server error is `DbError <SQLSTATE>: <message>` taken from `as_db_error()` (never `DETAIL`, which quotes row values); other failures are the error with its source chain; every message then goes through `core::redact` for password, host and user, as the contract does.

### Run with the adapter

Commands: `cd rust/spikes/dbrun && cargo build --features pg --bin adapter`, then from the repo root `python3 scripts/test-dbrun-engines.py --runner rust/spikes/dbrun/target/debug/adapter --engine postgresql --port 15433` (port 15433 because another worker used the default 15432 at the same time).

Result: every check `ok`, then `all checks passed` (29 s wall, container start to teardown). In order: `container started`, `server ready and seeded`, `local proof`, `read is masked`, `--reveal shows values`, `--explain returns the plan`, `query refuses a write`, `apply commits DDL and DML`, `the plan's table exists`, `restore runs`, `restore removes the plan's table`, `count mismatch rolls back`, `plan in system database template1 refused`, `published port is not local`, `remote read works`, `remote refusals (6)`, `change script written, never executed` (17 checks). Plan, apply and restore and the remote-declared checks (the REMOTE profile: read-only refusals and the change script) all ran through the adapter, which the earlier report did not do. A second run with `--keep` (used for the probes below) also passed all 17. Backups (`pg_dump`) and the restore SQL are Python and container shell-outs and do not depend on the driver, as the plan says.

First attempt (not a result about the adapter): failed at `server ready and seeded` after the 600 s wait (620 s) because the compose shim of that moment did not set the `com.docker.compose.project.working_dir` and `service` labels, so profile discovery found no local profile. The shim's owner fixed it; the adapter was not changed.

### Behaviours proved directly against the server

A scratch script (not committed) fed the adapter requests on the container kept by `--keep`; the container was then removed by name. No password is printed or stored in the repo.

| Behaviour | Request | Outcome |
|---|---|---|
| Write refused by the server | read mode, `INSERT INTO customer ...` | `ok:false`, stage `execute`, `DbError 25006: cannot execute INSERT in a read-only transaction`; `DELETE FROM customer` the same; `SELECT count(*)` afterwards still 3 |
| Not defeatable by the request | `SET default_transaction_read_only = off` then `INSERT` | still `25006`; `SELECT 1` then `SET TRANSACTION READ WRITE` gives `25001: transaction read-write mode must be set before any query` |
| Session settings | `SHOW statement_timeout / lock_timeout / default_transaction_read_only / transaction_read_only` (timeout 7) | `7s`, `5s`, `on`, `on` |
| Statement timeout | `SELECT pg_sleep(5)`, timeout 1 | `DbError 57014: canceling statement due to statement timeout`, wall 1.04 s |
| Lock timeout | a write request holds `LOCK TABLE customer IN ACCESS EXCLUSIVE MODE` for 9 s; a read request (timeout 20) waits | `DbError 55P03: canceling statement due to lock timeout`, wall 5.05 s |
| Redaction | wrong user and password; wrong password for the real user | `DbError 28P01: password authentication failed for user "<user>"`; no password in the output |
| Redaction of the host | not provable here: the contract skips `127.0.0.1` and `localhost` | not run against a real host name; `core::redact` is the same function the E1 goldens cover |
| Non-database error | port 1 | `error connecting to server: Connection refused (os error 111)` |
| Errors by type, not text | syntax error, missing table | `42601` and `42P01` from `as_db_error().code()`, message only |
| One statement per entry | `SELECT 1; SELECT 2` | refused by the prepare (`42601`), as psycopg's extended protocol |
| Values | int, float8, text, NULL, bytea, numeric, bool, date, timestamp, timestamptz, jsonb | `7, 1.5, "x", null, "<2 bytes>", "1.50", true, "2026-11-07", "2026-01-02T03:04:05", "2026-01-02T03:04:05+00:00", "{\"a\": 1}"` |

`cargo test --features pg --lib pg` passes 2 unit tests (type conversion; a refused connection is a redacted connect failure).

### Differences from the Python driver, and what surprised

- **No TLS** (above): the main gap, and a silent one in `prefer` mode.
- The server-side refusal has two layers here (session default and `BEGIN READ ONLY`); the run did not isolate the session default alone, only that `SHOW default_transaction_read_only` is `on`.
- `rowcount` for statements whose command tag has no number: `EXPLAIN` and `SHOW` report 0 (psycopg reports the rows it received); **DDL reports 0, where psycopg reports -1**: the client crate keeps only the number from the command tag and drops the tag text, so the two cannot be told apart without parsing the statement (rejected, see the first review finding). The harness does not look at it (DDL entries carry no expected count), but a PostgreSQL connector fixture would.
- **Review finding 1 (fixed): the row count came from parsing the statement text.** The first version returned -1 for a statement with no result columns unless its first word was INSERT, UPDATE, DELETE, MERGE, COPY or WITH, so `/* change */ UPDATE customer SET name = 'x' WHERE id = 7` reported -1 and, with a plan expecting 1 row, verification rolled back a correct change. Confirmed against `dbrun.py` `execute()`, which reports `cur.rowcount`, the server's count. Fix: `shape_entry` takes the count from the server's `CommandComplete` (via the crate) and never reads the statement text, so comments, CTEs and `RETURNING` cannot change it. Tests: `rowcount_is_the_servers_count_whatever_the_statement_text` (pure), and on the real server `/* change */ UPDATE ...` with `expect: [1]` committed with rowcount 1, `-- c\nINSERT ...` with a wrong `expect` rolled back at stage `verify` ("expected 2, got 1").
- **Review finding 2 (fixed): `max_rows` was not applied in write mode.** `UPDATE ... RETURNING` returned every row. `dbrun.py` fetches all rows in write mode but slices `rows[: max_rows]` in both modes, and sets `truncated` only in read mode (`len(rows) > max_rows if read else False`). Fix: the cap now applies in both modes and `truncated` is true only in read mode, as Python. Tests: `max_rows_caps_returned_rows_in_both_modes` (pure), and on the real server `UPDATE customer SET name = name RETURNING id` with `max_rows: 2` returned 2 rows, `rowcount` 3, `truncated: false`; the read-mode `SELECT` with `max_rows: 2` returned 2 rows with `truncated: true`.
- After both fixes `cargo test --features pg --lib pg` passes 4 tests and the engines script passes all 17 checks again (24 s).
- `json`/`jsonb` come back as the server's JSON text; psycopg returns a Python object that `to_jsonable` turns into `str(dict)` (a Python repr). `timestamp` text is normalised to `isoformat()`; trailing-zero microseconds, `interval`, `time` and `timetz` are not.
- Rows are buffered whole before the `max_rows` cap, as psycopg's client-side cursor does; the cap bounds the output, not memory.
- The error prefix is `DbError <SQLSTATE>:` instead of Python's exception class (`ReadOnlySqlTransaction:`); anything that matches on the class name needs the code instead.
- The harness cannot run a networked engine on a host with no compose provider without a shim.
- Not exercised: a unix-socket host, a password with special characters, `connect_timeout` expiry, `idle_in_transaction_session_timeout`, PostgreSQL versions other than 17 (SCRAM-SHA-256 authentication worked).

## E3: MySQL and MariaDB native (T006, 2026-11-07)

Machine as E8 (Linux x86_64, Podman 5.4.2, rustc 1.98.1). Code: `rust/spikes/dbrun/src/drivers/mysql.rs`, wired in `src/bin/adapter.rs` behind the spike's existing `mysql` feature (without it the adapter falls back to the Python `_exec` bridge, as before). One code path serves both families; the adapter is not told which server it talks to.

**Crate and TLS.** `mysql` 28.0.4 with `default-features = false, features = ["minimal"]` (flate2 with system zlib, no TLS, no OpenSSL, no `rustls`). TLS decision: not enabled in the prototype, to match the Python runner, whose PyMySQL call passes no `ssl` argument. The crate has `rustls-tls` and `rustls-tls-ring` features, but neither was built or run, so TLS to MySQL and MariaDB remains unverified (a TLS build may pull in `aws-lc-rs` or `ring`, as E8 noted). MySQL 8.4's default `caching_sha2_password` account logged in over the plain connection with no extra option (the crate handles the authentication exchange itself); MariaDB 11.4 likewise.

**Behaviour copied from `dbrun.py connect()` and `execute()`.** `prefer_socket(false)` (the crate would otherwise turn host `localhost` into a Unix socket), TCP connect timeout 10 s, read and write timeout `timeout + 5` s, `autocommit = 0`; `SELECT VERSION()` first, then `max_statement_time = <seconds>` if the version string contains "mariadb" and `MAX_EXECUTION_TIME = <milliseconds>` otherwise; `innodb_lock_wait_timeout` and `lock_wait_timeout` of 5 s; read mode sets `SESSION TRANSACTION READ ONLY` at connect and `START TRANSACTION READ ONLY` after the identity queries; write mode commits only if the row counts match `expect`, else rolls back with stage `verify`. Results use the text protocol, so values are converted by column type like PyMySQL's decoders (integers and floats as numbers, DECIMAL as a string, DATETIME as `isoformat`, binary-charset string columns as `<N bytes>`). A SELECT's `rowcount` is the number of rows it returned (PyMySQL buffers), so the whole result is read and only `max_rows` are kept.

**Errors and redaction.** Database errors come through `mysql::Error`; a server error is rendered `MySqlError: ERROR <code> (<sqlstate>): <message>` from `MySqlError { code, state, message }`. The adapter then redacts the profile's password, host and user, in that order, with the spike core's `redact` (the contract's whole-word rule); `127.0.0.1` and `localhost` are skipped by that rule exactly as in the Python. Seen: `Access denied for user '<user>'@'192.168.68.109' (using password: YES)` and `Table '<user>.nope' doesn't exist` (the database is named like the user, `shop`, in the test profile). `192.168.68.109` is the client address the server reports back, not a connection fact in the request, so it is not redacted (the Python runner would show the same).

### Engine script, through the adapter

Build: `cd rust/spikes/dbrun && cargo build --offline --features mysql --bin adapter` (35 s with dependencies, 1 s warm). Command, from the repo root: `python3 scripts/test-dbrun-engines.py --runner rust/spikes/dbrun/target/debug/adapter --engine <mysql|mariadb> --port 15434` (`--port` only to keep clear of the PostgreSQL worker's containers). Final runs, the adapter built as above:

| Check | mysql:8.4 (8.4.13) | mariadb:11.4 (11.4.13) |
|---|---|---|
| container started | ok | ok |
| server ready and seeded | ok | ok |
| local proof | ok | ok |
| read is masked | ok | ok |
| --reveal shows values | ok | ok |
| --explain returns the plan | ok | ok |
| query refuses a write | ok | ok |
| apply commits DDL and DML | ok | ok |
| the plan's table exists | ok | ok |
| restore runs | ok | ok |
| restore removes the plan's table | ok | ok |
| count mismatch rolls back | ok | ok |
| plan in system database mysql refused | ok | ok |
| published port is not local | ok | ok |
| remote read works | ok | ok |
| remote refusals (6) | ok | ok |
| change script written, never executed | ok | ok |
| Summary | `all checks passed`, 32 s | `all checks passed`, 40 s (run with `--keep`) |

The same 17 checks gave `all checks passed` in an earlier mysql run (38 s, `--keep`) made before two small fixes found by the direct proofs below (DATE columns came back as `<10 bytes>`; the error text had a doubled wrapper). Neither affects an engine-script check. A first attempt stalled 627 s on `server ready and seeded` and failed that one check: the host had no compose provider and the stand-in did not yet label the container (see the setup note); it was not an adapter failure. Disk before pulling: 71 GB free of 225 GB (floor 15 GB); after, 68 GB.

Setup note (not in the repository). `podman compose` has no provider on this host (`docker-compose` and `podman-compose` are absent) and installing one was not allowed. The engine script needs only `up -d`, `down -v` and the compose labels `project`, `project.working_dir` and `service` (dbrun's local-proof discovery reads the last two). A 40-line stand-in `docker-compose`, a Python script in the session scratchpad outside the repository and first on `PATH`, ran `podman run -d --name dbrun-e2e-<engine>-<engine>-1 --label ... --memory <cap>` from the generated `compose.yaml` and removed that container with `podman rm -f -v`. The container lifecycle is therefore the script's own but through a shim, not a real compose: it creates no compose network (the published port and the container's own namespace are what the checks use). `--keep` left the first mysql and the mariadb container up only to run the direct proofs below; each was then removed by name, and `podman ps -a` shows none left.

### Direct proofs (the adapter fed one request, container kept up by `--keep`)

Seed profile `shop`, `read` mode. The classifier is not in the path, so these show what the server itself does.

| Proof | MySQL 8.4.13 | MariaDB 11.4.13 |
|---|---|---|
| write in read mode (`UPDATE customer ...`) | `ERROR 1792 (25006): Cannot execute statement in a READ ONLY transaction.` stage `execute` | `ERROR 1792 (25006): Cannot execute statement in a READ ONLY transaction` stage `execute` |
| timeout variable set after `SELECT VERSION()` (request timeout 2 s) | `SELECT @@max_execution_time` returns `2000` (ms) | `SELECT @@max_statement_time` returns `2.0` (seconds) |
| enforcement (request timeout 1 s, a 3-way `information_schema.columns` cross join) | `ERROR 3024 (HY000): Query execution was interrupted, maximum statement execution time exceeded` after 1.006 s | `ERROR 1969 (70100): Query execution was interrupted (max_statement_time exceeded)` after 1.007 s |
| wrong password (stage `connect`) | `ERROR 1045 (28000): Access denied for user '<user>'@'192.168.68.109' (using password: YES)` | same text |
| value types (`7, 1.5, DECIMAL, 'x', NULL, x'0102', NOW(), CURDATE()`) | `[7,"1.5","1.25","x",null,"<2 bytes>","2026-11-07T14:59:58","2026-11-07"]` | same shape |

So the write refusal is ERROR 1792 from the server in both families, not only from dbrun's classifier; the timeout variable is chosen per server and enforced; and one binary and one client crate served both, with no second client. MySQL's `max_execution_time` applies to SELECT only, which is the read path; a write statement is bounded by the socket timeout `timeout + 5` and `lock_wait_timeout`, as in the Python runner.

### Differences from `dbrun.py` found in review, and their fixes

An independent review of `mysql.rs` found two real differences; both were checked against `dbrun.py` and fixed.

1. Fractional datetimes. The driver turned `2026-11-07 14:59:58.12` into `2026-11-07T14:59:58.12`, while `to_jsonable()` calls `isoformat()`, which gives `2026-11-07T14:59:58.120000` (microseconds padded to six digits, and none at all when they are zero). Fixed: `iso_datetime()` pads to six digits and drops an all-zero fraction; a zero date such as `0000-00-00 00:00:00` is left as the string PyMySQL returns. TIME: PyMySQL decodes TIME to a `timedelta`, not a `datetime.time`, and `to_jsonable()` falls through to `str()`, so the target is `str(timedelta)` (`1:02:03`, `1:02:03.120000`, `1 day, 0:00:00`, `34 days, 22:59:59` for `838:59:59`, `-1 day, 22:57:57` for `-01:02:03`), not `time.isoformat()`. Fixed: `py_timedelta()`. Tests `datetime_matches_python_isoformat` and `time_matches_python_str_of_timedelta` (expected values taken from Python itself) cover zero, partial (`.12`, `.123`) and six-digit fractions, `.000001`, zero dates, day rollover and negative values. Not run against a live server: the fractional-TIME and DATETIME(n) columns; only the pure conversion is tested.
2. Panic on a bad `timeout`. A negative, huge or non-numeric `timeout` reached `Duration::from_secs_f64`, which panics, so the adapter printed nothing. In `dbrun.py` the same inputs fail at stage `connect` (a negative `MAX_EXECUTION_TIME` is refused by the server, `int(nan)` raises inside `connect()`, which `execute()` catches). Fixed: `timeout_secs()` validates before connecting; a missing timeout means 30 s, a finite value of zero or more is accepted and capped at 86400 s, and anything else (negative, a non-number, a string) returns `{"ok": false, "stage": "connect", "error": "ValueError: invalid timeout"}`. The cap is a spike addition: `dbrun.py` has no cap in `execute()`. A bare `NaN` is not valid JSON, so the adapter's request parser already answers it with `invalid execution request JSON`. Tests: unit tests `timeout_is_validated_and_clamped`, `invalid_timeouts_fail_as_json_and_never_panic`, `a_huge_timeout_does_not_panic_before_connecting`, and `tests/mysql_request.rs`, which runs the adapter binary with `-6`, `-1e-9`, `"NaN"`, `1e300`, `1e999` and a bare `NaN` and requires one line of failure JSON at stage `connect` and an empty stderr.

Results after the fixes: `cargo test --offline --features mysql --lib` 7 passed (5 new, plus the 2 existing process tests); `cargo test --offline --features mysql --test mysql_request` 1 passed. The engine script, re-run once per engine with the rebuilt debug adapter (`--port 15434`, no `--keep`): mysql `all checks passed` (30 s), mariadb `all checks passed` (28 s), 17 of 17 checks each. `podman ps -a` is empty afterwards; disk was 63 GB free before the runs (floor 15 GB).

### Not verified

- TLS to either server (no TLS feature built).
- A write statement running past the timeout on MySQL (`max_execution_time` does not cover it; only the socket timeout does, not exercised).
- MySQL 5.7, Percona, a MariaDB older than 11.4, other character sets, `TIME`, `BIT`, `JSON`, `GEOMETRY` and zero-date values (the conversion handles them by type, but only the types in the proof row ran), and results with several result sets.
- Windows and macOS (E7), and Unix-socket connections (the adapter always uses TCP).
- Cold start against MySQL: the only number is the first connect in a debug build, `mode: test`, 0.156 s (the adapter's `seconds`), against the report's 160 ms. The mysql container was already gone when the release binary was timed.

## E5: SQL Server native (T007, 2026-11-07)

Machine as E8 (Linux x86_64, Podman 5.4.2). Code: `rust/spikes/dbrun/src/drivers/mssql.rs` (about 690 lines, about 190 of them tests), wired in `src/bin/adapter.rs` behind the spike's `mssql` feature; `--features pg,mysql,mssql` builds and the one dispatcher routes `postgresql`, `mysql`/`mariadb` and `sqlserver` to their native drivers (each answers a refused port with its own `connect` failure JSON); without the feature SQL Server still goes through the Python `_exec` bridge. Server: `mcr.microsoft.com/mssql/server:2022-latest` (SQL Server 2022 RTM-CU27-GDR, 16.0), 2 GB cap, `127.0.0.1:15435`, started by `scripts/test-dbrun-engines.py` through the same stand-in `docker-compose` shim as E2 and E3 (a Python script in the session scratchpad, outside the repository). Only this engine ran, nothing else concurrently. Disk before the pull 61 GB free of 225 GB (floor 15 GB), 54 GB after the pull and the builds.

### Candidates

| Candidate | What was done | Result |
|---|---|---|
| `tiberius-ng` 0.13.1 (`default-features = false`, `tds73`, `rustls`) | built the driver, ran the engine script and the direct proofs | the choice; all below |
| `tiberius` 0.13.0 (same features) | the same source, only the dependency line changed (a scratch copy outside the repository), built, then six requests compared with the `tiberius-ng` adapter on the live server (a type row, all 300 legacy `datetime` ticks, an `UPDATE ... OUTPUT` with `max_rows`, a syntax error, a timeout, a capped read) plus `encrypt_option` | compiles unchanged (the `Client`, `simple_query`, `Config`, `Row` and `ColumnData` surface this driver uses is the same) and every request answered byte for byte the same, TLS on. So the fallback exists and costs a dependency line. Not run through the whole engine script. |
| `sqlcmd` in a client image | not run | judged by interface only (and my attempt to call `sqlcmd` directly was stopped by the repository's db guard, correctly). `sqlcmd` prints text tables: no column types, so dates, decimals, money and binary could not be rendered as `to_jsonable` does; the row count would be parsed from `(N rows affected)` text, which is the statement-text parsing the E2 review rejected; a client image needs the Microsoft ODBC driver and its licence; each request is a container start. Rejected without a build. |

### Choice

`tiberius-ng` 0.13.1, pure Rust, TDS 7.3+, rustls (no OpenSSL, no FreeTDS, no ODBC). Runtime libraries of the release binary: `libgcc_s` and `libc` only. It is async, so the driver builds a current-thread Tokio runtime per request (`tokio` features `rt`, `net`, `time`; `futures-util` and `tokio-util` for the stream and the compat layer). The whole request is one `block_on`. `rustls` brings `aws-lc-rs` (a cmake and C build of `aws-lc-sys`), as E8 found for the mssql set; no `ring`.

### Fork health of `tiberius-ng` (retrieved 2026-11-07)

Retrieved offline from the downloaded crate sources (`~/.cargo/registry`, the cargo index through `cargo info`); the host rule allowed no other network, so nothing on GitHub or crates.io's web pages was fetched. Facts marked "not retrieved" are the ones a person must check before a production decision.

- Name and origin. `cargo info tiberius-ng`: version 0.13.1, MIT OR Apache-2.0, rust-version 1.88, repository `github.com/MattJackson/tiberius-ng`, description "actively-maintained community continuation of tiberius". The library is still imported as `tiberius`. Its `CHANGELOG.md` says it is a maintained community fork of `github.com/prisma/tiberius` and that entries up to 0.12.3 are the upstream history. Source: `tiberius-ng-0.13.1/CHANGELOG.md` and `Cargo.toml`.
- Is it a fork of a maintained crate? Of a crate that is not maintained, by its own account: `README.md` says the original "is no longer maintained upstream". Plain `tiberius` 0.13.0 does exist (repository `prisma/tiberius`, `Changes` file starting at "Version 0.13.0", rustls and `rustls-webpki-roots` features), so the original is not absent; I could not check its recent activity. The two sources have diverged: `diff -r` of the two `src/` trees shows 77 differing entries and about 14,500 diff lines.
- Age and releases. Changelog dates: 0.13.0 on 2026-08-29 (the first release under the new name; "0.13.0-alpha.1 was published to crates.io to claim the name") and 0.13.1 on 2026-08-30, so about 10 weeks old on the retrieval date, two releases in two days, none since in the changelog. The Unreleased section says: "0.13.1 is a stable settling point ... No further releases are planned in the immediate term; the crate is considered stable for production use." That is the maintainer's statement that the crate is done, not that it is being developed.
- Maintainers. `Cargo.toml` authors: Matthew Jackson, Steffen Butzer, Julius de Bruijn, Eric Sheppard, Yoshua Wuyts (the last four are the upstream authors). `CODEOWNERS` and `SECURITY.md` exist; the security policy supports only the 0.13.x series. One current maintainer is the repository owner; I found no sign of a second.
- Claims about quality, taken from the changelog and README and not checked: `cargo-deny` with zero advisories enforced in CI, about 93 percent line coverage against a live SQL Server, a mutation-testing harness with "100% mutation score". Not retrieved: GitHub open issue and pull request counts, last commit date, stars, crates.io download counts and the exact publish timestamps.
- If it is abandoned. (1) `Cargo.lock` pins the checksum (the spike's `Cargo.lock` holds the registry checksum of `tiberius-ng` 0.13.1), so a yanked or vanished crate does not break an existing build. (2) The licence allows vendoring and the surface the connector needs is small (connect, a batch, rows, errors). (3) Plain `tiberius` 0.13.0 is a measured drop-in for this driver (candidates table), though its own maintenance is the same open question. (4) The wire protocol is Microsoft's published MS-TDS, so a fork is possible but would be work owned by the project. Risk: moderate, because the replacement for an abandoned `tiberius-ng` is another unmaintained crate, not a maintained one; the mitigation is the small surface and a pinned version.

### TLS decision

rustls with the crate's default encryption level `Required`, and `trust_cert()` (the certificate is not verified) for the self-signed test image only. Measured on the server: `sys.dm_exec_connections.encrypt_option` is `TRUE` for the adapter's session and `FALSE` for the Python runner's (pymssql). So the Rust path encrypts where the Python path did not (E5 improves on the Python runner for loopback, but it also fails against a server that refuses encryption, where pymssql connects). `trust_cert()` is unacceptable outside the test image: a production connector needs a profile field to trust a given CA file (`trust_cert_ca`) or to verify against the system roots (the `rustls-native-certs` feature is already enabled), and the request JSON carries neither today. A test against a server with a CA-signed certificate was not run.

### Engine script, through the adapter

Build: `cd rust/spikes/dbrun && cargo build --features mssql --bin adapter` (cold 40 s including `aws-lc-sys`; 1 s warm). Command, from the repo root with the shim's directory first on `PATH`: `python3 scripts/test-dbrun-engines.py --runner rust/spikes/dbrun/target/debug/adapter --engine sqlserver --port 15435`. Three runs, all `all checks passed`; the last (after formatting, no `--keep`) in 42 s, 17 of 17:

`container started`, `server ready`, `local proof`, `read is masked`, `--reveal shows values`, `--explain returns the plan`, `query refuses a write`, `apply commits DDL and DML`, `the plan's table exists`, `restore runs`, `restore removes the plan's table`, `count mismatch rolls back`, `plan in system database master refused`, `published port is not local`, `remote read works`, `remote refusals (7)`, `change script written, never executed`: every one `ok`.

Plan, apply and restore and the remote-declared checks all ran through the adapter, except that the `BACKUP`/`RESTORE` statements go through `sqlserver_autocommit`, which `dbrun.py` runs as its own pymssql process (they cannot run inside a transaction), so those two checks do not exercise the Rust driver.

### SQL Server has no server-side read-only mode

PostgreSQL has `default_transaction_read_only`, MySQL and MariaDB a `READ ONLY` transaction (E2, E3). SQL Server has neither: there is no session or transaction setting that makes the server refuse a write (`ApplicationIntent=ReadOnly` only routes to a readable secondary and the server does not enforce it on a single instance; a `db_datareader`-only login is a database-permission choice the owner makes, not something the runner can switch on). So in read mode the server does not refuse a write. Read mode is protected by two layers that are ours:

1. The classifier and the statement scanner in `dbrun.py` (`classify`, `split_statements`), which decide before any connection is opened. They are what refuses `INSERT`, `UPDATE`, `DELETE`, DDL, `EXEC` and the rest. The engine script's `query refuses a write` and `remote refusals (7)` checks pass through them.
2. A transaction that read mode never commits: the adapter opens `BEGIN TRANSACTION` and always sends `ROLLBACK TRANSACTION` (as `dbrun.py` does with `conn.rollback()`). SQL Server DDL is transactional, so a rolled-back `CREATE TABLE` also disappears.

Proved directly, the classifier not in the path (the adapter fed read-mode requests, container kept up): `UPDATE customer SET name = 'HACKED'` returned `ok: true, rowcount: 3`; `INSERT`, `DELETE FROM customer`, `CREATE TABLE leak_t` and `DROP TABLE customer` all returned `ok: true`. The server executed them. Afterwards, in a fresh session, `customer` still had 3 rows, 0 rows named `HACKED` and no `leak_t` table, so nothing persisted. The Python runner gave the same results for the same requests. The consequence is stated plainly: a write that gets past the classifier runs on the server, holds locks while it runs, fires triggers, and can have effects a rollback does not undo (an `EXEC` of a procedure that calls out, `xp_cmdshell` where enabled, a sequence value consumed, a linked-server call, `WAITFOR`). The classifier is the real protection, and the account's privileges are the second one, which the profile should keep to `db_datareader` for read use. The adapter has no classifier of its own, so it is not a layer by itself; the spike's ported classifier (E1) could be called in it as a defence in depth, which this run did not do.

### Behaviours proved directly against the server

A scratch script in the session scratchpad (not in the repository) fed the same request to `dbrun.py _exec` (`uv run --offline --with pymssql==2.4.4`) and to the adapter, on the container kept by `--keep`; the container was then removed by name. No password is printed or stored in the repository.

| Behaviour | Outcome |
|---|---|
| Session settings (`@@LOCK_TIMEOUT`, `APP_NAME()`, `@@TRANCOUNT`, `DB_NAME()`) | `5000`, `ai-gent-dbrun`, `1`, `shop`; identical to pymssql |
| Statement timeout | `WAITFOR DELAY '00:00:04'` with timeout 1: `TimeoutError: statement exceeded 1 s`, stage `execute`, wall 1.04 s (pymssql: `OperationalError (20047 ...)` at 1.1 s) |
| A write that times out persists nothing | `UPDATE customer SET name = 'T'; WAITFOR DELAY '00:00:06'` in write mode, timeout 1: stage `execute` at 1.04 s; afterwards 0 rows named `T`. The connection is dropped without a rollback (the client cannot send one while the server is still running the statement; a first version tried and waited 4 s), and the server undoes the open transaction |
| Lock timeout | a write request holds a row lock for 9 s; a read request (timeout 20) waits: `MssqlError 1222 (state 51, class 16): Lock request time out period exceeded.` after 5.05 s |
| Row count mismatch rolls back | `UPDATE customer SET name = 'M'` with `expect: [1]`: stage `verify`, "expected 1, got 3"; afterwards 0 rows named `M` |
| Row count from the server | `UPDATE ... WHERE id <= 2` 2, `WHERE id = 9999` 0, `/* c */ UPDATE ...` 3, `INSERT` 1, `DELETE` 1, a `WITH ... UPDATE` 3, two statements in one request `UPDATE ...; UPDATE ... WHERE id = 1` 1: all equal to pymssql |
| `max_rows` in both modes | write mode `UPDATE ... OUTPUT inserted.id` with `max_rows: 2`: 2 rows, `truncated: false`, `rowcount` 3; read mode `SELECT` with `max_rows: 2`: 2 rows, `truncated: true`; `TOP 100000` rows with `max_rows: 5`: 5 rows kept (only `cap + 1` are converted) in 0.43 s |
| Errors | syntax error `MssqlError 2812 (state 62, class 16): Could not find stored procedure 'SELEC'.`; missing table `MssqlError 208 (state 1, class 16): Invalid object name 'nope'.`; refused port `Connection refused (os error 111)`; the same stages as pymssql (`execute`, `connect`) |
| Redaction | wrong password and a wrong user: `MssqlError 18456 (state 1, class 14): Login failed for user '<user>'.`, no password in the output. The host `127.0.0.1` and `localhost` are skipped by the contract's rule, so host redaction was not shown on a real name |
| Invalid timeout | `-6`, `"NaN"` and a bare `NaN`, `-1e-9`: `{"ok": false, "stage": "connect", "error": "ValueError: invalid timeout"}`, no panic, empty stderr (`tests/mssql_request.rs`, plus unit tests). `0` means no statement timeout, as pymssql (`timeout=0`); `1e300` and `1e999` are capped at 86400 s |
| `--explain` | `SET SHOWPLAN_TEXT ON`, the plan is the second result set, as `dbrun.py`: the `Clustered Index Seek` plan text equal to pymssql's |
| Login to a database that does not exist | `MssqlError 4060 ... Cannot open database "nodb"` (pymssql says 18456 for it) |

### Value rendering against pymssql

The adapter and pymssql were fed the same `SELECT` of every type, and the strings compared. Values rendered like Python's `isoformat()` and `str()`:

- integers, `bit` (bool), `float` and `real` (`CAST(0.1 AS real)` is `0.10000000149011612` in both), `decimal` and `numeric` (`str(Decimal)`: `1.50`, `0.05`, `-3`, `12345.6789`, and `decimal(38,0)`), `char` padding, `nvarchar` Unicode, `uniqueidentifier` (lowercase hyphenated), `varbinary` and `binary` (`<N bytes>`), `xml`, `date` (including `0001-01-01` and `9999-12-31`), `time(n)` (the scale-0 value has no fraction, a fraction is always six digits, and `time(7)` is truncated to microseconds: `23:59:59.9999999` is `23:59:59.999999`), `datetime2(n)`, `smalldatetime`, `datetimeoffset` (the stored wall time and its offset: `2026-11-07T14:59:58+05:30`, `...-03:00`), `money` and `smallmoney` (four decimals: `12.3400`, `-0.5000`).
- Legacy `datetime`: all 300 ticks of a second (`CAST(0x00000000 + CAST(k AS binary(4)) AS datetime)`, k from 0 to 299) rendered the same as pymssql, which shows pymssql's FreeTDS rounds to SQL Server's `.000 .003 .007 .010 ...` milliseconds and then pads to microseconds (`.003000`).
- Two review-style findings, both fixed before the run above. (1) `datetimeoffset`: the first version printed the UTC time TDS carries (`09:29:58+05:30`); pymssql shows the wall time, so the offset is added back (`shift_minutes`, with the day carry). (2) `money` arrives from the client crate as a binary float, so it is rebuilt as a four-decimal `Decimal` string by column type.
- Python-computed expectations in the unit tests: the dates (`date(2026, 11, 7)` is 739926 days after `0001-01-01`, `date(1969, 12, 31)` 719161, ...), `time(1, 2, 3, 120000).isoformat()`, `Decimal` strings, and the `datetime` tick table; `cargo test --features mssql --lib` gives 14 passed, `--test mssql_request` 1 passed (12 of the 14 are in `mssql.rs`; the other two are the process-bridge tests of the lib).

### Differences from the Python driver, and what surprised

- **Row count of statements that return rows.** pymssql reports `-1` for a `SELECT`, an `OUTPUT` statement, a stored procedure and `DECLARE ...; SELECT` (before the rows are fetched; `0` for an empty `SELECT`) and `-1` for DDL, `SET` and `PRINT`. The adapter reports the server's count: the rows returned for a statement with a result set, and `0` for DDL, `SET` and `PRINT` (the client crate does not say a DONE token carried no count). A plan that expected the Python `-1` for an `OUTPUT` statement would mismatch here, and the harness does not look at DDL counts. The count is read with `SELECT ROWCOUNT_BIG()` in the same session straight after the statement, from the server, never from the statement text: the client crate exposes DONE counts only through its RPC path, which drops the column names of an empty result set, and its streamed batch path does not expose them at all.
- **`sql_variant`** columns come back typed (`1`, `"abc"`, `"2026-11-07"`); pymssql returns raw bytes (`<4 bytes>`). **`money` beyond about 9e11** loses digits through the float (`922337203685477.5807` printed as `...5808`). Neither was fixed; both are rare.
- **The count probe is visible to the next statement** (found in the Codex review). After each write statement the adapter sends `SELECT ROWCOUNT_BIG()` as its own batch, and that SELECT leaves `@@ROWCOUNT` at 1, so a following statement that reads `@@ROWCOUNT` of the one before (`UPDATE …` then `IF @@ROWCOUNT = 0 INSERT …`) sees 1 where pymssql, which sends nothing between, shows the real count. No T-SQL statement can restore `@@ROWCOUNT`, and the client crate exposes no DONE counts, so this is not fixable in the driver: a production connector must either read the count from the DONE token or send each plan's statements as one batch. Not fixed in the spike.
- **Control batches are bounded.** Review found that the session setup, identity, transaction and showplan batches had no timeout; they now run under the request's timeout like a statement (none when it is zero).
- **Error text.** `MssqlError <number> (state, class): message` from the server's error token, without pymssql's `DB-Lib error message 20018 ...` suffix; anything that matches on `ProgrammingError:` or `OperationalError:` needs the number. The timeout error is `TimeoutError: ...` (a name of the spike's), not pymssql's 20047.
- **No timeout from the library.** pymssql has `timeout=`; `tiberius-ng` has none, so each statement runs under `tokio::time::timeout`; login and TCP connect are each capped at 10 s, as pymssql's `login_timeout`. Rows are buffered per statement only up to `cap + 1`, but the server still sends the rest.
- **`@@ROWCOUNT` and `SET NOCOUNT`.** The connect step does not set `NOCOUNT`, as `dbrun.py` (apply needs the counts). `ROWCOUNT_BIG()` is documented as independent of `NOCOUNT`, but a statement that sets `NOCOUNT ON` itself was not tested.
- **TLS differs** (above): the Rust path negotiates encryption and the Python runner did not.
- **The plain `tiberius` fork question** matters more than the crate API: the code needed nothing but a dependency line to move.
- The compose shim was needed again (no provider on this host).

### Not verified

- A server with a CA-signed certificate, `trust_cert_ca`, and TDS 8.0 strict encryption.
- Azure SQL, SQL Server 2019 or older, named instances (`sql-browser` is not enabled), Windows authentication and Azure AD tokens. Only SQL logins against 2022.
- The two statements that need autocommit (`BACKUP DATABASE`, `RESTORE DATABASE`) through the Rust driver: the adapter always runs a transaction. They stay a Python shell-out as in the current `dbrun.py`.
- Several statements in one request with several result sets (only the first is used, as pymssql), `sql_variant`, `hierarchyid`, spatial types and CLR types other than as text, and `NOCOUNT ON` behaviour.
- `sqlcmd` in a client image (not built) and the plain `tiberius` crate through the whole engine script.
- Windows and macOS (E7), cross-compilation.
- Fork health items marked "not retrieved" above.
