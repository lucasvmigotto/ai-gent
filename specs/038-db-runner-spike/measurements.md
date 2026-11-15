# Measurements: 038 dbrun spike

Status: Draft. The numbers below come from the research pass in `docs/product/dbrun-spike-report.md` (2026-11-01), taken on one machine in a sandbox; the prototype and its goldens lived in a scratchpad and are **not in the repo**, so each line is to be reproduced by the experiment named. Nothing here is a spike result yet.

| Measure | Value | Experiment to reproduce | Marker |
|---|---|---|---|
| Safety core port | about 330 lines of Rust; zero diffs on 1026 classify verdicts, 1026 splits, 1026 `is_ddl`, 4104 cost cases, mask, redact, network (reads only; goldens generated from the Python) | E1 | measured |
| sqlparser 0.63 as classifier | 163 of 1026 disagree; 119 say READ where dbrun says WRITE | E1 | measured |
| Cold start, sqlite | 48-65 ms | E4 | measured |
| Cold start, PostgreSQL 17 (NoTls) | 88 ms | E2 | measured |
| Cold start, MySQL 8.4 | 160 ms | E3 | measured |
| Cold start, MariaDB 11.4 | 57 ms | E3 | measured |
| Cold start, SQL Server 2022 (tiberius-ng + rustls, `trust_cert`) | 69 ms | E5 | measured |
| Python baseline | stdlib start 24 ms; warm `uv run --with PyMySQL` 176 ms before connecting | E8 | measured |
| Binary size (x86_64 gnu, opt-level z, LTO, strip) | core 1.56 MB; + sqlite 2.63; + postgresql 2.09; + mysql 1.98; + mssql (rustls) 3.72; sqlite + postgresql + mysql 3.52 MB (builds 20-66 s) | E8 | measured |
| All drivers together | 5-6 MB | E8 | estimate, not built |
| Server-side write refusal | sqlite, PostgreSQL 17, MySQL 8.4, MariaDB 11.4 shown; SQL Server has none | E2-E5 | measured |

Not run: Oracle (E6a, E6b); plan, apply and restore; the remote-declared checks; PostgreSQL and MySQL TLS; musl; aarch64; Windows (E7).

## E8 re-measured in the repo spike (T009, 2026-11-03)

Linux x86_64 (i7-1165G7, 8 threads), rustc 1.98.1, gcc 14.2.0, `dbq` release profile of `rust/spikes/dbrun/Cargo.toml` (already stripped). Method and toolchain notes in `results.md` "E8". The rows above stay as the report's figures.

