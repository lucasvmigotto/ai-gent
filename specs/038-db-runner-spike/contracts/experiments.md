# Experiments (each rule is fixed before it runs)

All prototypes live in `rust/spikes/dbrun/` and use only throwaway containers (images and memory caps as in `scripts/test-dbrun-engines.py`) or a temp sqlite file. Fixture password: the engines script's throwaway value only.

| # | Question | Method | Pass | Fail |
|---|---|---|---|---|
| E1 | Does the safety core port with no behaviour change? | Port `scan`, `split_statements`, `classify_statement`, `is_ddl`, `catalog_only`, `cost_rules`, `mask_*`, `luhn`, `redact`, `network_args`, `parse_modes` to a driver-free library; turn the cases of `ClassifierTest` (8), `CostTest` (2), `MaskTest` (3), `RedactTest` (2), `NetworkTest` (1) into fixtures | all fixture cases identical on both implementations | any difference not explained by a Python bug (then the bug is recorded and fixed first) |
| E2 | PostgreSQL native | prototype with a Rust PostgreSQL client against `postgres:17`: connect, session options (`statement_timeout`, `lock_timeout`, `idle_in_transaction_session_timeout`, read-only), run the engines script checks through a thin adapter | all engine checks incl. a write refused by the server in read mode | any check fails or needs a native library |
| E3 | MySQL and MariaDB native | same with `mysql:8.4` and `mariadb:11.4`; per-session timeout variable chosen after `SELECT VERSION()` | all checks on both | a timeout cannot be enforced, or MariaDB and MySQL need different clients |
| E4 | SQLite native | `rusqlite` or equivalent with a bundled build; the `SqliteEndToEndTest` cases | 8 cases pass; no system libsqlite needed | needs a system library |
| E5 | SQL Server | survey maintained Rust drivers; prototype the best against `mssql/server:2022-latest`; also try `sqlcmd` in a client image as the fallback | one route passes all checks incl. TLS to the 2022 image | none passes: container route only |
| E6 | Oracle | same with `gvenzl/oracle-free:23-slim`; thin versus requires-Instant-Client matters | a route passes all checks without shipping Oracle client libraries | needs Instant Client: container route only |
| E7 | Windows | run E1 and E4 on a Windows runner; the client-container network namespace join on Docker Desktop or a Podman machine | E1, E4 pass; the join works or is documented unsupported | E1 or E4 fail |
| E8 | Size and build cost | release binary size and build time with each driver set (N, H, C) on Linux x86_64 and aarch64 | numbers recorded; H or N within a size budget the owner sets (starting figure: +10 MB over the no-db binary, arbitrary, the owner may change it) | not measured |

Results go in `specs/038-db-runner-spike/results.md` (one section per experiment: date, machine, commands, outcome), then are summarised in `decision.md`.
