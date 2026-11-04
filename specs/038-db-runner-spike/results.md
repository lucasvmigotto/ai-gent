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
