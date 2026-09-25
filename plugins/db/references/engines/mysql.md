# MySQL and MariaDB

- **Driver:** PyMySQL (`uv run --with pymysql`), which serves both. CLI
  fallback: `mysql --batch` / `mariadb --batch` from the official image,
  password through `MYSQL_PWD` in the container's environment, never
  argv.
- **Session guards (runner):** `SET SESSION TRANSACTION READ ONLY`
  (remote), `MAX_EXECUTION_TIME` (MySQL, ms, `SELECT` only) or
  `max_statement_time` (MariaDB, seconds), `lock_wait_timeout = 5`,
  `innodb_lock_wait_timeout = 5`, connect and read timeouts.
- **Plain plan:** `EXPLAIN <query>` (or `EXPLAIN FORMAT=TREE` on MySQL
  8). Never `EXPLAIN ANALYZE` (MySQL 8.0.18+, it executes) or `ANALYZE
  <query>` (MariaDB, it executes).

## Row estimate (statistics)

```sql
SELECT table_schema, table_name, engine,
       table_rows AS estimated_rows,         -- InnoDB: an estimate, can be off by 40-50 %
       data_length + index_length AS total_bytes,
       update_time
FROM information_schema.tables
WHERE table_type = 'BASE TABLE'
  AND table_schema NOT IN ('mysql', 'sys', 'information_schema', 'performance_schema')
ORDER BY total_bytes DESC
LIMIT 200;
```

## Catalog for `db:inspect`

| What | Query source |
|---|---|
| version, settings | `SELECT VERSION()`; `@@sql_mode`, `@@time_zone`, `@@transaction_isolation`, `@@character_set_server`, `@@lower_case_table_names` |
| tables, partitions | `information_schema.tables`, `information_schema.partitions` |
| columns | `information_schema.columns` (type, nullable, default, `extra`, charset/collation) |
| PK, FK, unique, check | `information_schema.table_constraints`, `key_column_usage`, `referential_constraints` (rules), `check_constraints` (MySQL 8.0.16+ / MariaDB 10.2+) |
| indexes | `information_schema.statistics`; unused: `sys.schema_unused_indexes`; duplicates: `sys.schema_redundant_indexes` (MySQL `sys` schema) |
| views, routines, triggers, events | `information_schema.views`, `routines`, `triggers`, `events` |
| grants | `information_schema.user_privileges`, `schema_privileges`, `table_privileges` (never `mysql.user` password columns) |
| links | tables with `engine = 'FEDERATED'` (and `CONNECT` on MariaDB) |
| stats freshness | `mysql.innodb_table_stats.last_update` (if readable) |
| legacy/backup tables | `table_name REGEXP '(_bak|_backup|_old|_tmp|_copy|_[0-9]{6,8})$'` |
| MyISAM leftovers | `engine <> 'InnoDB'` — no transactions, no FKs |

Schema export: `mysqldump --no-data --skip-add-drop-table --routines
--triggers --events` (`--single-transaction`).

## Local writes

- **DDL commits implicitly** — it escapes the transaction and the
  read-only setting. A plan with DDL must have a backup; the runner
  refuses it otherwise.
- **Backup:** `mysqldump --single-transaction --routines --triggers
  --events` streamed out of the container. **Restore:** load the dump
  with the client inside the container.
- DML on InnoDB runs in one transaction; MyISAM tables can't roll back —
  the plan lists them.

## Gotchas

- `SELECT … INTO OUTFILE / @var` writes; `SELECT … FOR UPDATE` and
  `LOCK IN SHARE MODE` lock.
- `sql_mode` differences explain "works here, fails there" data (zero
  dates, silent truncation when strict mode is off).
- `table_rows` for InnoDB is an estimate; `COUNT(*)` scans an index.
