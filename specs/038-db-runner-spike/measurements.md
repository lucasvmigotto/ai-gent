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
