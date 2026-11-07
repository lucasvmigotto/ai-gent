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
