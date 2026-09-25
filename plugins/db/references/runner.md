# The `dbrun` runner

`../scripts/dbrun.py` is the **only** way a `db:*` skill touches a
database. It enforces `safety.md` mechanically: it classifies the target,
proves "local", refuses anything but reads on remote targets, applies
timeouts, row caps and masking, and writes the command log before a
statement runs. Never call a database client (`psql`, `mysql`, `sqlcmd`,
`sqlplus`, `sqlite3`, …) directly — the `db` plugin's guard hook blocks it
in Claude Code.

Run it with Python 3 (stdlib only on the host; database drivers are
fetched on demand with `uv`):

```bash
DBRUN="python3 <plugin>/scripts/dbrun.py"     # <plugin> = this plugin's directory
```

## Commands

| Command | What it does | Allowed on |
|---|---|---|
| `profiles [--json]` | list connection profiles: name, engine, class, source — never credentials | — |
| `classify PROFILE [--json]` | run the local proof and print the class with its evidence (a SQLite file is local only inside this repository) | any |
| `test PROFILE` | connect and run the engine's trivial query; prints SUCCESS or FAILED and the reason | any |
| `query PROFILE --sql FILE\|- --reason TEXT [--max-rows N] [--timeout S] [--reveal] [--format table\|csv\|json] [--explain] [--allow-full-count]` | run read-only statements; on remote targets it also refuses `SELECT *` and unfiltered `COUNT` on user tables (`--allow-full-count` only when the user asked for an exact full count) | any |
| `plan PROFILE --sql FILE --reason TEXT [--expect N,N,…]` | record a write plan (statements, expected row counts, backup and restore method) and print it with its id. Get each expected count first with a `query` using the statement's own `WHERE` (`SELECT COUNT(*) … WHERE <same condition>`); leave a position blank for DDL (`--expect 2,`) | proven local only |
| `apply PROFILE PLAN_ID --confirmed` | re-prove local, back up, run the plan in a transaction, check the row counts, commit only on a match | proven local only |
| `restore PROFILE PLAN_ID --confirmed` | restore the backup taken by `apply` | proven local only |
| `script PROFILE --sql FILE --reason TEXT --out PATH` | write a reviewed change script (header, statements, verification and rollback sections) for **a person** to run — never executes it; refuses an existing `--out` file | any (the way to change a remote database) |
| `session start\|stop\|status PROFILE` | keep a warm client container for an investigation, so repeated queries start fast | any |
| `log [--tail N]` | print the command log's path and its last entries | — |

`--reason` is mandatory for everything that touches a database; it goes
into the log. `--confirmed` means the user confirmed *this* plan in the
conversation — never pass it on your own.

## Defaults

- `--max-rows 100`, `--timeout 30` (seconds, per statement); lock waits
  time out after 5 seconds.
- Output is masked (`safety.md` § Masking). `--reveal` shows values
  unmasked and is logged; pass it only when the user asked to see them.
- `--explain` runs the engine's plain plan (never `EXPLAIN ANALYZE`).
- Error messages and log entries have the profile's host, user and
  password replaced with `<host>`, `<user>` and `<password>` — driver
  errors often quote them.

## Exit codes

`0` success · `1` error · `2` usage · `3` refused by policy (the message
says which rule) · `4` connection failed.

## Where things live

| Path | Content |
|---|---|
| `${AI_GENT_DB_CONFIG:-~/.config/ai-gent/db}/connections.env` | the user's profiles (`connections.md`), mode 600 |
| `${AI_GENT_DB_STATE:-~/.local/state/ai-gent/db}/log/` | the command log, one file per day and project |
| `…/state/…/plans/`, `…/backups/` | write plans and the backups `apply` takes |
| `…/state/…/exports/` | schema exports from production or staging, written by the skill from `query` output (`dbrun` itself never exports) |

## Backends, cheapest reliable first

1. **Proven local container:** the driver runs in a client container
   that joins the database container's network namespace
   (`--network container:<db>`), so `127.0.0.1` *is* that database —
   local by construction, and it works rootless under Podman.
2. **Remote:** the driver runs on the host through `uv run --with
   <driver>`; without `uv`, in the client container on the default
   network.
3. **Engine tools in the local container** — `pg_dump`/`pg_restore`,
   `mysqldump`/`mysql`, SQL Server `BACKUP`/`RESTORE` — used by `apply`
   and `restore` for backups, through `exec` into the proven local
   container with the password on stdin.
4. **Last resort:** a new backend added to `dbrun` for the engine (never a
   separate script), shown to the user before it's used, so the same
   classification, statement checks, masking and log apply.
