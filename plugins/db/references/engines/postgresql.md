# PostgreSQL

- **Driver:** psycopg 3 (`uv run --with 'psycopg[binary]'`). CLI
  fallback: `psql --csv` from the official `postgres` image.
- **Session guards (runner):** `default_transaction_read_only = on`
  (remote), `statement_timeout`, `lock_timeout = 5s`,
  `idle_in_transaction_session_timeout`, `connect_timeout`,
  `application_name = 'ai-gent-db'`.
- **Plain plan:** `EXPLAIN (FORMAT TEXT) <query>` — never `ANALYZE`,
  `BUFFERS` with `ANALYZE`, or `auto_explain` changes.

## Row estimate (statistics)

```sql
SELECT n.nspname AS schema, c.relname AS table_name,
       c.reltuples::bigint AS estimated_rows,           -- -1 = never analyzed
       pg_total_relation_size(c.oid) AS total_bytes,
       s.last_analyze, s.last_autoanalyze
FROM pg_class c
JOIN pg_namespace n ON n.oid = c.relnamespace
LEFT JOIN pg_stat_user_tables s ON s.relid = c.oid
WHERE c.relkind IN ('r', 'p') AND n.nspname NOT IN ('pg_catalog', 'information_schema')
ORDER BY total_bytes DESC
LIMIT 200;
```

## Catalog for `db:inspect`

| What | Query source |
|---|---|
| version, settings | `SELECT version()`; `pg_settings` (name, setting) for `max_connections`, `work_mem`, `shared_buffers`, `timezone`, `default_transaction_isolation` |
| schemas, tables, partitions | `pg_namespace`, `pg_class` (`relkind` `r`/`p`/`v`/`m`/`f`), `pg_inherits` / `pg_partitioned_table` |
| columns | `information_schema.columns` (type, nullable, default, identity/generated) |
| PK, FK, unique, check | `pg_constraint` (`contype` `p`/`f`/`u`/`c`) with `pg_get_constraintdef(oid)`; FK actions from `confupdtype` / `confdeltype` |
| indexes | `pg_index` + `pg_get_indexdef(indexrelid)`; invalid: `NOT indisvalid`; unused: `pg_stat_user_indexes.idx_scan = 0` (since stats reset — `pg_stat_database.stats_reset`); duplicates: same `indrelid` and `indkey` |
| FKs without an index | FK columns not leading any index on the referencing table |
| views, materialized views | `pg_views`, `pg_matviews` (definition) |
| routines, triggers | `pg_proc` (`prokind`), `pg_trigger` (not `tgisinternal`) with `pg_get_triggerdef` |
| sequences | `pg_sequences` (`last_value`, `max_value`) — read the view, never call `nextval` |
| grants, roles | `information_schema.role_table_grants`, `pg_roles` (no password columns) |
| extensions, links | `pg_extension`; `pg_foreign_server`, `pg_user_mappings` (no options — they can hold passwords) |
| stats freshness | `pg_stat_user_tables` (`last_analyze`, `n_dead_tup`) |
| legacy/backup tables | `relname ~* '(_bak|_backup|_old|_tmp|_copy|_\d{6,8})$'` |

Schema export: `pg_dump --schema-only --no-owner --no-privileges` (local
via `exec` in the container; remote through the client image, read-only
account).

## Local writes

- DDL and DML are transactional: the runner wraps the plan in one
  transaction.
- **Backup:** `pg_dump -Fc` streamed out of the container to
  `…/backups/<plan-id>.dump`. **Restore:** `pg_restore --clean
  --if-exists --no-owner` into the same database.

## Gotchas

- `EXPLAIN ANALYZE` executes the statement, writes included.
- `SELECT … INTO new_table` creates a table; `nextval()` advances a
  sequence even in a rolled-back transaction.
- A data-modifying CTE (`WITH d AS (DELETE …) SELECT …`) is a write.
- `reltuples = -1` means never analyzed: say so instead of reporting 0.
