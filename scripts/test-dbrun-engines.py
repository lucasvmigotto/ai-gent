#!/usr/bin/env python3
"""End-to-end test of dbrun against real database servers, one engine at a time.

  scripts/test-dbrun-engines.py [--engine NAME ...] [--keep]

Engines: postgresql, mysql, mariadb, sqlserver, oracle (default: all, in that
order). For each one it starts a container with a memory cap from a throwaway
compose project (a temporary git repository, so the local proof holds), seeds
it, runs the checks, and removes the container and its volumes before the next
engine starts. Peak memory is one engine's cap, at most 2 GB (SQL Server).

Checks: the local proof, masked and revealed reads, --explain, plan → apply →
restore (the plan's new table must be gone after the restore), a
count-mismatch rollback, the system-database refusal; then the same server as
a remote target (its published port, declared `development`): the read-only
refusals and a change script that is never executed.

No profiles file is involved: the local profiles are discovered from the
containers, and the others exist only in this process (dbrun's profile loader
is replaced). dbrun's log, plans and backups go to a temporary
AI_GENT_DB_STATE. Needs Podman or Docker with compose, and uv (dbrun fetches
drivers with it). Images are pulled on first use and kept; `--engine` limits
which. Not part of scripts/check.sh: run it before pushing a change to dbrun.
"""

from __future__ import annotations

import argparse
import contextlib
import io
import json
import os
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(REPO / "plugins" / "db" / "scripts"))
import dbrun  # noqa: E402

PASSWORD = "Dbrun_Test_1"  # a throwaway container's; never a real credential
SEED_ROWS = [(1, "Ana Souza", "ana.souza@example.com", "123.456.789-09"),
             (2, "Bruno Lima", "bruno@example.com", "987.654.321-00"),
             (3, "Carla Dias", None, None)]


def seed_sql(ts: str, text: str = "VARCHAR", number: str = "INT", prefix: str = "", tstype: str = "TIMESTAMP") -> str:
    def lit(v):
        return f"'{v}'" if v else "NULL"
    rows = [f"INSERT INTO {prefix}customer VALUES ({i}, {lit(n)}, {lit(e)}, {lit(c)}, {ts})" for i, n, e, c in SEED_ROWS]
    return (f"CREATE TABLE {prefix}customer (id {number} PRIMARY KEY, name {text}(80) NOT NULL, email {text}(120), "
            f"cpf {text}(14), created_at {tstype} NOT NULL);\n" + ";\n".join(rows) + ";\n")


# name → service definition and what the checks need to know about it
ENGINES = {
    "postgresql": {
        "image": "docker.io/library/postgres:17", "port": 5432, "mem": "512m",
        "env": {"POSTGRES_USER": "shop", "POSTGRES_PASSWORD": PASSWORD, "POSTGRES_DB": "shop"},
        "init": ("/docker-entrypoint-initdb.d", "seed.sql", seed_sql("now()")),
        "remote": {"database": "shop", "user": "shop"}, "system_db": "template1", "table": "customer",
        "exists": "SELECT COUNT(*) FROM information_schema.tables WHERE table_name = 'e2e_t'",
        "plan_word": "Scan", "refusals": ["SELECT pg_sleep(1)", "SELECT nextval('s')"],
    },
    "mysql": {
        "image": "docker.io/library/mysql:8.4", "port": 3306, "mem": "1g",
        "env": {"MYSQL_ROOT_PASSWORD": PASSWORD, "MYSQL_DATABASE": "shop", "MYSQL_USER": "shop", "MYSQL_PASSWORD": PASSWORD},
        "init": ("/docker-entrypoint-initdb.d", "seed.sql", seed_sql("NOW()")),
        "remote": {"database": "shop", "user": "shop"}, "system_db": "mysql", "table": "customer",
        "exists": "SELECT COUNT(*) FROM information_schema.TABLES WHERE TABLE_NAME = 'e2e_t'",
        "plan_word": "customer",
        "refusals": ["SELECT GET_LOCK('x', 1)", "SELECT id INTO OUTFILE '/tmp/x' FROM customer WHERE id = 1"],
    },
    "mariadb": {
        "image": "docker.io/library/mariadb:11.4", "port": 3306, "mem": "512m",
        "env": {"MARIADB_ROOT_PASSWORD": PASSWORD, "MARIADB_DATABASE": "shop", "MARIADB_USER": "shop",
                "MARIADB_PASSWORD": PASSWORD},
        "init": ("/docker-entrypoint-initdb.d", "seed.sql", seed_sql("NOW()")),
        "remote": {"database": "shop", "user": "shop"}, "system_db": "mysql", "table": "customer",
        "exists": "SELECT COUNT(*) FROM information_schema.TABLES WHERE TABLE_NAME = 'e2e_t'",
        "plan_word": "customer", "refusals": ["SELECT NEXT VALUE FOR s", "SELECT SLEEP(1)"],
    },
    "sqlserver": {
        "image": "mcr.microsoft.com/mssql/server:2022-latest", "port": 1433, "mem": "2g",
        "env": {"ACCEPT_EULA": "Y", "MSSQL_SA_PASSWORD": PASSWORD, "MSSQL_MEMORY_LIMIT_MB": "1536"},
        "init": None,  # the image has no init directory: seeded with its own sqlcmd after start
        "remote": {"database": "shop", "user": "sa"}, "system_db": "master", "table": "dbo.customer",
        "exists": "SELECT COUNT(*) FROM sys.tables WHERE name = 'e2e_t'",
        "plan_word": "Index",
        "refusals": ["SELECT id FROM dbo.customer WITH (UPDLOCK) WHERE id = 1", "SELECT NEXT VALUE FOR dbo.s", "EXEC sp_who"],
    },
    "oracle": {
        "image": os.environ.get("DBRUN_E2E_ORACLE_IMAGE", "docker.io/gvenzl/oracle-free:23-slim"),
        "port": 1521, "mem": "2g",
        "env": {"ORACLE_PASSWORD": PASSWORD, "APP_USER": "shop", "APP_USER_PASSWORD": PASSWORD},
        "init": ("/container-entrypoint-initdb.d", "01-seed.sql",
                 "ALTER SESSION SET CONTAINER=FREEPDB1;\n"
                 + seed_sql("SYSTIMESTAMP", "VARCHAR2", "NUMBER", "shop.")
                 + "COMMIT;\n"),
        "remote": {"service": "FREEPDB1", "user": "shop"}, "system_db": None, "table": "customer",
        "exists": None, "plan_word": "TABLE ACCESS",
        "refusals": ["SELECT s.NEXTVAL FROM DUAL", "SELECT DBMS_RANDOM.VALUE FROM DUAL"],
    },
}
REMOTE_REFUSALS = ["SELECT * FROM {t} WHERE id = 1", "SELECT COUNT(*) FROM {t}",
                   "SELECT id FROM {t} WHERE id = 1 FOR UPDATE",
                   "SELECT id FROM {t} WHERE id = 1; DELETE FROM {t} WHERE id = 1"]


