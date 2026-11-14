#!/usr/bin/env python3
"""Generates rust/fixtures/db-core/golden-writes.json from the ORIGINAL plugins/db/scripts/dbrun.py (spec 043 T006, T009).

The 038 golden corpus came from the strings of test_dbrun.py and was read-heavy. This one is the write side: statements
that change data, schema, sessions or the server, with obfuscations (comments inside words, mixed case, literals that
hide keywords), every engine, plus the system-database matrix, the change script text, and the plan record, the text and
the command-log entry the original writes. Run: python3 rust/fixtures/db-core/gen_writes.py (no network, no database,
no credentials; a throwaway state directory per case).
"""
import contextlib
import datetime as dt
import io
import json
import re
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.dont_write_bytecode = True
sys.path.insert(0, str(ROOT / "plugins/db/scripts"))
import dbrun  # noqa: E402

WRITES = [
    "INSERT INTO t VALUES (1)", "insert into t select * from s", "REPLACE INTO t VALUES (1)", "UPSERT INTO t VALUES (1)",
    "INSERT INTO t VALUES (1) ON CONFLICT DO NOTHING", "INSERT INTO t VALUES (1) RETURNING id",
    "UPDATE t SET a = 1", "UPDATE t SET a = 1 WHERE id = 2 RETURNING id", "update t set a='x;y' where id=1",
    "DELETE FROM t", "DELETE FROM t WHERE id IN (SELECT id FROM s)", "DELETE\nFROM t", "DeLeTe FrOm t",
    "DEL/**/ETE FROM t", "/* a */ DELETE /* b */ FROM t", "-- c\nDELETE FROM t",
    "MERGE INTO t USING s ON (t.id = s.id) WHEN MATCHED THEN UPDATE SET a = 1",
    "WITH d AS (DELETE FROM t RETURNING id) SELECT id FROM d", "WITH u AS (UPDATE t SET a = 1 RETURNING id) SELECT * FROM u",
    "WITH i AS (INSERT INTO t VALUES (1) RETURNING id) SELECT id FROM i", "WITH x AS (SELECT 1) DELETE FROM t",
    "CREATE TABLE t (id int)", "CREATE INDEX i ON t (a)", "CREATE OR REPLACE VIEW v AS SELECT 1", "CREATE TEMP TABLE t AS SELECT 1",
    "ALTER TABLE t ADD c int", "DROP TABLE t", "DROP DATABASE d", "DROP TABLE IF EXISTS t CASCADE", "TRUNCATE TABLE t", "TRUNCATE t",
    "RENAME TABLE a TO b", "COMMENT ON TABLE t IS 'x'", "GRANT ALL ON t TO u", "REVOKE ALL ON t FROM u",
    "SELECT * INTO t2 FROM t", "SELECT id INTO #tmp FROM t", "SELECT id INTO OUTFILE '/tmp/x' FROM t", "SELECT 1 INTO @v",
    "SELECT id FROM t FOR UPDATE", "SELECT id FROM t FOR SHARE", "SELECT id FROM t FOR NO KEY UPDATE SKIP LOCKED",
    "SELECT id FROM t FOR KEY SHARE", "SELECT id FROM t LOCK IN SHARE MODE",
    "SELECT id FROM t WITH (UPDLOCK)", "SELECT id FROM t WITH (XLOCK, HOLDLOCK)", "SELECT id FROM t WITH (TABLOCKX)",
    "SELECT id FROM t WITH (PAGLOCK)", "SELECT id FROM t WITH (SERIALIZABLE)",
    "SELECT nextval('s')", "SELECT setval('s', 1)", "SELECT NEXT VALUE FOR s", "SELECT s.NEXTVAL FROM DUAL", "SELECT s . nextval FROM dual",
    "SELECT pg_sleep(1)", "SELECT pg_sleep_for('1s')", "SELECT pg_advisory_lock(1)", "SELECT pg_try_advisory_lock(1)",
    "SELECT pg_terminate_backend(1)", "SELECT pg_cancel_backend(1)", "SELECT pg_reload_conf()", "SELECT pg_rotate_logfile()",
    "SELECT pg_read_file('/etc/passwd')", "SELECT pg_read_binary_file('/x')", "SELECT pg_ls_dir('/')", "SELECT pg_stat_file('/x')",
    "SELECT set_config('a','b',false)", "SELECT lo_import('/x')", "SELECT lo_export(1,'/x')", "SELECT lo_unlink(1)",
    "SELECT dblink('c','select 1')", "SELECT dblink_exec('c','delete from t')", "SELECT txid_current()",
    "SELECT sleep(1)", "SELECT benchmark(1000000, md5('a'))", "SELECT get_lock('a', 1)", "SELECT release_lock('a')",
    "SELECT release_all_locks()", "SELECT load_file('/etc/passwd')",
    "SELECT xp_cmdshell('dir')", "SELECT sp_who()", "SELECT * FROM OPENROWSET('SQLNCLI','x','select 1')",
    "SELECT * FROM OPENQUERY(s, 'select 1')", "SELECT * FROM OPENDATASOURCE('a','b').d.dbo.t", "WAITFOR DELAY '00:00:10'",
    "SELECT UTL_HTTP.REQUEST('http://x') FROM DUAL", "SELECT UTL_FILE.FOPEN('a','b','w') FROM DUAL", "SELECT DBMS_LOCK.SLEEP(1) FROM DUAL",
    "SELECT DBMS_OUTPUT.PUT_LINE('x') FROM DUAL", "SELECT DBMS_SCHEDULER.CREATE_JOB(1) FROM DUAL", "SELECT HTP.P('x') FROM DUAL",
    "SELECT OWA_UTIL.WHO_CALLED_ME FROM DUAL", "SELECT sys_exec('id')", "SELECT sys_eval('id')", "SELECT load_extension('x')",
    "SELECT writefile('/x','y')", "SELECT readfile('/x')",
    "EXPLAIN ANALYZE SELECT 1", "EXPLAIN ANALYSE SELECT 1", "EXPLAIN (ANALYZE, BUFFERS) SELECT 1", "EXPLAIN PLAN FOR SELECT 1 FROM DUAL",
    "CALL p()", "EXEC sp_who", "EXECUTE p", "DO $$ BEGIN DELETE FROM t; END $$", "BEGIN", "COMMIT", "ROLLBACK", "SAVEPOINT a", "START TRANSACTION",
    "SET search_path = x", "SET @a = 1", "RESET ALL", "USE d", "VACUUM", "VACUUM t", "VACUUM INTO '/tmp/x'", "ANALYZE t", "REINDEX t", "CLUSTER t",
    "COPY t TO '/tmp/x'", "COPY t FROM '/tmp/x'", "LOAD DATA INFILE '/x' INTO TABLE t", "LOCK TABLE t", "LOCK TABLES t WRITE", "UNLOCK TABLES",
    "KILL 1", "SHUTDOWN", "ATTACH DATABASE '/x' AS a", "DETACH DATABASE a", "PRAGMA foreign_keys = OFF", "PRAGMA writable_schema = 1",
    "PRAGMA journal_mode = DELETE", "PRAGMA user_version = 3", "PRAGMA table_info(t)", "PRAGMA foreign_keys", "PRAGMA writable_schema",
    "PRAGMA journal_mode", "PRAGMA table_info(t) ; PRAGMA foreign_keys=0", "PRAGMA optimize", "PRAGMA wal_checkpoint",
    "HANDLER t OPEN", "DECLARE c CURSOR FOR SELECT 1", "FETCH 1 FROM c", "PREPARE s AS SELECT 1", "DEALLOCATE s", "CHECKPOINT",
    "REFRESH MATERIALIZED VIEW v",
    "SELECT 1; DELETE FROM t", "SELECT 1;DELETE FROM t;", "SELECT 'a';/**/DROP TABLE t", "SELECT $$x$$; DROP TABLE t",
    "SELECT 1 -- x\n; DROP TABLE t", "SELECT 'it''s'; DELETE FROM t", "SELECT 'a\\'; DELETE FROM t; SELECT '",
    "SELECT \"a\"\"b\"; DROP TABLE t", "SELECT [a]]b]; DROP TABLE t", "SELECT `a``b`; DROP TABLE t",
    "SELECT $a$ ; $b$ ; $a$; DROP TABLE t", "SELECT 1 /* open; DROP TABLE t", "SELECT 'open; DROP TABLE t",
    "  \t\n DELETE FROM t", " DELETE FROM t", "SELECT 1; DELETE FROM t", "SЕLECT 1", "ＤＥＬＥＴＥ FROM t",
    "", ";", " ; ; ", "-- only\n", "/* only */",
]
READS_THAT_LOOK_LIKE_WRITES = [
    "SELECT 'DELETE FROM t'", "SELECT \"update\" FROM t", "SELECT created, updated_at, deleted_flag FROM t", "SELECT id FROM t -- DELETE",
    "SELECT id FROM t /* DROP */", "SELECT $$ DELETE $$", "SELECT [delete] FROM t", "SELECT `drop` FROM t", "SELECT 1 AS \"insert\"",
    "SELECT insert_date FROM t", "SELECT s.CURRVAL FROM DUAL", "SELECT DBMS_METADATA.GET_DDL('TABLE','T') FROM DUAL",
    "SELECT DBMS_XPLAN.DISPLAY FROM DUAL", "SELECT id FROM t WITH (NOLOCK)", "SELECT current_timestamp", "SELECT 'a' || 'b'",
    "SELECT sleeper FROM t", "SELECT sleep_time FROM t", "SELECT xp_name FROM t", "SELECT sp_name FROM t", "SELECT intoxicated FROM t",
    "SELECT forupdate FROM t", "SELECT 'x' AS \"for update\"", "SELECT pg_sleepy FROM t", "EXPLAIN SELECT 1",
    "EXPLAIN (FORMAT JSON) SELECT 1", "SHOW TABLES", "DESCRIBE t", "DESC t", "VALUES (1)", "TABLE t",
    "WITH a AS (SELECT 1) SELECT * FROM a",
]
STATEMENTS = WRITES + READS_THAT_LOOK_LIKE_WRITES
out = {"classify": [], "split": [], "ddl": [], "cost": [], "system_db": [], "explain": [], "script": [], "plan": []}
for e in dbrun.ENGINES:
    for s in STATEMENTS:
        k, why = dbrun.classify_statement(s, e)
        out["classify"].append([e, s, k, why])
        try:
            sp = ["ok", dbrun.split_statements(s, e)]
        except dbrun.Refused as x:
            sp = ["refused", str(x)]
        out["split"].append([e, s, sp[0], sp[1]])
        out["ddl"].append([e, s, dbrun.is_ddl(s, e)])
        for cls in ("production", "staging", "local"):
            out["cost"].append([e, s, cls, False, dbrun.cost_rules(s, cls, False, e)])
        out["cost"].append([e, s, "production", True, dbrun.cost_rules(s, "production", True, e)])
    out["explain"].append([e, "SELECT 1", dbrun.explain_sql(e, "SELECT 1")])