| Driver set | Size MB | Cold build | Warm rebuild | Crypto and C needs observed | Cold start median / p95 (refuse path) | Marker |
|---|---|---|---|---|---|---|
| core | 1.56 | 10.0 s | 5.8 s | none | 0.6 / 0.9 ms | measured |
| + sqlite | 2.65 | 26.8 s | 6.1 s | C compiler (bundled SQLite) | 0.7 / 0.8 ms; `SELECT 1` 6.3 / 7.1 ms | measured |
| + postgres | 2.09 | 24.6 s | 8.7 s | none (NoTls) | 0.6 / 0.8 ms | measured |
| + mysql | 1.98 | 20.8 s | 8.0 s | C compiler runs, links system `libz.so.1` | 1.4 / 1.9 ms | measured |
| + mssql | 3.72 | 38.1 s | 11.5 s | aws-lc-rs via rustls (aws-lc-sys: C and asm, no ring) | 1.8 / 2.1 ms | measured |
| all four | 5.58 | 63.3 s | 15.8 s | aws-lc-rs plus the above | 2.1 / 2.4 ms; `SELECT 1` 7.1 / 7.8 ms | measured (the report's 5-6 MB estimate) |
| musl core / + postgres | 1.66 / 2.22 | 9.7 s / 16.2 s | not measured | host gcc suffices | core 1.0 / 1.2 ms | measured |
| musl + sqlite, mysql, mssql | | | | failed: no `x86_64-linux-musl-gcc` | | measured (failure) |
| aarch64, Windows, macOS | | | | not run: no cross C compiler, linker or host | | not run |

Same-machine baselines: `python3 -I -c pass` 7.4 / 8.5 ms; warm `uv run --with PyMySQL` 46.2 / 48.0 ms (the 176 ms figure above is from another machine). Median and p95 over 50 runs after 5 discarded.

## E2 PostgreSQL through the adapter (T005, 2026-11-07)

Same machine as E8. Method and results in `results.md` "E2". Server `postgres:17` (17.11) in podman, 512 MB cap, loopback.

| Measure | Value | Marker |
|---|---|---|
| `scripts/test-dbrun-engines.py --runner .../adapter --engine postgresql`, whole run (start, seed, 17 checks, teardown) | 29 s wall (25 s on a second run with `--keep`) | measured |
| Adapter, one native request (connect, identity, links, statement, rollback) | about 0.04 s wall per request (`seconds` field 0.04 to 0.07) | measured |
| Statement timeout 1 s on `pg_sleep(5)` | refused at 1.04 s | measured |
| Lock timeout (5 s) | refused at 5.05 s | measured |
| Adapter, release profile, `--bin adapter` without `pg` | 388,984 bytes | measured |
| Adapter, release profile, `--features pg` | 957,984 bytes | measured |
| Size delta of the PostgreSQL driver | +569,000 bytes (+0.57 MB) | measured |
| Debug build with `pg` (what the script ran) | 43,350,360 bytes; cold build 41 s including the dependency tree | measured |
| Runtime libraries of the release `pg` adapter (`ldd`) | `libgcc_s`, `libc` | measured |
| First attempt, compose shim without labels | 620 s (the 600 s readiness wait), failed `server ready and seeded` | measured (harness, not the adapter) |
| Python driver (`uv run --with psycopg`) on the same checks | not run (a driver fetch needs the network) | not run |

The size is the `adapter` binary alone (no safety-core CLI), so it differs from the `dbq` rows above.

## E3 MySQL and MariaDB through the adapter (T006, 2026-11-07)

Same machine as E8. `mysql` crate 28.0.4, `minimal` features, no TLS. Details and the full check table in `results.md` "E3".

| Measure | Value | Marker |
|---|---|---|
| Engine script, `--engine mysql` (8.4.13) | 17 of 17 checks ok; 32 s wall (38 s in an earlier run with `--keep`) | measured |
| Engine script, `--engine mariadb` (11.4.13) | 17 of 17 checks ok; 40 s wall | measured |
| Server-side write refusal | ERROR 1792 (25006) on both, in read mode with no classifier in the path | measured |
| Timeout variable | MySQL `max_execution_time` = 2000 for a 2 s request, ERROR 3024 after 1.006 s on a 1 s request; MariaDB `max_statement_time` = 2.0, ERROR 1969 after 1.007 s | measured |
| Adapter release binary, bridge only (no native driver) | 388824 bytes | measured |
| Adapter release binary, `--features mysql` | 2044048 bytes (2.04 MB); delta +1655224 bytes (+1.66 MB) over the bridge-only adapter | measured |
| Runtime libraries of that binary | `libz.so.1`, `libgcc_s.so.1`, `libc` (the system zlib dependency E8 found) | measured |
| Build, `--features mysql` | debug 35 s with dependencies, 1 s warm; release 19 s (the bridge-only release build already in the target dir) | measured |
| One request, release adapter vs MariaDB 11.4 (process start, connect, session setup, identity, one SELECT) | median 3.8 ms, p95 4.1 ms, 50 runs after 5 discarded | measured |
| First connect to MySQL 8.4, debug build, test mode | 0.156 s | measured (one run) |
| Python baseline on this machine | warm `uv run --with PyMySQL` import alone 46.2 ms | from E8 |

Surprises: (1) the host has no compose provider, so the engine script could not start a container until a stand-in with the compose labels was supplied (see `results.md`); (2) the text protocol reports DATE and TIME as the binary charset, so a "charset 63 means blob" rule renders a date as `<10 bytes>`, fixed by deciding on column type first; (3) the MySQL and MariaDB timeout units differ by a factor of 1000, and MariaDB reports the variable as a float.

## E5 SQL Server through the adapter (T007, 2026-11-07)

Same machine as E8. `tiberius-ng` 0.13.1 (`tds73`, `rustls`), image `mcr.microsoft.com/mssql/server:2022-latest` (SQL Server 2022 RTM-CU27-GDR), 2 GB cap, loopback, one engine at a time. Details and the 17-check list in `results.md` "E5".

| Measure | Value | Marker |
|---|---|---|
| Engine script, `--engine sqlserver` | 17 of 17 checks ok in three runs; 70 s (first, includes the image start and first seed), 35 s, 42 s (final, no `--keep`) | measured |
| Image pull (`mcr.microsoft.com/mssql/server:2022-latest`) | done once; disk 61 GB free before, 54 GB after the pull and the builds (floor 15 GB) | measured |
| Adapter release binary, bridge only | 387600 bytes | measured |
| Adapter release binary, `--features mssql` | 2599528 bytes (2.60 MB); delta +2211928 bytes (+2.21 MB) over the bridge-only adapter | measured |
| Runtime libraries of that binary | `libgcc_s.so.1`, `libc` (no OpenSSL, no zlib) | measured (`ldd`) |
| Build, `--features mssql` | debug cold about 40 s (`aws-lc-sys` through cmake and a C compiler), 1 s warm; release 36 s with the dependencies | measured |
| One request, release adapter vs SQL Server 2022 (process start, TCP, TLS handshake, login, session setup, identity, links, one `SELECT 1`, rollback) | median 65.4 ms, p95 104.2 ms, 50 runs after 5 discarded | measured |
| Python runner, same request (`uv run --offline --with pymssql`, warm cache) | median 104 ms (15 runs) | measured |
| Statement timeout 1 s on `WAITFOR DELAY '00:00:04'` | refused at 1.04 s (a first version that tried to roll back before dropping the connection took 4.04 s) | measured |
| Lock timeout (5 s) | refused at 5.05 s | measured |
| 100,000-row read with `max_rows: 5` | 0.43 s, 5 rows returned | measured |
| TLS on the session | `encrypt_option = TRUE` for the adapter; `FALSE` for pymssql | measured |
| Plain `tiberius` 0.13.0, same source | builds, answers 6 requests identically (a scratch copy outside the repository); size and time not measured | measured (behaviour only) |
| `sqlcmd` in a client image | not run | not run |
| Unit tests | `cargo test --features mssql --lib` 14 passed (12 in `mssql.rs`); `--test mssql_request` 1 passed; `clippy` and `rustfmt` clean for the new files | measured |

Fork-health facts (retrieved 2026-11-07 from the crate sources and `cargo info` only; no web access): `tiberius-ng` 0.13.1 released 2026-08-30 after 0.13.0 on 2026-08-29 (changelog dates), MIT OR Apache-2.0, rust-version 1.88, one active maintainer on the repository `MattJackson/tiberius-ng`, a fork of `prisma/tiberius` whose README says it is no longer maintained, changelog says "No further releases are planned in the immediate term", `src/` differs from plain `tiberius` 0.13.0 by about 14,500 diff lines. Not retrieved: GitHub issue counts, last commit date, stars, downloads.

Surprises: (1) the client crate gives no row count for a streamed batch and its RPC path drops the column names of an empty result set, so the count comes from `SELECT ROWCOUNT_BIG()` in the same session; (2) pymssql returns `-1` as the row count of a `SELECT` and of DDL, the adapter the server's count; (3) `datetimeoffset` travels as UTC and pymssql shows the stored wall time; (4) the server accepts and runs a write in read mode, and only the rollback undoes it (no server-side read-only mode); (5) a timed-out statement cannot be rolled back from the client, the dropped connection does it; (6) the Rust path encrypts the session where pymssql did not.

## E6 Oracle through the adapter (T008, T014, 2026-11-07)

Same machine as E8. `oracledb` 26.0.0-beta.5 (thin), image `docker.io/gvenzl/oracle-free:23-slim` (Oracle 26ai Free), 2 GB cap, loopback. Details in `results.md` "E6".

| Measure | Value | Marker |
|---|---|---|
| Engine script, `--engine oracle` | 14 of 14 checks ok in two runs; 58 s and 59 s including container start and seed | measured |
| Image pull | `gvenzl/oracle-free:23-slim` 1.97 GB; disk 58 GB free before, 52 GB after the pull and builds (floor 15 GB); container memory 96.7 MB of the 2 GB cap when idle after the run (`podman stats`) | measured |
| Adapter release binary, `--features oracle` only | 2,780,000 bytes (2.78 MB) | measured |
| Runtime libraries | `libgcc_s.so.1`, `libc` (no Oracle client, no OpenSSL) | measured (`ldd`) |
| Build, `--features oracle --bin adapter` | release cold 43.8 s with the dependencies, 16 s after an edit | measured |
| One request vs Oracle Free (process start, TCP, login, identity, links, `SELECT 1`, rollback) | Rust median 39.8 ms, p95 67.5 ms (50 runs after 5 discarded); Python `uv run --offline --with oracledb` median 337.5 ms, p95 786 ms (15 runs after 2) | measured |
| 100,000-row read, `max_rows 5` | Rust 63.6 ms, Python 482 ms (3 runs after 1 discarded; both stop at `cap + 1`) | measured |
| Statement timeout 1 s on `DBMS_SESSION.SLEEP(8)` | refused at 2.1 s (Python 2.1 s): break plus a second wait | measured |
| Refused port | 0.0002 s with `(RETRY_DELAY=0)`; 1.00 s with the default or an Easy Connect `retry_delay` | measured |
| Wrong password | about 1.0 s (server-side delay; Python 1.3 s) | measured |
| `oracle` 0.6.3 probe | builds (957 KB, C compiler only); `DPI-1047` without a client library; SIGSEGV with the server home's libclntsh on this host; no query measured | measured (failure only) |
| 10G verifier | not reproducible on 26ai (`ORA-02153`); 11G-only account: sqlplus and Python thin log in, `oracledb` Rust gets `ORA-01017` | measured |
| TLS | not run against a server; `tcps://` to a plain listener fails in 1.0 s | measured (negative probe only) |
| Unit tests | `cargo test --features oracle` 8 new tests (6 in `oracle_thin.rs`, 2 in `tests/oracle_request.rs`) pass; `clippy -D warnings` and `fmt --check` clean | measured |

Surprises: (1) the crate sleeps one second after any failed connect and ignores `retry_delay` in an Easy Connect string; (2) it cannot log in to an account with only an 11G verifier, which Python thin can; (3) Oracle 26ai cannot create a 10G verifier at all, so DPY-3015 needs an older server; (4) `cursor.rowcount` is read before the fetch in `dbrun.py`, so a query's count is 0 and the driver reproduces that; (5) `TIMESTAMP WITH TIME ZONE` comes back as UTC without its offset.