class Report:
    def __init__(self):
        self.failed = 0

    def check(self, label: str, ok: bool, detail: str = "") -> bool:
        print(f"      {'ok  ' if ok else 'FAIL'}  {label}" + ("" if ok or not detail else f"\n            {detail.strip()[:500]}"))
        self.failed += not ok
        return ok


def call(*argv: str, sql: str | None = None) -> tuple[int, str]:
    """Run dbrun's main in-process (so the in-memory profiles apply); returns (exit code, output)."""
    out = io.StringIO()
    stdin = sys.stdin
    try:
        sys.stdin = io.StringIO(sql or "")
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(out):
            try:
                code = dbrun.main(list(argv) + (["--sql", "-"] if sql is not None else []))
            except SystemExit as e:
                code = int(e.code or 0)
    finally:
        sys.stdin = stdin
    return code, out.getvalue()


def compose_cmd(engine: str) -> list[str]:
    for cmd in ([engine, "compose"], ["docker-compose"], ["podman-compose"]):
        if shutil.which(cmd[0]) and subprocess.run(cmd + ["version"], capture_output=True).returncode == 0:
            return cmd
    sys.exit("error: no compose command found")


def write_project(root: Path, name: str, spec: dict, host_port: int) -> None:
    lines = [f"name: dbrun-e2e-{name}", "services:", f"  {name}:", f"    image: {spec['image']}",
             f"    mem_limit: {spec['mem']}", "    environment:"]
    lines += [f"      {k}: {json.dumps(v)}" for k, v in spec["env"].items()]
    lines += ["    ports:", f'      - "127.0.0.1:{host_port}:{spec["port"]}"']
    if spec["init"]:
        target, filename, sql = spec["init"]
        (root / "init").mkdir()
        (root / "init" / filename).write_text(sql)
        lines += ["    volumes:", f'      - "./init:{target}:ro,Z"']
    (root / "compose.yaml").write_text("\n".join(lines) + "\n")


def seed_sqlserver(engine: str, container: str) -> None:
    sql = ("IF DB_ID('shop') IS NULL CREATE DATABASE shop;\nGO\nUSE shop;\n"
           + seed_sql("SYSDATETIME()", "NVARCHAR", "INT", "dbo.", "DATETIME2") + "GO\n")
    err = ""
    for tools in ("/opt/mssql-tools18/bin/sqlcmd", "/opt/mssql-tools/bin/sqlcmd"):
        # the throwaway container's own client; the password goes in its environment, not its arguments
        proc = subprocess.run([engine, "exec", "-i", "-e", f"SQLCMDPASSWORD={PASSWORD}", container, tools, "-C", "-b",
                               "-S", "localhost", "-U", "sa"], input=sql, capture_output=True, text=True)
        if proc.returncode == 0:
            return
        err = proc.stderr or proc.stdout
    raise RuntimeError(f"seeding SQL Server failed: {err}")