DBS = {
    "sqlserver": ["master", "model", "msdb", "tempdb", "TempDB", "MASTER", None, "", "shop", " master", "master "],
    "mysql": ["mysql", "sys", "information_schema", "performance_schema", "MySQL", "shop", None, ""],
    "mariadb": ["mysql", "sys", "Sys", "shop", None],
    "postgresql": ["template0", "template1", "TEMPLATE1", "postgres", "shop", None, ""],
    "oracle": ["master", "mysql", None], "sqlite": ["master", None],
}
for e, dbs in DBS.items():
    for db in dbs:
        try:
            dbrun.require_user_database({"engine": e, "database": db}, "plan")
            out["system_db"].append([e, db, "ok", ""])
        except dbrun.Refused as x:
            out["system_db"].append([e, db, "refused", str(x)])


class FakeDT(dt.datetime):
    """A fixed clock for the original's cmd_script header (the Rust side passes the same text)."""

    @classmethod
    def now(cls, tz=None):
        return cls(2026, 1, 1, 12, 0, 0, tzinfo=dt.timezone.utc)


real_dt = dbrun.dt
dbrun.dt = type("dtmod", (), {"datetime": FakeDT, "date": dt.date, "timezone": dt.timezone})
SCRIPTS = [
    ("sqlite", "UPDATE t SET a = 1 WHERE id = 1"), ("sqlite", "UPDATE t SET a = 1"), ("sqlite", "DELETE FROM t"),
    ("sqlite", "delete from t where 1=1; UPDATE t SET b=2 WHERE id=1"), ("postgresql", "UPDATE t SET a = 1 WHERE id = 1; SELECT 1"),
    ("mysql", "ALTER TABLE t ADD c int; UPDATE t SET a = 1 WHERE id = 1"), ("mariadb", "UPDATE t SET a = 1 WHERE id = 1"),
    ("sqlserver", "UPDATE t SET a = 1 WHERE id = 1"), ("oracle", "CREATE INDEX i ON t(a); UPDATE t SET a = 1 WHERE id = 1"),
    ("oracle", "DELETE FROM t WHERE id = 1"), ("postgresql", "UPDATE t SET a = 'WHERE'"), ("postgresql", "DELETE FROM t -- WHERE"),
    ("postgresql", "UPDATE t SET a = 1 /* WHERE */"), ("postgresql", "-- only"), ("postgresql", "SELECT 'a\\'; DELETE FROM t; SELECT '"),
    ("postgresql", "WITH d AS (DELETE FROM t WHERE id = 1 RETURNING id) SELECT id FROM d"), ("postgresql", "INSERT INTO t VALUES (1);;"),
]


