Reconstructed by project:introspec on 2026-10-18 from 384c771

# Data model — dbrun

All files live outside the repository, per user, mode `0700` directories and `0600` files.

| Entity | Where | Fields | Evidence |
|---|---|---|---|
| Profile | `$AI_GENT_DB_CONNECTIONS`, else `${XDG_CONFIG_HOME:-~/.config}/ai-gent/db/connections.toml` then `.env`; plus running compose containers | name, engine ∈ {postgresql, mysql, mariadb, sqlserver, oracle, sqlite}, class ∈ {local, development, staging, production} (default production), host, port, database, service/sid (Oracle), user, password or `pass:` reference, path (SQLite), container, family, source | `plugins/db/scripts/dbrun.py:34-35,64-67,122-256,407-422` |
| Classification | computed per run | class, writable, evidence[], via ∈ {file, host, container}, container_id, pod, pid | `dbrun.py:471-526` |
| Plan | `${XDG_STATE_HOME:-~/.local/state}/ai-gent/db/plans/<YYYYMMDD-HHMMSS-xxxx>.json` | id, profile, engine, reason, created, statements[], expect[], ddl[], backup, sha256, evidence, backup_location (after apply) | `dbrun.py:1297-1352,1477` |
| Backup | `…/ai-gent/db/backups/` (or a server-side path for SQL Server) | engine dump of the database | `dbrun.py:1385-1441` |
| Log entry | `…/ai-gent/db/log/<date>_<repo>.md` | timestamp, action, profile, id, engine, class, writable, via, evidence, reason, SQL, status, result | `dbrun.py:856-889` |

## Lifecycles

Plan: `recorded` → (`apply --confirmed`) → `backed up` → `committed` | `rolled back`; a committed plan with a backup → (`restore --confirmed`) → `restored` [OBSERVED: dbrun.py:1327-1539].

Profile class: declared → classified each run; `local` only on proof, never cached [OBSERVED: dbrun.py:1466 "re-proven now, not trusted from plan time"].

## Invariants

- A plan applies only to the profile it was made for and only with statements matching its hash [OBSERVED: dbrun.py:1371-1382].
- Writes target only user databases [OBSERVED: dbrun.py:1311-1324].
- No credential is written to a plan, a log entry or output [OBSERVED: dbrun.py:838-854].
