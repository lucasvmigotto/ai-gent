"""Unit tests for dbrun: python3 -m unittest plugins/db/scripts/test_dbrun.py

Covers the statement classifier (including quoting tricks), the remote cost
rules, masking, profile parsing, classification of SQLite files, and the
SQLite end-to-end paths (read, refusals, plan → apply → restore, change
scripts, the command log) in a throwaway repository.
"""

import contextlib
import io
import json
import os
import sqlite3
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import dbrun  # noqa: E402


def kind(sql, engine="postgresql"):
    return dbrun.classify_statement(sql, engine)[0]


class ClassifierTest(unittest.TestCase):
    def test_reads(self):
        for sql in [
            "SELECT id, name FROM customer WHERE id = 1",
            "with recent as (select id from orders where created_at > now() - interval '1 day') select count(*) from recent",
            "SHOW max_connections",
            "EXPLAIN SELECT id FROM t WHERE id = 1",
            "VALUES (1), (2)",
            "TABLE customer",
            "SELECT 'DELETE FROM x; DROP TABLE y' AS text",
            "SELECT \"update\", \"delete\" FROM t",
            "SELECT id FROM t -- DELETE FROM t",
            "SELECT id FROM t /* DROP TABLE t */",
            "SELECT $$ DELETE FROM t $$",
            "SELECT created, updated_at, deleted_flag FROM t",
            "SELECT DBMS_METADATA.GET_DDL('TABLE','T') FROM DUAL",
        ]:
            with self.subTest(sql=sql):
                self.assertEqual(kind(sql), "read", dbrun.classify_statement(sql, "postgresql"))

    def test_writes(self):
        for sql in [
            "DELETE FROM t",
            "update t set a = 1",
            "INSERT INTO t VALUES (1)",
            "MERGE INTO t USING s ON (1=1) WHEN MATCHED THEN UPDATE SET a = 1",
            "WITH d AS (DELETE FROM t RETURNING id) SELECT id FROM d",
            "SELECT id INTO backup_t FROM t",
            "SELECT id FROM t FOR UPDATE",
            "SELECT id FROM t FOR NO KEY UPDATE",
            "SELECT id FROM t LOCK IN SHARE MODE",
            "SELECT nextval('seq')",
            "SELECT pg_sleep(10)",
            "SELECT pg_terminate_backend(123)",
            "SELECT dblink_exec('conn', 'DELETE FROM t')",
            "SELECT pg_read_file('/etc/passwd')",
            "EXPLAIN ANALYZE SELECT 1",
            "EXPLAIN (ANALYZE, BUFFERS) SELECT 1",
            "CALL do_things()",
            "EXEC sp_who",
            "BEGIN",
            "SET search_path = x",
            "TRUNCATE t",
            "VACUUM t",
            "ANALYZE t",
            "COPY t TO '/tmp/x'",
            "GRANT SELECT ON t TO u",
            "LOCK TABLE t",
            "SELECT UTL_HTTP.REQUEST('http://x') FROM DUAL",
            "SELECT DBMS_LOCK.SLEEP(5) FROM DUAL",
            "SELECT SLEEP(5)",
            "SELECT GET_LOCK('x', 10)",
            "",
        ]:
            with self.subTest(sql=sql):
                self.assertEqual(kind(sql), "write")

    def test_engine_specific(self):
        self.assertEqual(kind("EXPLAIN PLAN FOR SELECT 1 FROM DUAL", "oracle"), "write")
        self.assertEqual(kind("SELECT id INTO #tmp FROM t", "sqlserver"), "write")
        self.assertEqual(kind("SELECT [delete] FROM t", "sqlserver"), "read")
        self.assertEqual(kind("PRAGMA table_info(t)", "sqlite"), "read")
        self.assertEqual(kind("PRAGMA foreign_keys", "sqlite"), "read")
        self.assertEqual(kind("PRAGMA foreign_keys = OFF", "sqlite"), "write")
        self.assertEqual(kind("PRAGMA writable_schema", "sqlite"), "write")
        self.assertEqual(kind("PRAGMA table_info(t)", "postgresql"), "write")
        self.assertEqual(kind("SELECT load_extension('x')", "sqlite"), "write")

    def test_split(self):
        self.assertEqual(dbrun.split_statements("SELECT 1; SELECT 2;", "postgresql"), ["SELECT 1", "SELECT 2"])
        self.assertEqual(dbrun.split_statements("SELECT 'a;b'; SELECT 2", "postgresql"), ["SELECT 'a;b'", "SELECT 2"])
        self.assertEqual(dbrun.split_statements("SELECT $x$ ; $x$", "postgresql"), ["SELECT $x$ ; $x$"])
        self.assertEqual(dbrun.split_statements("-- only a comment\n", "postgresql"), [])

    def test_backslash_trick_refused(self):
        # PostgreSQL ends the string at \' and would run the DELETE; MySQL wouldn't.
        trick = "SELECT 'a\\'; DELETE FROM t; SELECT '"
        for engine in ("postgresql", "mysql"):
            with self.subTest(engine=engine), self.assertRaises(dbrun.Refused):
                dbrun.split_statements(trick, engine)

    def test_ddl(self):
        self.assertTrue(dbrun.is_ddl("ALTER TABLE t ADD c int", "postgresql"))
        self.assertTrue(dbrun.is_ddl("/* x */ create index i on t(c)", "postgresql"))
        self.assertFalse(dbrun.is_ddl("UPDATE t SET a = 1", "postgresql"))