def sandbox(d, prof, cls):
    dbrun.get_profile = lambda name, secret=True: dict(prof)
    dbrun.classify = lambda p: dict(cls, evidence=list(cls["evidence"]))
    dbrun.state_dir = lambda: Path(d) / "state"


for e, sql in SCRIPTS:
    with tempfile.TemporaryDirectory() as d:
        target = Path(d) / "sub" / "change.sql"
        sandbox(d, {"name": "P", "engine": e, "class": "production", "source": "x"},
                {"class": "production", "writable": False, "evidence": ["e"], "via": "host"})
        args = type("A", (), {"profile": "P", "sql": "-", "reason": "why", "out": str(target)})
        sys.stdin = io.StringIO(sql)
        try:
            with contextlib.redirect_stdout(io.StringIO()):
                dbrun.cmd_script(args)
            out["script"].append([e, sql, "ok", target.read_text()])
        except dbrun.Refused as x:
            out["script"].append([e, sql, "refused", str(x)])
        except dbrun.Failed as x:
            out["script"].append([e, sql, "failed", str(x)])
PLANS = [
    ("sqlite", "UPDATE t SET a = 1 WHERE id = 1", "1"), ("sqlite", "DELETE FROM t WHERE id = 3; UPDATE t SET a=1 WHERE id=2", "1,,x,-2"),
    ("postgresql", "CREATE TABLE n (id int); INSERT INTO n VALUES (1)", ""), ("oracle", "UPDATE t SET a = 1 WHERE id = 1", "1"),
    ("oracle", "CREATE TABLE n (id int)", ""), ("mysql", "UPDATE t SET a = 1 WHERE id = 1", " 2 , null"),
    ("mysql", "UPDATE t SET a = 1 WHERE id = 1", ""), ("sqlserver", "UPDATE t SET a = 1 WHERE id = 1", "1"),
]
for e, sql, expect in PLANS:
    with tempfile.TemporaryDirectory() as d:
        sandbox(d, {"name": "P", "engine": e, "class": "local", "source": "x", "database": "shop"},
                {"class": "local", "writable": True, "evidence": ["ev one", "ev two"], "via": "file"})
        args = type("A", (), {"profile": "P", "sql": "-", "reason": "why", "expect": expect})
        sys.stdin = io.StringIO(sql)
        buf = io.StringIO()
        try:
            with contextlib.redirect_stdout(buf):
                dbrun.cmd_plan(args)
        except (dbrun.Refused, dbrun.Failed) as x:
            out["plan"].append({"engine": e, "sql": sql, "expect": expect, "refused": str(x)})
            continue
        rec = json.loads(next((Path(d) / "state" / "plans").iterdir()).read_text())
        log = next((Path(d) / "state" / "log").iterdir()).read_text()
        plan_id = rec.pop("id")
        rec.pop("created")
        stdout = buf.getvalue().replace(plan_id, "<ID>").replace(str(Path(d)), "<STATE>")
        log = log.replace(plan_id, "<ID>").replace(str(Path(d)), "<STATE>")
        log = re.sub(r"\b[0-9a-f]{8}\b", "<EID>", log)
        log = re.sub(r"## \S+ ·", "## <TS> ·", log)
        out["plan"].append({"engine": e, "sql": sql, "expect": expect, "record": rec, "stdout": stdout, "log": log})
dbrun.dt = real_dt
# Output formats of `query` (print_table, --format json, --format csv) on awkward values.
import csv  # noqa: E402

out["format"] = []
for cols, rows, masked in [
    (["id", "nome", "note"], [[1, "Ana\nLima", None], [22, "x" * 130, True], [3, "é", 1.5]], {"nome": "name", "email": "email"}),
    (["a"], [], {}),
    (["a", "b"], [["q\"uote", "c,omma"], [False, -0.0], [10 ** 15, "tab\there"]], {}),
]:
    tbl = io.StringIO()
    with contextlib.redirect_stdout(tbl):
        dbrun.print_table(cols, rows)
    cs = io.StringIO()
    w = csv.writer(cs)
    w.writerow(cols)
    w.writerows(rows)
    out["format"].append({"columns": cols, "rows": rows, "masked": masked, "table": tbl.getvalue(), "csv": cs.getvalue(),
                          "json": json.dumps({"columns": cols, "rows": rows, "truncated": False, "masked": masked})})
(ROOT / "rust/fixtures/db-core/golden-writes.json").write_text(json.dumps(out, ensure_ascii=False, indent=1) + "\n")
print({k: len(v) for k, v in out.items()})