def wait_ready(profile: str, timeout: int, probe: str | None = None) -> bool:
    """Until the profile connects (and, with `probe`, the seed is visible)."""
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        code, out = call("test", profile)
        if code == 0 and "SUCCESS" in out and (probe is None or call("query", profile, "--reason", "e2e ready", sql=probe)[0] == 0):
            return True
        time.sleep(5)
    return False


def plan_id(out: str) -> str:
    return next((line.split()[1] for line in out.splitlines() if line.startswith("Plan ")), "00000000-000000-0000")


def run_engine(name: str, spec: dict, rep: Report, engine: str, compose: list[str], host_port: int, keep: bool) -> None:
    print(f"  -- {name} ({spec['image']}, capped at {spec['mem']})")
    root = Path(tempfile.mkdtemp(prefix=f"dbrun-e2e-{name}-"))
    subprocess.run(["git", "init", "-q", str(root)], check=True)
    write_project(root, name, spec, host_port)
    os.chdir(root)
    container = f"dbrun-e2e-{name}-{name}-1"
    t = spec["table"]
    try:
        up = subprocess.run(compose + ["up", "-d"], capture_output=True, text=True)
        if not rep.check("container started", up.returncode == 0, up.stderr):
            return
        local = f"local:{name}"
        seeded = f"SELECT id FROM {t} WHERE id = 1"
        if name == "sqlserver":
            if not rep.check("server ready", wait_ready(local, 300)):
                return
            seed_sqlserver(engine, container)
            local = "MSSQL_LOCAL"  # the discovered profile is master, where writes are refused
        elif not rep.check("server ready and seeded", wait_ready(local, 600, seeded)):
            return

        # ------------------------------------------------------------ local
        code, out = call("classify", local)
        rep.check("local proof", code == 0 and ": local" in out, out)
        code, out = call("query", local, "--reason", "e2e", sql=f"SELECT id, name, email, cpf FROM {t} WHERE id <= 3 ORDER BY id")
        rep.check("read is masked", code == 0 and "a***@e***.com" in out and "ana.souza" not in out, out)
        code, out = call("query", local, "--reason", "e2e", "--reveal", sql=f"SELECT email FROM {t} WHERE id = 1")
        rep.check("--reveal shows values", code == 0 and "ana.souza@example.com" in out, out)
        code, out = call("query", local, "--reason", "e2e", "--explain", sql=f"SELECT name FROM {t} WHERE id = 2")
        rep.check("--explain returns the plan", code == 0 and spec["plan_word"].lower() in out.lower(), out)
        code, out = call("query", local, "--reason", "e2e", sql=f"UPDATE {t} SET name = 'X' WHERE id = 1")
        rep.check("query refuses a write", code == dbrun.EXIT_REFUSED, out)

        if spec["exists"]:
            (root / "plan.sql").write_text("CREATE TABLE e2e_t (n INT PRIMARY KEY);\nINSERT INTO e2e_t VALUES (1), (2)")
            _, out = call("plan", local, "--sql", str(root / "plan.sql"), "--reason", "e2e", "--expect", ",2")
            pid = plan_id(out)
            code, out = call("apply", local, pid, "--confirmed")
            rep.check("apply commits DDL and DML", code == 0 and "COMMITTED" in out, out)
            code, out = call("query", local, "--reason", "e2e", "--format", "json", sql=spec["exists"])
            rep.check("the plan's table exists", code == 0 and json.loads(out)["rows"] == [[1]], out)
            code, out = call("restore", local, pid, "--confirmed")
            rep.check("restore runs", code == 0 and "RESTORED" in out, out)
            code, out = call("query", local, "--reason", "e2e", "--format", "json", sql=spec["exists"])
            rep.check("restore removes the plan's table", code == 0 and json.loads(out)["rows"] == [[0]], out)
        else:
            (root / "plan.sql").write_text(f"UPDATE {t} SET email = NULL WHERE id = 3")
            _, out = call("plan", local, "--sql", str(root / "plan.sql"), "--reason", "e2e", "--expect", "1")
            code, out = call("apply", local, plan_id(out), "--confirmed")
            rep.check("apply commits DML, offers no restore", code == 0 and "COMMITTED" in out and "dbrun restore" not in out, out)
            (root / "ddl.sql").write_text("CREATE TABLE e2e_t (n NUMBER)")
            code, out = call("plan", local, "--sql", str(root / "ddl.sql"), "--reason", "e2e")
            rep.check("DDL refused without a backup", code == dbrun.EXIT_REFUSED, out)

        (root / "wide.sql").write_text(f"UPDATE {t} SET name = 'Zed' WHERE id >= 1")
        _, out = call("plan", local, "--sql", str(root / "wide.sql"), "--reason", "e2e", "--expect", "1")
        code, out = call("apply", local, plan_id(out), "--confirmed")
        _, out2 = call("query", local, "--reason", "e2e", "--format", "json", sql=f"SELECT COUNT(*) FROM {t} WHERE name = 'Zed'")
        rep.check("count mismatch rolls back", code != 0 and "ROLLED BACK" in out and '"rows": [[0]]' in out2, out + out2)

        if spec["system_db"]:
            code, out = call("plan", "SYSTEM_DB", "--sql", str(root / "wide.sql"), "--reason", "e2e")
            rep.check(f"plan in system database {spec['system_db']} refused",
                      code == dbrun.EXIT_REFUSED and "system database" in out, out)

        # ----------------------------------------------------------- remote
        code, out = call("classify", "REMOTE")
        rep.check("published port is not local", code == 0 and "development (read only)" in out, out)
        code, out = call("query", "REMOTE", "--reason", "e2e", "--format", "json", sql=seeded)
        rep.check("remote read works", code == 0 and '"rows": [[1]]' in out, out)
        refused = [s.format(t=t) for s in REMOTE_REFUSALS] + spec["refusals"]
        let_through = [s for s in refused if call("query", "REMOTE", "--reason", "e2e", sql=s)[0] != dbrun.EXIT_REFUSED]
        rep.check(f"remote refusals ({len(refused)})", not let_through, "let through: " + " | ".join(let_through))
        (root / "fix.sql").write_text(f"UPDATE {t} SET name = 'Scripted' WHERE id = 2")
        code, out = call("script", "REMOTE", "--sql", str(root / "fix.sql"), "--reason", "e2e", "--out", str(root / "change.sql"))
        _, out2 = call("query", "REMOTE", "--reason", "e2e", "--format", "json",
                       sql=f"SELECT COUNT(*) FROM {t} WHERE name = 'Scripted'")
        rep.check("change script written, never executed",
                  code == 0 and (root / "change.sql").exists() and '"rows": [[0]]' in out2, out + out2)
    finally:
        os.chdir(REPO)
        if keep:
            print(f"      kept {container} (compose project in {root})")
        else:
            subprocess.run(compose + ["down", "-v"], cwd=root, capture_output=True)
            shutil.rmtree(root, ignore_errors=True)


