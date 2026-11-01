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
