# Engine matrix: Rust drivers for the dbrun spike (T002)

One row per engine for the driver survey of `specs/038-db-runner-spike/tasks.md` task T002, feeding E2-E6 and the verdict in `specs/038-db-runner-spike/decision.md`. The rows start from the research table and the engine x route table of `docs/product/dbrun-spike-report.md` and re-state each candidate across the fields T002 lists: crate, version, maintainer activity, native library needs, TLS, timeouts, verdict-relevant risks.

Claims are labelled per `shared/research.md`. `[OBSERVED]` claims are sourced in this run and carry a source URL or a repo-relative source. `[INFERRED]` claims follow from the spike documents or from the wrapper crates without being measured here. `[ASSUMED]` marks the report's unverified starting points that task T002 set out to close.

## The matrix

| Engine | Crate | Version | Maintainer activity | Native library needs | TLS | Timeouts | Verdict-relevant risks |
|---|---|---|---|---|---|---|---|
| SQLite | `rusqlite` [OBSERVED] — https://crates.io/crates/rusqlite/0.41.1 | 0.41.1 [OBSERVED] | gwenn and the `rusqlite` org; last publish 2026-10-28 [OBSERVED] — https://github.com/rusqlite/rusqlite | none at runtime; `bundled` compiles SQLite via a C compiler [OBSERVED] — https://crates.io/crates/rusqlite/0.41.1 | n/a: SQLite has no transport layer [INFERRED] | n/a: rusqlite exposes `busy_timeout`, no statement-level timeout [INFERRED] | build toolchain needs a C compiler for `bundled`; read and server-side write refusal via `mode=ro` + `query_only` measured (cold start 48-65 ms) [OBSERVED] — docs/product/dbrun-spike-report.md |
| PostgreSQL | `tokio-postgres` 0.7.18 (async) and `postgres` 0.19.14 (sync) [OBSERVED] | tokio-postgres 0.7.18; postgres 0.19.14 [OBSERVED] | sfackler and paolobarbolini; last release 2026-06-12 [OBSERVED] — https://github.com/rust-postgres/rust-postgres | none, pure Rust [OBSERVED] | external libraries: `NoTls`, `postgres-native-tls`, `postgres-openssl`; no rustls feature in tokio-postgres; rustls via third-party `tokio-postgres-rustls` 0.14.0 [OBSERVED] — https://docs.rs/tokio-postgres/0.7.18/tokio_postgres/, https://crates.io/crates/tokio-postgres-rustls/0.14.0 | session timeouts via connect options; server-side write refusal via `default_transaction_read_only=on` measured [OBSERVED] — docs/product/dbrun-spike-report.md | TLS backend not chosen yet; type-mapping risk on the native route [INFERRED] |
| MySQL / MariaDB | `mysql` 28.0.4 and `mysql_async` 0.37.1; repos https://github.com/blackbeam/rust-mysql-simple and https://github.com/blackbeam/mysql_async [OBSERVED] | mysql 28.0.4; mysql_async 0.37.1 [OBSERVED] | blackbeam (Anatoly I.); mysql 28.0.4 published 2026-10-16 [OBSERVED] | none, pure Rust [OBSERVED] — https://crates.io/crates/mysql/28.0.4 | mysql 28.0.4 declares `native-tls`, `rustls`, `rustls-tls`, `rustls-tls-ring`; mysql_async 0.37.1 declares rustls/aws-lc-rs [OBSERVED] — https://crates.io/crates/mysql/28.0.4; closes the report's assumed-TLS item | per-session timeout variable after `SELECT VERSION()`: `max_execution_time` (MySQL) vs `max_statement_time` (MariaDB) [INFERRED] — specs/038-db-runner-spike/contracts/experiments.md | one client binary for MySQL and MariaDB; write refusal ERROR 1792 measured (cold start 160 ms) [OBSERVED] — docs/product/dbrun-spike-report.md |
| SQL Server | `tiberius` 0.13.0 and fork `tiberius-ng` 0.13.1 [OBSERVED] | tiberius 0.13.0; tiberius-ng 0.13.1 [OBSERVED] | upstream `prisma/tiberius` is not archived and was last pushed 2026-09-25 [OBSERVED] — https://github.com/prisma/tiberius; RustSec RUSTSEC-2020-0010 (unmaintained) covers 0.3.x only and lists versions above 0.3.2 as unaffected [OBSERVED]; fork `tiberius-ng` last pushed 2026-10-03, one maintainer [OBSERVED] — https://github.com/MattJackson/tiberius-ng — https://raw.githubusercontent.com/rustsec/advisory-db/main/crates/tiberius/RUSTSEC-2020-0010.md | none for the TDS protocol; rustls backend or native-tls [OBSERVED] | rustls via fork feature or native-tls; read over TLS on `mssql/server:2022-latest` measured (69 ms), encryption required, `trust_cert` for the test image [OBSERVED] — docs/product/dbrun-spike-report.md | not established; driver timeout not exercised [INFERRED] | fork health (one maintainer); the upstream `tiberius` 0.13 line is maintained at a slow pace, the RustSec notice does not apply to it [OBSERVED]; no server-side read-only mode on SQL Server [OBSERVED] — specs/038-db-runner-spike/spec.md; `trust_cert` throwaway-only [OBSERVED] |
| Oracle | `oracle` 0.6.3 (ODPI-C) and `oracledb` 26.0.0-beta.5 (pure Rust thin) [OBSERVED] | oracle 0.6.3; oracledb 26.0.0-beta.5 [OBSERVED] | oracle crate last publish 2025-01-02; oracledb updated 2026-10-28 but beta [OBSERVED] | oracle crate needs Oracle Client libraries 11.2+; oracledb needs none [OBSERVED] | oracledb thin-mode TLS and legacy 10G verifier not established (beta) [INFERRED] | not established; E6 not run [INFERRED] | experiment not run; Instant Client not shippable; oracledb beta [OBSERVED] |
| Process fallback | no Rust crate; process connector spawns `dbrun.py _exec` with JSON on stdin and one JSON line back [OBSERVED] | n/a, helper pin [OBSERVED] — docs/product/dbrun-spike-report.md | n/a, no Rust crate, helper in the db plugin [OBSERVED] | none, Python driver child process [OBSERVED] | uv-managed in the child (`uv run --with <pin>`) or TLS inside the digest-pinned client container [OBSERVED] — specs/038-db-runner-spike/spec.md | enforced by the Python helper [OBSERVED] | db requires uv or a container engine today; plain install cannot run db [OBSERVED] — specs/038-db-runner-spike/spec.md |

