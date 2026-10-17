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
import shutil
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
        for sql in ["SELECT id FROM t WITH (UPDLOCK) WHERE id = 1", "SELECT id FROM t WITH (XLOCK, HOLDLOCK)",
                    "SELECT id FROM t WITH (TABLOCKX)", "SELECT NEXT VALUE FOR dbo.s"]:
            with self.subTest(sql=sql):
                self.assertEqual(kind(sql, "sqlserver"), "write")
        self.assertEqual(kind("SELECT id FROM t WITH (NOLOCK) WHERE id = 1", "sqlserver"), "read")
        self.assertEqual(kind("SELECT s.NEXTVAL FROM DUAL", "oracle"), "write")
        self.assertEqual(kind("SELECT s.CURRVAL FROM DUAL", "oracle"), "read")
        self.assertEqual(kind("SELECT NEXT VALUE FOR s", "mariadb"), "write")

    def test_system_databases_refused_for_writes(self):
        for engine, db in [("sqlserver", "master"), ("sqlserver", None), ("sqlserver", "TempDB"),
                           ("mysql", "mysql"), ("mariadb", "sys"), ("postgresql", "template1")]:
            with self.subTest(engine=engine, db=db), self.assertRaises(dbrun.Refused):
                dbrun.require_user_database({"engine": engine, "database": db}, "plan")
        for engine, db in [("sqlserver", "shop"), ("mysql", "shop"), ("postgresql", "postgres"), ("oracle", None)]:
            with self.subTest(engine=engine, db=db):
                dbrun.require_user_database({"engine": engine, "database": db}, "plan")

    def test_explain_forms(self):
        self.assertEqual(dbrun.explain_sql("oracle", "SELECT 1 FROM DUAL"), "EXPLAIN PLAN FOR SELECT 1 FROM DUAL")
        self.assertEqual(dbrun.explain_sql("sqlite", "SELECT 1"), "EXPLAIN QUERY PLAN SELECT 1")
        self.assertEqual(dbrun.explain_sql("mysql", "SELECT 1"), "EXPLAIN SELECT 1")

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


class NetworkTest(unittest.TestCase):
    def test_container_or_pod(self):
        self.assertEqual(dbrun.network_args({"container_id": "abc", "pod": ""}), ["--network", "container:abc"])
        self.assertEqual(dbrun.network_args({"container_id": "abc"}), ["--network", "container:abc"])
        self.assertEqual(dbrun.network_args({"container_id": "abc", "pod": "p1", "pid": 4242}),
                         ["--network", "ns:/proc/4242/ns/net"])


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


FAKE_PASS = """#!/bin/sh
# A stand-in for pass: entries are plain files named <entry>.gpg in the store.
store="$PASSWORD_STORE_DIR"
case "$1" in
  show) [ -f "$store/$2.gpg" ] || { echo "Error: $2 is not in the password store." >&2; exit 1; }
        cat "$store/$2.gpg" ;;
  insert) e="$3"; mkdir -p "$(dirname "$store/$e")"; cat > "$store/$e.gpg" ;;
  *) exit 2 ;;
esac
"""


