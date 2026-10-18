Reconstructed by project:introspec on 2026-10-18 from 384c771

# Contract — dbrun CLI

From `build_parser()` [OBSERVED: plugins/db/scripts/dbrun.py:1741-1820]. Exit codes: 0 ok, 1 error, 2 usage, 3 refused, 4 connection [OBSERVED: dbrun.py:32].

| Command | Arguments | Writes to the database | Notes |
|---|---|---|---|
| `profiles` | `[--json] [--export-env [--out FILE]]` | no | never prints credentials; `--export-env` writes a `0600` file with `pass:` references |
| `classify` | `<profile> [--json]` | no | the proof and its evidence |
| `test` | `<profile> [--reason R] [--timeout 30]` | no | connect and a trivial query |
| `query` | `<profile> --sql FILE\|- --reason R [--max-rows 100] [--timeout 30] [--reveal] [--format table\|csv\|json] [--explain] [--allow-full-count]` | no (read-only session) | masked unless `--reveal` (secrets always) |
| `plan` | `<profile> --sql FILE --reason R [--expect N,N,…]` | no | proven local only; prints the plan id |
| `apply` | `<profile> <plan_id> --confirmed [--timeout 120]` | yes | proven local only; backup first |
| `restore` | `<profile> <plan_id> --confirmed` | yes | from the plan's backup |
| `script` | `<profile> --sql FILE --reason R --out FILE` | no, never | a file for a person to run |
| `session` | `start\|stop\|status <profile> [--reason R]` | no | warm client container |
| `secrets` | `migrate [<profile>] [--all]` | no | moves plain-text passwords into pass |
| `log` | `[--tail 5]` | no | the command log |
| `_exec` | (stdin JSON) | per request | internal: the driver child process |

Environment: `AI_GENT_DB_CONFIG`, `AI_GENT_DB_STATE`, `AI_GENT_DB_CONNECTIONS`, `AI_GENT_DB_CLIENT_IMAGE`, `AI_GENT_PASS_PREFIX`, `PASSWORD_STORE_DIR`, `DOCKER_HOST`, `CONTAINER_HOST`, `XDG_CONFIG_HOME`, `XDG_STATE_HOME` [OBSERVED: dbrun.py:44,64-73,151-161,454-458; shared/passstore.py:24-29].