def profiles_for(name: str, spec: dict, host_port: int) -> dict[str, dict]:
    """In-memory profiles: the remote view of the container, and local ones naming a database."""
    container = f"dbrun-e2e-{name}-{name}-1"
    local = {"engine": name, "class": "local", "container": container, "host": "127.0.0.1", "port": str(spec["port"]),
             "password": PASSWORD, "source": "test", **spec["remote"]}
    out = {"REMOTE": {**local, "class": "development", "port": str(host_port), "container": None}}
    if name == "sqlserver":
        out["MSSQL_LOCAL"] = dict(local)
    if spec["system_db"]:
        out["SYSTEM_DB"] = {**local, "database": spec["system_db"]}
    for k, v in out.items():
        v["name"] = k
        if v.get("container") is None:
            v.pop("container")
    return out


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--engine", action="append", choices=list(ENGINES), help="only this engine (repeatable)")
    ap.add_argument("--keep", action="store_true", help="leave each container running, for debugging")
    ap.add_argument("--port", type=int, default=15432, help="host port for the remote checks (default 15432)")
    args = ap.parse_args()
    engine = dbrun.engine_cmd()
    if not engine:
        sys.exit("error: neither podman nor docker found")
    if not shutil.which("uv"):
        sys.exit("error: uv not found (dbrun fetches database drivers with it)")
    compose = compose_cmd(engine)
    state = tempfile.mkdtemp(prefix="dbrun-e2e-state-")
    os.environ["AI_GENT_DB_STATE"] = state
    rep = Report()
    print(f"dbrun end to end · {os.path.basename(engine)} · one engine at a time")
    for name in args.engine or list(ENGINES):
        spec = ENGINES[name]
        dbrun.user_profiles = lambda n=name, s=spec: profiles_for(n, s, args.port)
        started = time.monotonic()
        run_engine(name, spec, rep, engine, compose, args.port, args.keep)
        print(f"      ({time.monotonic() - started:.0f} s)")
    shutil.rmtree(state, ignore_errors=True)
    print("\nall checks passed" if not rep.failed else f"\n{rep.failed} check(s) failed")
    return 1 if rep.failed else 0


if __name__ == "__main__":
    sys.exit(main())
