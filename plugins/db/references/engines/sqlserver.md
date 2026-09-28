# SQL Server

- **Driver:** pymssql (`uv run --with pymssql`). No CLI fallback in v1;
  `sqlcmd` (in the `mssql-tools18` image, or in the server image under
  `/opt/mssql-tools18/bin`) is used only for local backups and restores.
- **Session guards (runner):** login and query timeouts,
  `SET LOCK_TIMEOUT 5000`, `SET NOCOUNT ON`, `SET TRANSACTION ISOLATION
  LEVEL READ COMMITTED`. **There is no read-only transaction**: on remote
  targets the runner's statement classifier is the guard, and a read-only
  login is strongly preferred.
- **Plain plan:** `SET SHOWPLAN_XML ON` → the query → `SET SHOWPLAN_XML
  OFF` (the runner does this for `--explain`; the query is not executed).
  Never `SET STATISTICS PROFILE/XML ON`, which executes.

## Row estimate (statistics — exact, and cheap)

```sql
SELECT s.name AS schema_name, t.name AS table_name,
       SUM(p.row_count) AS row_count,
       SUM(p.reserved_page_count) * 8 * 1024 AS reserved_bytes
FROM sys.dm_db_partition_stats p
JOIN sys.tables t ON t.object_id = p.object_id
JOIN sys.schemas s ON s.schema_id = t.schema_id
WHERE p.index_id IN (0, 1)
GROUP BY s.name, t.name
ORDER BY reserved_bytes DESC
OFFSET 0 ROWS FETCH NEXT 200 ROWS ONLY;
```

Needs `VIEW DATABASE STATE`; without it, fall back to `sys.partitions.rows`.

## Catalog for `db:inspect`

| What | Query source |
|---|---|
| version, settings | `@@VERSION`, `SERVERPROPERTY('Edition')`, `sys.databases` (compatibility level, `is_read_committed_snapshot_on`, recovery model, collation) |
| tables, partitions | `sys.tables`, `sys.partitions`, `sys.partition_schemes` |
| columns | `sys.columns` + `sys.types` (nullable, identity, computed, default via `sys.default_constraints`) |
| PK, FK, unique, check | `sys.key_constraints`, `sys.foreign_keys` + `sys.foreign_key_columns` (`is_disabled`, `is_not_trusted`), `sys.check_constraints` |
| indexes | `sys.indexes` + `sys.index_columns`; unused: `sys.dm_db_index_usage_stats` (since the last restart — `sys.dm_os_sys_info.sqlserver_start_time`); duplicates by key columns; disabled: `is_disabled = 1` |
| views, routines, triggers | `sys.views`, `sys.procedures`, `sys.objects` (functions), `sys.triggers` (incl. `is_disabled`), definitions via `OBJECT_DEFINITION` |
| sequences | `sys.sequences` (`current_value`) |
| grants | `sys.database_permissions`, `sys.database_principals`, role members |
| links | `sys.servers WHERE is_linked = 1`; also `OPENROWSET`/`OPENQUERY` usage in routines |
| stats freshness | `STATS_DATE(object_id, stats_id)` over `sys.stats` |
| legacy/backup tables | `name LIKE '%[_]bak' OR name LIKE '%[_]old' OR name LIKE '%[_]backup' OR name LIKE '%[_]tmp'` or a date suffix |

## Local writes

- DDL and DML are transactional (most DDL); the runner wraps the plan in
  one transaction.
- **Never in a system database** (`master`, `model`, `msdb`, `tempdb`):
  `master` can't go single-user, so it can't be restored. The official
  image has no variable for a user database, and the auto-discovered
  `local:<service>` profile connects to `master` — so the runner refuses
  plans there. Create the project's database (the app's migrations or an
  init step) and add a profile naming it: `<P>_CLASS=local`,
  `<P>_CONTAINER=<container>`, `<P>_DATABASE=<db>`.
- **Backup:** `BACKUP DATABASE [<db>] TO DISK =
  '/var/opt/mssql/data/ai-gent-<plan-id>.bak' WITH INIT, COPY_ONLY`,
  kept inside the container. **Restore:** single-user mode, `RESTORE
  DATABASE … WITH REPLACE`, multi-user — the whole database, so objects
  the plan created go too.

## Gotchas

- A long read under `READ COMMITTED` (without RCSI) takes shared locks
  and can block writers: keep reads short, filtered and timed out.
- `NOLOCK` / `READ UNCOMMITTED` avoids blocking but reads uncommitted and
  inconsistent data — never use it for evidence in an investigation.
- `SELECT … INTO` creates a table; `EXEC` / `sp_*` / `xp_*` are refused on
  remote targets.
- Table hints that lock (`UPDLOCK`, `XLOCK`, `HOLDLOCK`, `TABLOCK(X)`,
  `PAGLOCK`, `SERIALIZABLE`, `REPEATABLEREAD`) and `NEXT VALUE FOR` are
  writes to the runner. SQL Server has no read-only session, so on
  remote targets the classifier is the barrier (reads also run in a
  transaction that is rolled back).
- `SET NOCOUNT ON` hides row counts from the driver; the runner leaves it
  off so `apply` can check them.
- `is_not_trusted = 1` foreign keys aren't enforced for existing rows.
