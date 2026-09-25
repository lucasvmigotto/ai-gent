# SQLite

- **Driver:** Python's standard `sqlite3` module — nothing to install.
  CLI fallback: `sqlite3 -csv`.
- **Session guards (runner):** remote-class files open with
  `file:<path>?mode=ro` (URI) plus `PRAGMA query_only = ON`; a busy
  timeout of 5 seconds. Local files (inside this repository or the
  runner's state directory) open read-write only for `apply`.
- **Plain plan:** `EXPLAIN QUERY PLAN <query>` (`EXPLAIN` alone prints
  bytecode).

## Row estimate

SQLite keeps no row counts. `sqlite_stat1` (after `ANALYZE`) holds
approximate counts per index; otherwise `SELECT COUNT(*)` on a filtered
indexed range, or the file size as a rough scale. SQLite files are small
enough that a full `COUNT(*)` is usually fine locally — still ask for a
remote-class file over a few hundred MB.

## Catalog for `db:inspect`

| What | Query source |
|---|---|
| version, settings | `sqlite_version()`; `PRAGMA foreign_keys`, `journal_mode`, `page_size`, `page_count` |
| tables, views, triggers, indexes | `sqlite_schema` (`type`, `name`, `tbl_name`, `sql`) |
| columns | `PRAGMA table_xinfo('<table>')` (includes generated columns) |
| PK, FK, unique | `PRAGMA table_info` (`pk`), `PRAGMA foreign_key_list('<table>')`, `PRAGMA index_list` (`unique`, `origin` u/pk) |
| check constraints | only inside `sqlite_schema.sql` |
| integrity | `PRAGMA foreign_key_check` (lists orphans), `PRAGMA integrity_check` (slow on big files) |
| attached databases | `PRAGMA database_list` — any file outside the repository makes the target remote |
| legacy/backup tables | name suffixes `_bak`, `_old`, `_backup`, `_tmp`, dates |

Schema export: the `sql` column of `sqlite_schema` (or `sqlite3 .schema`).

## Local writes

- DDL and DML are transactional.
- **Backup:** the online backup API (`Connection.backup`) to
  `…/backups/<plan-id>.sqlite`. **Restore:** copy it back with the file
  closed.

## Gotchas

- `PRAGMA foreign_keys` is **off by default** per connection: foreign
  keys may be declared and never enforced — `foreign_key_check` shows the
  damage.
- Type affinity: a column declared `INTEGER` can hold text; check
  `typeof(col)` distributions when data looks wrong.
- WAL mode leaves `-wal` / `-shm` files next to the database: back up
  through the API, not by copying the main file alone.
