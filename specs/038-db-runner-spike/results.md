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
