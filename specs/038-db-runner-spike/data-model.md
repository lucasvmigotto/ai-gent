# Data Model: dbrun spike

The spike produces documents, not data. For reference, the shapes the runner uses today (unchanged by the spike):

| Entity | Shape | Source |
|---|---|---|
| Profile | `engine` (postgresql, mysql, mariadb, sqlserver, oracle, sqlite), `class` (production, staging, development, local), connection facts (`host`, `port`, `database`, `user`, `password` or a `pass:` reference, `path` for sqlite, `container`, Oracle `service`/`sid`), `source` | profile files in `$AI_GENT_DB_CONFIG` (default `~/.config/ai-gent/db`), plus local profiles discovered from containers |
| Classification | `{class, writable, evidence[], via: host\|container\|file}` | `classify` |
| Driver request (stdin of `_exec`) | `{engine, conn{...}, mode: read\|write, statements[], timeout, max_rows, ...}` | `base_request` |
| Plan | id, profile, statements, expected counts, reason, digest for the tamper check | `plans_dir()` |
| Log entry | start line before the statement, result line after, secrets redacted | `log_path()` |

Environment: `AI_GENT_DB_CONFIG`, `AI_GENT_DB_STATE`, `AI_GENT_DB_CLIENT_IMAGE`, `AI_GENT_ORACLE_CLIENT_LIB`, `CONTAINER_ENGINE`, `AI_GENT_PASS_PREFIX`.

Spike artifacts: `engine-matrix.md`, `results.md`, `decision.md` and the measurement log `measurements.md` in this directory.

After a non-S verdict (D28), two more shapes matter:

| Entity | Shape | Source |
|---|---|---|
| Connector | a trait in `ai-gent-dbrun`: takes the driver request above (engine, connection facts, mode, statements, timeout, row cap) and returns the one-line JSON result; implemented by the process connector (drives `dbrun.py _exec`) and by one module per engine behind the features `sqlite`, `postgres`, `mysql`, `mssql`, `oracle` | the Python `_exec` contract |
| Crate set | `ai-gent-core` (lean, no driver), `ai-gent-dbrun` (independent SemVer, published after core), `ai-gent` (lockstep with the tag, published last) | D23, D28 |