class CostTest(unittest.TestCase):
    def rule(self, sql, cls="production", allow=False):
        return dbrun.cost_rules(sql, cls, allow, "postgresql")

    def test_star_and_counts_on_user_tables(self):
        self.assertIsNotNone(self.rule("SELECT * FROM orders WHERE id = 1"))
        self.assertIsNotNone(self.rule("SELECT o.* FROM orders o WHERE id = 1"))
        self.assertIsNotNone(self.rule("SELECT count(*) FROM orders"))
        self.assertIsNone(self.rule("SELECT count(*) FROM orders WHERE created_at >= '2026-01-01'"))
        self.assertIsNone(self.rule("SELECT count(*) FROM orders", allow=True))
        self.assertIsNone(self.rule("SELECT id, total FROM orders WHERE id = 1"))

    def test_catalog_and_local_exempt(self):
        self.assertIsNone(self.rule("SELECT * FROM information_schema.tables"))
        self.assertIsNone(self.rule("SELECT count(*) FROM pg_catalog.pg_class"))
        self.assertIsNone(self.rule("SELECT * FROM pg_stat_user_tables"))
        self.assertIsNone(self.rule("SELECT * FROM orders", cls="local"))


class MaskTest(unittest.TestCase):
    def test_columns(self):
        cols = ["id", "nome", "email", "cpf", "telefone", "password_hash", "birth_date", "status", "created_at", "total"]
        row = [7, "Maria da Silva", "maria.silva@example.com", "123.456.789-09", "(11) 98765-4321",
               "$2b$12$abc", "1990-05-17", "active", "2026-01-02T03:04:05", 99.5]
        masked, kinds = dbrun.mask_result(cols, [row])
        m = masked[0]
        self.assertEqual(m[0], 7)
        self.assertEqual(m[1], "M*** d*** S***")
        self.assertEqual(m[2], "m***@e***.com")
        self.assertEqual(m[3], "***.***.***-09")
        self.assertEqual(m[4], "(**) *****-**21")
        self.assertEqual(m[5], "[redacted]")
        self.assertEqual(m[6], "1990-**-**")
        self.assertEqual(m[7:], ["active", "2026-01-02T03:04:05", 99.5])
        self.assertEqual(set(kinds), {"nome", "email", "cpf", "telefone", "password_hash", "birth_date"})

    def test_reveal_keeps_secrets_redacted(self):
        masked, _ = dbrun.mask_result(["email", "senha"], [["a@b.com", "hunter2"]], reveal=True)
        self.assertEqual(masked[0], ["a@b.com", "[redacted]"])

    def test_free_text(self):
        masked, _ = dbrun.mask_result(["note"], [["call joao@corp.com.br, cpf 123.456.789-09, card 4111 1111 1111 1111"]])
        text = masked[0][0]
        self.assertNotIn("joao@corp", text)
        self.assertNotIn("123.456.789", text)
        self.assertNotIn("4111 1111 1111 1111", text)
        self.assertIn("1111", text)
        untouched, _ = dbrun.mask_result(["note"], [["order 1234567890123 shipped"]])
        self.assertEqual(untouched[0][0], "order 1234567890123 shipped")  # not a valid card number