class PassTest(Sandbox):
    """Passwords from pass first, the profiles file last (shared/secrets.md)."""

    def setUp(self):
        super().setUp()
        base = Path(self.tmp.name)
        self.store = base / "store"
        (self.store / "ai-gent" / "db").mkdir(parents=True)
        bin_dir = base / "bin"
        bin_dir.mkdir()
        fake = bin_dir / "pass"
        fake.write_text(FAKE_PASS)
        fake.chmod(0o755)
        for k, v in (("PASSWORD_STORE_DIR", str(self.store)), ("PATH", f"{bin_dir}:{os.environ['PATH']}"),
                     ("AI_GENT_PASS_PREFIX", None)):
            self.old_env.setdefault(k, os.environ.get(k))
            if v is None:
                os.environ.pop(k, None)
            else:
                os.environ[k] = v
        dbrun.REDACT.clear()

    def entry(self, name, text):
        f = self.store / f"{name}.gpg"
        f.parent.mkdir(parents=True, exist_ok=True)
        f.write_text(text)

    def test_sources_in_order(self):
        self.entry("ai-gent/db/shop-dev", "from-default-entry\nnotes line\n")
        self.entry("work/db-pw", "from-explicit-entry\n")
        self.write_profiles("SHOP_DEV_ENGINE=postgresql\nSHOP_DEV_HOST=db.internal\nSHOP_DEV_USER=app\n"
                            "EXPLICIT_ENGINE=postgresql\nEXPLICIT_HOST=db\nEXPLICIT_USER=app\nEXPLICIT_PASSWORD=pass:work/db-pw\n"
                            "PLAIN_ENGINE=postgresql\nPLAIN_PASSWORD=plain-value\n"
                            "NONE_ENGINE=postgresql\n")
        profiles = dbrun.user_profiles()
        self.assertEqual((profiles["SHOP_DEV"]["secret"], profiles["SHOP_DEV"]["pass"]), ("pass", "ai-gent/db/shop-dev"))
        self.assertEqual((profiles["EXPLICIT"]["secret"], profiles["EXPLICIT"]["pass"]), ("pass", "work/db-pw"))
        self.assertNotIn("password", profiles["EXPLICIT"])
        self.assertEqual(profiles["PLAIN"]["secret"], "file")
        self.assertEqual(profiles["NONE"]["secret"], "none")
        # Read only when connecting; the first line only.
        self.assertNotIn("password", dbrun.get_profile("SHOP_DEV", secret=False))
        self.assertEqual(dbrun.get_profile("SHOP_DEV")["password"], "from-default-entry")
        self.assertEqual(dbrun.get_profile("EXPLICIT")["password"], "from-explicit-entry")
        self.assertEqual(dbrun.redact("auth failed (from-default-entry)"), "auth failed (<password>)")

    def test_prefix_override(self):
        os.environ["AI_GENT_PASS_PREFIX"] = "me"
        self.entry("me/db/x", "v\n")
        self.write_profiles("X_ENGINE=postgresql\n")
        self.assertEqual(dbrun.user_profiles()["X"]["pass"], "me/db/x")

    def test_listing_shows_source_never_value(self):
        self.entry("ai-gent/db/shop-dev", "never-shown\n")
        self.write_profiles("SHOP_DEV_ENGINE=postgresql\nPLAIN_ENGINE=postgresql\nPLAIN_PASSWORD=plain-value\n")
        code, out, _ = self.dbrun("profiles")
        self.assertEqual(code, 0)
        self.assertIn("ai-gent/db/shop-dev", out)
        self.assertIn("dbrun secrets migrate", out)
        self.assertNotIn("never-shown", out)
        self.assertNotIn("plain-value", out)

    def test_bad_or_unreadable_entry(self):
        self.write_profiles("A_ENGINE=postgresql\nA_HOST=db\nA_USER=app\nA_PASSWORD=pass:../escape\n"
                            "B_ENGINE=postgresql\nB_HOST=db\nB_USER=app\nB_PASSWORD=pass:ai-gent/db/missing\n")
        with self.assertRaisesRegex(dbrun.Failed, "isn't a pass entry"):
            dbrun.get_profile("A")
        with self.assertRaisesRegex(dbrun.Failed, "couldn't read ai-gent/db/missing"):
            dbrun.get_profile("B")

    def test_migrate(self):
        self.write_profiles("SHOP_DEV_ENGINE=postgresql\nSHOP_DEV_PASSWORD='s3cr3t value'\nSHOP_DEV_USER=app\n")
        code, out, err = self.dbrun("secrets", "migrate", "SHOP_DEV")
        self.assertEqual(code, 0, err)
        self.assertNotIn("s3cr3t", out + err)
        self.assertEqual((self.store / "ai-gent/db/shop-dev.gpg").read_text(), "s3cr3t value\n")
        path = next(dbrun.config_dir().iterdir())
        self.assertNotIn("PASSWORD", path.read_text())
        self.assertEqual(path.stat().st_mode & 0o777, 0o600)
        self.assertEqual(dbrun.user_profiles()["SHOP_DEV"]["secret"], "pass")
        code, out, _ = self.dbrun("secrets", "migrate", "SHOP_DEV")
        self.assertIn("nothing to migrate", out)



TOML = """
[defaults]
engine = "postgresql"
class = "development"
port = 5432

[families.shop]
host = "db.internal"
user = "shop_ro"
password = "pass:team/db/shop"

[profiles.SHOP_DEV]
family = "shop"
database = "shop_dev"

[profiles.SHOP_QA]
family = "shop"
database = "shop_qa"
host = "qa.internal"

[profiles.LEGACY]
engine = "oracle"
host = "ora.internal"
user = "app"
sid = "ORCL"
port = 1522
"""