## Closed unverified items

### The `mysql` repository 404

The report read the `mysql` repository page, got a 404 and therefore treated the crate's TLS options as assumed. Re-checked this run: crates.io indexes `mysql` 28.0.4 at https://github.com/blackbeam/rust-mysql-simple and `mysql_async` 0.37.1 at https://github.com/blackbeam/mysql_async, both of which exist today, and the stale path https://github.com/grayblake/rust-mysql answers 404 now. The `mysql` 28.0.4 crate declares the TLS features `native-tls`, `rustls`, `rustls-tls` and `rustls-tls-ring`; `mysql_async` 0.37.1 declares the rustls/aws-lc-rs feature.

Closed: the repository 404 was a stale path, and TLS is an explicit crate feature, not an assumption.

### rustls for tokio-postgres

The report carried rustls for `tokio-postgres` as a third-party search snippet only. Re-checked this run: the docs.rs page for `tokio-postgres` 0.7.18 states that TLS support is implemented through external libraries, with `NoTls`, `postgres-native-tls` and `postgres-openssl`; the crate's own feature set contains no rustls feature; the third-party crate `tokio-postgres-rustls` 0.14.0 supplies rustls with the `aws-lc-rs`, `ring`, `webpki-roots` and `native-certs` features.

Closed: rustls is available for `tokio-postgres` through the third-party `tokio-postgres-rustls` crate, not bundled upstream.

### The RustSec claim on `tiberius`

The report recorded, with doubt, that a fork author called `tiberius` effectively unmaintained and raised open RustSec TLS advisories. Re-checked this run: the RustSec advisory database holds one advisory for `tiberius`, RUSTSEC-2020-0010, dated 2020-02-28, marked informational and unmaintained, with no patched version and unaffected versions above 0.3.2, and the advisory text says the author archived the repository and did not intend to maintain the 0.3 version relying on the old futures ecosystem.

Closed (corrected by the supervisor): the unmaintained claim is **not** true for the current line. RUSTSEC-2020-0010 covers 0.3.x; `prisma/tiberius` is not archived and was pushed on 2026-09-25 (https://github.com/prisma/tiberius). The fork `tiberius-ng` 0.13.1 (pushed 2026-10-03) remains the candidate the spike measured with rustls; whether the upstream 0.13 line offers rustls too is still to be confirmed.

## Sources

| # | Claim | URL | Retrieved | Confidence | Label |
|---|---|---|---|---|---|
| 1 | `rusqlite` 0.41.1 and the `bundled` feature | https://crates.io/crates/rusqlite/0.41.1 | 2026-11-02 | High | [OBSERVED] |
| 2 | `tokio-postgres` 0.7.18 TLS via external libraries, no rustls feature | https://docs.rs/tokio-postgres/0.7.18/tokio_postgres/ | 2026-11-02 | High | [OBSERVED] |
| 3 | `tokio-postgres-rustls` 0.14.0 | https://crates.io/crates/tokio-postgres-rustls/0.14.0 | 2026-11-02 | Medium | [OBSERVED] |
| 4 | sync `postgres` crate 0.19.14 | https://crates.io/crates/postgres/0.19.14 | 2026-11-02 | High | [OBSERVED] |
| 5 | `mysql` 28.0.4 and TLS features, blackbeam repositories | https://crates.io/crates/mysql/28.0.4, https://github.com/blackbeam/rust-mysql-simple, https://github.com/blackbeam/mysql_async | 2026-11-02 | High | [OBSERVED] |
| 6 | stale `grayblake/rust-mysql` repository path | https://github.com/grayblake/rust-mysql | 2026-11-02 | High | [OBSERVED] |
| 7 | `tiberius`, `tiberius-ng`, RustSec RUSTSEC-2020-0010 | https://crates.io/crates/tiberius/0.13.0, https://crates.io/crates/tiberius-ng/0.13.1, https://raw.githubusercontent.com/rustsec/advisory-db/main/crates/tiberius/RUSTSEC-2020-0010.md | 2026-11-02 | High | [OBSERVED] |
| 8 | `oracledb` 26.0.0-beta.5 and `oracle` 0.6.3 | https://crates.io/crates/oracledb/26.0.0-beta.5, https://crates.io/crates/oracle/0.6.3 | 2026-11-02 | High | [OBSERVED] |
| 9 | spike report measurements | docs/product/dbrun-spike-report.md | 2026-11-02 | High | [OBSERVED] |