class RedactTest(unittest.TestCase):
    def setUp(self):
        dbrun.REDACT.clear()

    def tearDown(self):
        dbrun.REDACT.clear()

    def test_host_user_password_redacted(self):
        dbrun.remember_secrets({"host": "db.secret-host.internal", "user": "app_ro", "password": "s3cr3t!"})
        msg = ('connection to server at "db.secret-host.internal" (10.0.0.5), port 5432 failed: '
               'FATAL:  password authentication failed for user "app_ro" (tried s3cr3t!)')
        out = dbrun.redact(msg)
        self.assertNotIn("secret-host", out)
        self.assertNotIn("app_ro", out)
        self.assertNotIn("s3cr3t!", out)
        self.assertIn("<host>", out)
        self.assertIn("<user>", out)

    def test_whole_words_only_and_loopback_kept(self):
        dbrun.remember_secrets({"host": "127.0.0.1", "user": "app"})
        self.assertEqual(dbrun.redact("user app on 127.0.0.1 in application"), "user <user> on 127.0.0.1 in application")


class Sandbox(unittest.TestCase):
    """A throwaway git repository with its own config and state directories."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        base = Path(self.tmp.name)
        self.repo = base / "repo"
        self.repo.mkdir()
        subprocess.run(["git", "init", "-q", str(self.repo)], check=True)
        self.outside = base / "elsewhere"
        self.outside.mkdir()
        self.env = {
            "AI_GENT_DB_CONFIG": str(base / "config"),
            "AI_GENT_DB_STATE": str(base / "state"),
            "CONTAINER_ENGINE": "no-such-engine",
        }
        self.old_env = {k: os.environ.get(k) for k in self.env}
        os.environ.update(self.env)
        self.old_cwd = os.getcwd()
        os.chdir(self.repo)
        (base / "config").mkdir()
        for path in (self.repo / "app.db", self.outside / "prod.db"):
            with sqlite3.connect(path) as c:
                c.executescript(
                    "CREATE TABLE customer (id INTEGER PRIMARY KEY, nome TEXT, email TEXT, status TEXT);"
                    "INSERT INTO customer VALUES (1, 'Ana Lima', 'ana@example.com', 'active'),"
                    "(2, 'Bruno Reis', 'bruno@example.com', 'blocked'), (3, 'Caio Dias', 'caio@example.com', 'active');"
                )
        self.write_profiles(
            f"LOCALDB_ENGINE=sqlite\nLOCALDB_PATH={self.repo / 'app.db'}\n"
            f"PRODDB_ENGINE=sqlite\nPRODDB_CLASS=production\nPRODDB_PATH={self.outside / 'prod.db'}\n"
            f"NOCLASS_ENGINE=sqlite\nNOCLASS_PATH={self.outside / 'prod.db'}\n"
        )

    def tearDown(self):
        os.chdir(self.old_cwd)
        for k, v in self.old_env.items():
            if v is None:
                os.environ.pop(k, None)
            else:
                os.environ[k] = v
        self.tmp.cleanup()

    def write_profiles(self, text):
        path = Path(self.env["AI_GENT_DB_CONFIG"]) / "connections.env"
        path.write_text(text)
        path.chmod(0o600)

    def sql(self, text):
        f = Path(self.tmp.name) / f"q{abs(hash(text))}.sql"
        f.write_text(text)
        return str(f)

    def dbrun(self, *argv):
        out, err = io.StringIO(), io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            code = dbrun.main(list(argv))
        return code, out.getvalue(), err.getvalue()


class ProfilesTest(Sandbox):
    def test_parse_and_classify(self):
        profiles = dbrun.user_profiles()
        self.assertEqual(set(profiles), {"LOCALDB", "PRODDB", "NOCLASS"})
        self.assertEqual(profiles["NOCLASS"]["class"], "production")  # unclassified = production
        self.assertEqual(dbrun.classify(profiles["LOCALDB"])["class"], "local")
        self.assertTrue(dbrun.classify(profiles["LOCALDB"])["writable"])
        self.assertEqual(dbrun.classify(profiles["PRODDB"])["class"], "production")
        self.assertFalse(dbrun.classify(profiles["PRODDB"])["writable"])

    def test_errors_are_redacted(self):
        self.write_profiles("GONE_ENGINE=sqlite\nGONE_CLASS=development\nGONE_PATH=/nowhere/private-host.db\n"
                            "GONE_HOST=private-host.internal\nGONE_USER=svc_reader\n")
        code, out, err = self.dbrun("test", "GONE")
        self.assertNotEqual(code, 0)
        self.assertNotIn("svc_reader", out + err)

    def test_listing_never_shows_secrets(self):
        self.write_profiles("R_ENGINE=postgresql\nR_CLASS=development\nR_HOST=db.internal\nR_USER=app\nR_PASSWORD=s3cr3t-value\n")
        code, out, _ = self.dbrun("profiles", "--json")
        self.assertEqual(code, 0)
        self.assertNotIn("s3cr3t-value", out)
        self.assertNotIn("db.internal", out)
        self.assertEqual(json.loads(out)[0]["class"], "development")

    def test_declared_local_without_proof_is_production(self):
        self.write_profiles("L_ENGINE=postgresql\nL_CLASS=local\nL_HOST=127.0.0.1\nL_CONTAINER=db\n")
        cls = dbrun.classify(dbrun.user_profiles()["L"])
        self.assertEqual(cls["class"], "production")
        self.assertFalse(cls["writable"])


class SqliteEndToEndTest(Sandbox):
    def test_read_masks_and_logs(self):
        code, out, err = self.dbrun("query", "LOCALDB", "--sql", self.sql("SELECT id, nome, email FROM customer ORDER BY id"),
                                    "--reason", "test read")
        self.assertEqual(code, 0, err)
        self.assertIn("A*** L***", out)
        self.assertNotIn("ana@example.com", out)
        code, out, _ = self.dbrun("query", "LOCALDB", "--sql", self.sql("SELECT email FROM customer WHERE id = 1"),
                                  "--reason", "user asked to see it", "--reveal")
        self.assertIn("ana@example.com", out)
        log = next(Path(self.env["AI_GENT_DB_STATE"], "log").iterdir()).read_text()
        self.assertIn("reason: test read", log)
        self.assertIn("reveal: yes", log)
        self.assertNotIn("ana@example.com", log)  # results are never logged

    def test_row_cap(self):
        code, out, _ = self.dbrun("query", "LOCALDB", "--sql", self.sql("SELECT id FROM customer"), "--reason", "t",
                                  "--max-rows", "2")
        self.assertEqual(code, 0)
        self.assertIn("truncated", out)

    def test_remote_refusals(self):
        for sql in ("DELETE FROM customer WHERE id = 1", "SELECT * FROM customer WHERE id = 1", "SELECT count(*) FROM customer"):
            with self.subTest(sql=sql):
                code, _, err = self.dbrun("query", "PRODDB", "--sql", self.sql(sql), "--reason", "t")
                self.assertEqual(code, dbrun.EXIT_REFUSED, err)
        code, _, err = self.dbrun("plan", "PRODDB", "--sql", self.sql("UPDATE customer SET status = 'x' WHERE id = 1"), "--reason", "t")
        self.assertEqual(code, dbrun.EXIT_REFUSED)
        self.assertIn("dbrun script", err)
        with sqlite3.connect(self.outside / "prod.db") as c:
            self.assertEqual(c.execute("SELECT COUNT(*) FROM customer WHERE status = 'active'").fetchone()[0], 2)

    def test_remote_read_is_read_only_even_past_the_classifier(self):
        # mode=ro + query_only: a write that slipped past the classifier still fails.
        req = {"engine": "sqlite", "conn": {"path": str(self.outside / "prod.db")}, "mode": "read",
               "statements": ["UPDATE customer SET status = 'x'"], "timeout": 5, "max_rows": 10}
        self.assertFalse(dbrun.execute(req)["ok"])

    def test_plan_apply_restore(self):
        code, out, err = self.dbrun("plan", "LOCALDB", "--sql", self.sql("UPDATE customer SET status = 'blocked' WHERE status = 'active'"),
                                    "--reason", "experiment", "--expect", "2")
        self.assertEqual(code, 0, err)
        plan_id = next(line.split()[1] for line in out.splitlines() if line.startswith("Plan "))
        code, _, err = self.dbrun("apply", "LOCALDB", plan_id)
        self.assertEqual(code, dbrun.EXIT_REFUSED, "apply without --confirmed must be refused")
        code, out, err = self.dbrun("apply", "LOCALDB", plan_id, "--confirmed")
        self.assertEqual(code, 0, err + out)
        db = self.repo / "app.db"
        with sqlite3.connect(db) as c:
            self.assertEqual(c.execute("SELECT COUNT(*) FROM customer WHERE status = 'active'").fetchone()[0], 0)
        code, out, err = self.dbrun("restore", "LOCALDB", plan_id, "--confirmed")
        self.assertEqual(code, 0, err)
        with sqlite3.connect(db) as c:
            self.assertEqual(c.execute("SELECT COUNT(*) FROM customer WHERE status = 'active'").fetchone()[0], 2)

    def test_apply_rolls_back_on_count_mismatch(self):
        code, out, _ = self.dbrun("plan", "LOCALDB", "--sql", self.sql("DELETE FROM customer WHERE status = 'active'"),
                                  "--reason", "t", "--expect", "5")
        plan_id = next(line.split()[1] for line in out.splitlines() if line.startswith("Plan "))
        code, out, _ = self.dbrun("apply", "LOCALDB", plan_id, "--confirmed")
        self.assertEqual(code, dbrun.EXIT_ERROR)
        self.assertIn("ROLLED BACK", out)
        with sqlite3.connect(self.repo / "app.db") as c:
            self.assertEqual(c.execute("SELECT COUNT(*) FROM customer").fetchone()[0], 3)

    def test_tampered_plan_refused(self):
        code, out, _ = self.dbrun("plan", "LOCALDB", "--sql", self.sql("DELETE FROM customer WHERE id = 3"), "--reason", "t")
        plan_id = next(line.split()[1] for line in out.splitlines() if line.startswith("Plan "))
        path = Path(self.env["AI_GENT_DB_STATE"], "plans", f"{plan_id}.json")
        plan = json.loads(path.read_text())
        plan["statements"] = ["DELETE FROM customer"]
        path.write_text(json.dumps(plan))
        code, _, err = self.dbrun("apply", "LOCALDB", plan_id, "--confirmed")
        self.assertEqual(code, dbrun.EXIT_REFUSED, err)

    def test_script_never_executes(self):
        out_file = self.repo / "docs" / "change.sql"
        code, out, err = self.dbrun("script", "PRODDB", "--sql", self.sql("UPDATE customer SET status = 'x' WHERE id = 1"),
                                    "--reason", "fix a record", "--out", str(out_file))
        self.assertEqual(code, 0, err)
        text = out_file.read_text()
        self.assertIn("NOT executed", text)
        self.assertIn("BEGIN;\nUPDATE customer SET status = 'x' WHERE id = 1;", text)
        self.assertIn("COMMIT;", text)
        code, _, err = self.dbrun("script", "PRODDB", "--sql", self.sql("DELETE FROM customer"), "--reason", "t",
                                  "--out", str(self.repo / "docs" / "bad.sql"))
        self.assertEqual(code, dbrun.EXIT_REFUSED, err)
        with sqlite3.connect(self.outside / "prod.db") as c:
            self.assertEqual(c.execute("SELECT status FROM customer WHERE id = 1").fetchone()[0], "active")


if __name__ == "__main__":
    unittest.main()