class TomlTest(PassTest):
    def write_toml(self, text):
        path = dbrun.config_dir() / ("connections" + ".toml")
        path.write_text(text)
        path.chmod(0o600)
        return path

    def setUp(self):
        super().setUp()
        self.write_profiles("")

    def test_families_and_precedence(self):
        self.write_toml(TOML)
        profiles = dbrun.user_profiles()
        self.assertEqual(set(profiles), {"SHOP_DEV", "SHOP_QA", "LEGACY"})
        self.assertEqual(profiles["SHOP_DEV"]["host"], "db.internal")      # from the family
        self.assertEqual(profiles["SHOP_QA"]["host"], "qa.internal")       # the profile wins
        self.assertEqual(profiles["SHOP_DEV"]["port"], "5432")             # defaults, an integer
        self.assertEqual(profiles["SHOP_DEV"]["pass"], "team/db/shop")     # one password per family
        self.assertEqual(profiles["LEGACY"]["port"], "1522")
        self.assertEqual(profiles["LEGACY"]["class"], "development")

    def test_family_default_entry(self):
        self.entry("ai-gent/db/orders", "v\n")
        self.write_toml('[families.orders]\nengine = "postgresql"\nuser = "o"\nhost = "h"\n'
                        '[profiles.ORDERS_DEV]\nfamily = "orders"\n[profiles.ORDERS_QA]\nfamily = "orders"\n')
        profiles = dbrun.user_profiles()
        self.assertEqual(profiles["ORDERS_DEV"]["pass"], "ai-gent/db/orders")
        self.assertEqual(profiles["ORDERS_QA"]["pass"], "ai-gent/db/orders")

    def test_errors_name_keys_never_values(self):
        for text, want in [
            ('[profiles.X]\nengine = "postgresql"\npasword = "s3cr3t-typo"\n', "unknown key 'pasword' in [profiles.X]"),
            ('[profiles.X]\nfamily = "nope"\n', "names family 'nope'"),
            ('[secrets]\nx = "s3cr3t-typo"\n', "unknown table [secrets]"),
            ('[profiles.x]\nengine = "postgresql"\n', "must be [A-Z]"),
            ('[profiles.X]\nport = true\n', "port must be a string or an integer"),
            ('[profiles.X]\nhost = "s3cr3t-typo\n', "line 2"),
        ]:
            with self.subTest(want=want):
                self.write_toml(text)
                with self.assertRaises(dbrun.Failed) as ctx:
                    dbrun.user_profiles()
                self.assertIn(want, str(ctx.exception))
                self.assertNotIn("s3cr3t-typo", str(ctx.exception))
                self.assertNotIn(str(self.tmp.name), str(ctx.exception))

    def test_toml_before_env_and_no_duplicates(self):
        self.write_toml(TOML)
        self.write_profiles("OTHER_ENGINE=postgresql\n")
        self.assertIn("OTHER", dbrun.user_profiles())
        self.write_profiles("LEGACY_ENGINE=postgresql\n")
        with self.assertRaisesRegex(dbrun.Failed, "LEGACY is defined in both"):
            dbrun.user_profiles()

    def test_explicit_file(self):
        f = Path(self.tmp.name) / "team.toml"
        f.write_text(TOML)
        os.environ["AI_GENT_DB_CONNECTIONS"] = str(f)
        try:
            self.assertIn("SHOP_DEV", dbrun.user_profiles())
        finally:
            del os.environ["AI_GENT_DB_CONNECTIONS"]

    def test_oracle_sid_and_missing_keys(self):
        self.assertEqual(dbrun.oracle_dsn({"host": "h", "port": "1522", "service": "SVC"}), "h:1522/SVC")
        self.assertEqual(dbrun.oracle_dsn({"host": "h", "database": "PDB1"}), "h:1521/PDB1")
        self.assertEqual(dbrun.oracle_dsn({"host": "h", "port": "1522", "sid": "ORCL"}),
                         "(DESCRIPTION=(ADDRESS=(PROTOCOL=TCP)(HOST=h)(PORT=1522))(CONNECT_DATA=(SID=ORCL)))")
        self.assertEqual(dbrun.missing_keys({"engine": "oracle", "host": "h", "user": "u", "sid": "S"}), [])
        self.assertEqual(dbrun.missing_keys({"engine": "oracle", "host": "h", "user": "u"}), ["SERVICE|SID"])
        self.assertEqual(dbrun.missing_keys({"engine": "sqlite"}), ["PATH"])
        self.write_toml('[profiles.ORA]\nengine = "oracle"\nhost = "h"\n')
        with self.assertRaisesRegex(dbrun.Failed, "missing USER, SERVICE\\|SID"):
            dbrun.get_profile("ORA")

    def test_export_env(self):
        self.write_toml(TOML + '[profiles.PLAIN]\nengine = "postgresql"\nhost = "h"\nuser = "u"\npassword = "s3cr3t-plain"\n')
        out = Path(self.tmp.name) / "exported.env"
        code, stdout, err = self.dbrun("profiles", "--export-env", "--out", str(out))
        self.assertEqual(code, 0, err)
        text = out.read_text()
        self.assertEqual(out.stat().st_mode & 0o777, 0o600)
        self.assertIn("SHOP_DEV_PASSWORD=pass:team/db/shop", text)
        self.assertIn("LEGACY_SID=ORCL", text)
        self.assertNotIn("s3cr3t-plain", text + stdout + err)
        self.assertIn("PLAIN", stdout)
        self.assertNotIn("db.internal", stdout)
        code, _, err = self.dbrun("profiles", "--export-env", "--out", str(out))
        self.assertNotEqual(code, 0)                                         # never overwrites


