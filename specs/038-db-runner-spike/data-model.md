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

Spike artifacts: `engine-matrix.md`, `results.md`, `decision.md` in this directory.