class MigrateAllTest(PassTest):
    def test_accounts_and_reuse(self):
        self.write_profiles(
            "A_ENGINE=postgresql\nA_HOST=h1\nA_USER=app\nA_PASSWORD=shared-one\n"
            "B_ENGINE=postgresql\nB_HOST=h2\nB_USER=app\nexport B_PASSWORD='shared-one'\n"
            "C_ENGINE=postgresql\nC_HOST=h3\nC_USER=report\nC_PASSWORD=shared-one\n"
            "D_ENGINE=postgresql\nD_HOST=h4\nD_USER=solo\nD_PASSWORD=only-d\n"
            "E_ENGINE=postgresql\nE_HOST=h5\nE_USER=dup\nE_PASSWORD=pw-e1\n"
            "F_ENGINE=postgresql\nF_HOST=h6\nF_USER=dup\nF_PASSWORD=pw-f2\n")
        code, out, err = self.dbrun("secrets", "migrate", "--all")
        self.assertEqual(code, 0, err)
        for value in ("shared-one", "only-d", "pw-e1", "pw-f2"):
            self.assertNotIn(value, out + err)
        text = (dbrun.config_dir() / ("connections" + ".env")).read_text()
        self.assertIn("A_PASSWORD=pass:ai-gent/db/app", text)               # one entry per shared account
        self.assertIn("export B_PASSWORD=pass:ai-gent/db/app", text)
        self.assertIn("C_PASSWORD=pass:ai-gent/db/c", text)                 # another account: its own entry
        self.assertIn("E_PASSWORD=pass:ai-gent/db/e", text)                 # same user, different passwords
        self.assertIn("F_PASSWORD=pass:ai-gent/db/f", text)
        self.assertEqual((self.store / "ai-gent/db/app.gpg").read_text(), "shared-one\n")
        self.assertIn("6 profile(s) now reference 5 new pass entries", out)
        self.assertIn("hygiene: different accounts share one password: A, B, C", out)
        self.assertEqual({p["secret"] for p in dbrun.user_profiles().values()}, {"pass"})


class PassStoreTest(unittest.TestCase):
    """shared/passstore.py: mounts, recipients, the gpg fallback, sync."""

    def setUp(self):
        import passstore
        self.ps = passstore
        self.tmp = tempfile.TemporaryDirectory()
        base = Path(self.tmp.name)
        self.personal, self.team = base / "personal", base / "team"
        for d in (self.personal, self.team):
            d.mkdir()
        self.saved = {k: os.environ.get(k) for k in ("PASSWORD_STORE_DIR", "AI_GENT_PASS_MOUNTS", "PATH",
                                                     "AI_GENT_PASS_FRESHNESS")}
        os.environ["PASSWORD_STORE_DIR"] = str(self.personal)
        os.environ["AI_GENT_PASS_MOUNTS"] = f"team={self.team}"

    def tearDown(self):
        for k, v in self.saved.items():
            if v is None:
                os.environ.pop(k, None)
            else:
                os.environ[k] = v
        self.tmp.cleanup()

    def test_mounts_resolve(self):
        self.assertEqual(self.ps.resolve("team/db/shop"), (self.team, "db/shop"))
        self.assertEqual(self.ps.resolve("ai-gent/db/x"), (self.personal, "ai-gent/db/x"))
        self.assertEqual(self.ps.resolve("team"), (self.personal, "team"))  # a bare name isn't a mount path
        os.environ["AI_GENT_PASS_MOUNTS"] = "Bad Name=/x"
        with self.assertRaises(self.ps.StoreError):
            self.ps.resolve("x/y")

    def test_nearest_gpg_id(self):
        (self.team / ".gpg-id").write_text("AAAA\nBBBB\n")
        (self.team / "db" / "restricted").mkdir(parents=True)
        (self.team / "db" / "restricted" / ".gpg-id").write_text("AAAA\n")
        self.assertEqual(self.ps.recipients("team/db/shop"), ["AAAA", "BBBB"])
        self.assertEqual(self.ps.recipients("team/db/restricted/core"), ["AAAA"])

    def test_gpg_fallback_without_pass(self):
        fake = Path(self.tmp.name) / "bin"
        fake.mkdir()
        gpg = fake / "gpg"
        # Decrypt prints the file as is; encrypt copies stdin to --output (stand-ins for real crypto).
        gpg.write_text('#!/bin/sh\nout=""; prev=""\nfor a in "$@"; do [ "$prev" = --output ] && out="$a"; prev="$a"; done\n'
                       'case " $* " in *" --decrypt "*) for a in "$@"; do f="$a"; done; cat "$f" ;; '
                       '*" --encrypt "*) cat > "$out" ;; esac\n')
        gpg.chmod(0o755)
        (fake / "cat").symlink_to(shutil.which("cat"))
        os.environ["PATH"] = str(fake)  # no pass on PATH: the fallback path
        (self.team / ".gpg-id").write_text("AAAA\n")
        self.assertEqual(self.ps.write("team/db/shop", "v4lue"), "")
        self.assertEqual(self.ps.read("team/db/shop"), ("v4lue", []))
        with self.assertRaisesRegex(self.ps.StoreError, "already exists"):
            self.ps.write("team/db/shop", "other")

    def test_freshen_fast_forwards_a_synced_store(self):
        env = {**os.environ, "GIT_AUTHOR_NAME": "t", "GIT_AUTHOR_EMAIL": "t@t", "GIT_COMMITTER_NAME": "t",
               "GIT_COMMITTER_EMAIL": "t@t", "GIT_CONFIG_GLOBAL": "/dev/null"}
        base = Path(self.tmp.name)
        run = lambda *a, cwd=None: subprocess.run(a, cwd=cwd, env=env, check=True, capture_output=True)  # noqa: E731
        run("git", "init", "-q", "--bare", "-b", "main", str(base / "remote.git"))
        run("git", "clone", "-q", str(base / "remote.git"), str(base / "other"))
        (base / "other" / "seed").write_text("x")
        run("git", "add", ".", cwd=base / "other")
        run("git", "commit", "-q", "-m", "seed", cwd=base / "other")
        run("git", "push", "-q", "origin", "HEAD:main", cwd=base / "other")
        self.team.rmdir()
        run("git", "clone", "-q", str(base / "remote.git"), str(self.team))
        (base / "other" / "db").mkdir()
        (base / "other" / "db" / "new.gpg").write_text("x")
        run("git", "add", ".", cwd=base / "other")
        run("git", "commit", "-q", "-m", "add", cwd=base / "other")
        run("git", "push", "-q", "origin", "HEAD:main", cwd=base / "other")
        self.assertFalse(self.ps.has("team/db/new"))
        os.environ["AI_GENT_PASS_FRESHNESS"] = "300"
        self.assertEqual(self.ps.freshen(self.team), "")                   # never fetched: stale, so it pulls
        self.assertTrue(self.ps.has("team/db/new"))
        os.environ["AI_GENT_PASS_FRESHNESS"] = "0"
        self.assertEqual(self.ps.freshen(self.team), "")                   # 0 turns reads' sync off



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
