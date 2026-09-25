#!/usr/bin/env python3
"""dbrun — the db plugin's only path to a database.

Enforces ../references/safety.md mechanically: classifies the target, proves
"local", refuses anything but reads on remote targets, applies timeouts, row
caps and masking, and writes the command log before a statement runs. See
../references/runner.md for the interface.

Python 3.9+ stdlib on the host. Database drivers run in a child process
(`_exec`) started with `uv run --with <driver>`, on the host for remote
targets or inside a client container that joins a local database
container's network namespace. Credentials travel only on the child's stdin.
"""

from __future__ import annotations

import argparse
import datetime as dt
import decimal
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
import uuid
from pathlib import Path

EXIT_OK, EXIT_ERROR, EXIT_USAGE, EXIT_REFUSED, EXIT_CONNECT = 0, 1, 2, 3, 4

ENGINES = ("postgresql", "mysql", "mariadb", "sqlserver", "oracle", "sqlite")
REMOTE_CLASSES = ("production", "staging", "development")
DRIVERS = {
    "postgresql": "psycopg[binary]",
    "mysql": "PyMySQL",
    "mariadb": "PyMySQL",
    "sqlserver": "pymssql",
    "oracle": "oracledb",
}
DEFAULT_PORTS = {"postgresql": 5432, "mysql": 3306, "mariadb": 3306, "sqlserver": 1433, "oracle": 1521}
CLIENT_IMAGE = os.environ.get("AI_GENT_DB_CLIENT_IMAGE", "ghcr.io/astral-sh/uv:python3.12-bookworm-slim")
LOCK_TIMEOUT_MS = 5000
SCRIPT = Path(__file__).resolve()


class Refused(Exception):
    """A policy refusal: exit 3 with the rule that applied."""


class Failed(Exception):
    """An operational failure: exit 1 (or 4 for connections)."""

    def __init__(self, message: str, code: int = EXIT_ERROR):
        super().__init__(message)
        self.code = code


# ------------------------------------------------------------------ paths


def config_dir() -> Path:
    base = os.environ.get("XDG_CONFIG_HOME") or str(Path.home() / ".config")
    return Path(os.environ.get("AI_GENT_DB_CONFIG") or Path(base) / "ai-gent" / "db")


def state_dir() -> Path:
    base = os.environ.get("XDG_STATE_HOME") or str(Path.home() / ".local" / "state")
    d = Path(os.environ.get("AI_GENT_DB_STATE") or Path(base) / "ai-gent" / "db")
    d.mkdir(parents=True, exist_ok=True, mode=0o700)
    return d


def repo_root() -> Path:
    try:
        out = subprocess.run(["git", "rev-parse", "--show-toplevel"], capture_output=True, text=True, timeout=10)
        if out.returncode == 0 and out.stdout.strip():
            return Path(out.stdout.strip()).resolve()
    except (OSError, subprocess.SubprocessError):
        pass
    return Path.cwd().resolve()


def inside(path: Path, root: Path) -> bool:
    try:
        path.resolve().relative_to(root.resolve())
        return True
    except ValueError:
        return False


def engine_cmd() -> str | None:
    env = os.environ.get("CONTAINER_ENGINE")
    if env:
        return env if shutil.which(env) else None
    for candidate in ("podman", "docker"):
        if shutil.which(candidate):
            return candidate
    return None


def ro_mount() -> str:
    """Read-only bind-mount options; Podman on SELinux hosts also needs a relabel."""
    return "ro,z" if os.path.basename(engine_cmd() or "") == "podman" else "ro"


def run(cmd: list[str], *, input: str | bytes | None = None, timeout: float = 60, text: bool = True):
    """subprocess.run that turns a missing binary or a timeout into a failed result."""
    try:
        return subprocess.run(cmd, input=input, capture_output=True, text=text, timeout=timeout)
    except subprocess.TimeoutExpired:
        return subprocess.CompletedProcess(cmd, 124, "" if text else b"", f"timed out after {timeout} s")
    except OSError as e:
        return subprocess.CompletedProcess(cmd, 127, "" if text else b"", str(e))


# --------------------------------------------------------------- profiles

SUFFIXES = ("ENGINE", "CLASS", "HOST", "PORT", "DATABASE", "SERVICE", "USER", "PASSWORD", "PATH", "CONTAINER", "PROJECT")
SECRET_FIELDS = ("password",)


def parse_env_file(path: Path) -> dict[str, str]:
    """KEY=VALUE lines; quotes stripped; no expansion, no execution."""
    values: dict[str, str] = {}
    for lineno, raw in enumerate(path.read_text().splitlines(), 1):
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        if line.startswith("export "):
            line = line[7:].lstrip()
        key, sep, value = line.partition("=")
        key = key.strip()
        if not sep or not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", key):
            raise Failed(f"{path}:{lineno}: not a KEY=VALUE line")
        value = value.strip()
        if len(value) >= 2 and value[0] == value[-1] and value[0] in "\"'":
            value = value[1:-1]
        elif " #" in value:
            value = value.split(" #", 1)[0].rstrip()
        values[key] = value
    return values


def user_profiles() -> dict[str, dict]:
    path = config_dir() / "connections.env"
    if not path.exists():
        return {}
    mode = path.stat().st_mode & 0o777
    if mode & 0o077:
        print(f"warn: {path} is mode {mode:o}; it holds credentials — chmod 600 it", file=sys.stderr)
    profiles: dict[str, dict] = {}
    for key, value in parse_env_file(path).items():
        for suffix in SUFFIXES:
            if key.endswith("_" + suffix) and len(key) > len(suffix) + 1:
                name = key[: -len(suffix) - 1]
                if re.fullmatch(r"[A-Z][A-Z0-9_]*", name):
                    profiles.setdefault(name, {"name": name, "source": str(path)})[suffix.lower()] = value
                break
    for p in profiles.values():
        p.setdefault("class", "production")  # unclassified = production
        p["class"] = p["class"].lower()
        if p["class"] not in REMOTE_CLASSES + ("local",):
            p["class"] = "production"
        p["engine"] = p.get("engine", "").lower()
    return profiles


PROXY_IMAGES = re.compile(r"pgbouncer|pgpool|cloud-sql-proxy|cloudsql|socat|haproxy|envoy|proxysql|odyssey|nginx|traefik", re.I)
ENGINE_IMAGES = [
    ("postgresql", re.compile(r"postgres|postgis|timescale", re.I)),
    ("mariadb", re.compile(r"mariadb", re.I)),
    ("mysql", re.compile(r"(^|/)mysql|percona", re.I)),
    ("sqlserver", re.compile(r"mssql|sql-server|azure-sql-edge", re.I)),
    ("oracle", re.compile(r"oracle|gvenzl", re.I)),
]


def image_engine(image: str) -> str | None:
    if PROXY_IMAGES.search(image):
        return None
    for engine, pattern in ENGINE_IMAGES:
        if pattern.search(image):
            return engine
    return None


def list_containers(engine: str) -> list[dict]:
    out = run([engine, "ps", "--no-trunc", "--format", "json"], timeout=30)
    if out.returncode != 0:
        return []
    text = out.stdout.strip()
    if not text:
        return []
    items = json.loads(text) if text.startswith("[") else [json.loads(l) for l in text.splitlines() if l.strip()]
    result = []
    for item in items:
        labels = item.get("Labels") or {}
        if isinstance(labels, str):
            labels = dict(kv.split("=", 1) for kv in labels.split(",") if "=" in kv)
        names = item.get("Names")
        name = names[0] if isinstance(names, list) else names
        result.append({"id": item.get("ID") or item.get("Id"), "name": name, "image": item.get("Image", ""), "labels": labels})
    return result


def container_env(engine: str, container: str) -> dict[str, str]:
    out = run([engine, "inspect", "--format", "{{json .Config.Env}}", container], timeout=30)
    if out.returncode != 0:
        return {}
    return dict(e.split("=", 1) for e in json.loads(out.stdout) or [] if "=" in e)


def credentials_from_env(eng: str, env: dict[str, str]) -> dict:
    if eng == "postgresql":
        user = env.get("POSTGRES_USER", "postgres")
        return {"user": user, "password": env.get("POSTGRES_PASSWORD", ""), "database": env.get("POSTGRES_DB", user)}
    if eng in ("mysql", "mariadb"):
        p = "MARIADB_" if eng == "mariadb" and any(k.startswith("MARIADB_") for k in env) else "MYSQL_"
        if env.get(p + "USER"):
            return {"user": env[p + "USER"], "password": env.get(p + "PASSWORD", ""), "database": env.get(p + "DATABASE", "")}
        return {"user": "root", "password": env.get(p + "ROOT_PASSWORD", ""), "database": env.get(p + "DATABASE", "")}
    if eng == "sqlserver":
        return {"user": "sa", "password": env.get("MSSQL_SA_PASSWORD") or env.get("SA_PASSWORD", ""), "database": "master"}
    if eng == "oracle":
        if env.get("APP_USER"):
            return {"user": env["APP_USER"], "password": env.get("APP_USER_PASSWORD", ""), "service": "FREEPDB1"}
        return {"user": "system", "password": env.get("ORACLE_PASSWORD", ""), "service": "FREEPDB1"}
    return {}


def local_profiles(root: Path) -> dict[str, dict]:
    engine = engine_cmd()
    if not engine or not engine_endpoint_local(engine)[0]:
        return {}  # a remote engine's containers are never local
    profiles = {}
    for c in list_containers(engine):
        wd = c["labels"].get("com.docker.compose.project.working_dir")
        eng = image_engine(c["image"])
        if not eng or not wd or not inside(Path(wd), root):
            continue
        service = c["labels"].get("com.docker.compose.service") or c["name"]
        p = {"name": f"local:{service}", "source": "running container", "engine": eng, "class": "local",
             "container": c["name"], "port": str(DEFAULT_PORTS[eng]), "host": "127.0.0.1"}
        p.update(credentials_from_env(eng, container_env(engine, c["name"])))
        profiles[p["name"]] = p
    return profiles


def all_profiles() -> dict[str, dict]:
    profiles = local_profiles(repo_root())
    for name, p in user_profiles().items():
        if name in profiles:
            raise Failed(f"profile {name} is defined twice; rename one")
        profiles[name] = p
    return profiles


def get_profile(name: str) -> dict:
    profiles = all_profiles()
    if name not in profiles:
        known = ", ".join(sorted(profiles)) or "none"
        raise Failed(f"no profile {name!r} (known: {known}); see `dbrun profiles`")
    p = dict(profiles[name])
    remember_secrets(p)
    if p.get("engine") not in ENGINES:
        raise Failed(f"profile {name}: engine {p.get('engine')!r} isn't one of {', '.join(ENGINES)}")
    return p


# ---------------------------------------------------------- classification


def engine_endpoint_local(engine: str) -> tuple[bool, str]:
    for var in ("DOCKER_HOST", "CONTAINER_HOST"):
        value = os.environ.get(var, "")
        if value and not value.startswith("unix://"):
            return False, f"{var}={value.split('://')[0]}:// is not a local socket"
    if os.path.basename(engine) == "podman":
        out = run([engine, "info", "--format", "{{.Host.ServiceIsRemote}}"], timeout=30)
        if out.returncode != 0 or out.stdout.strip() != "false":
            return False, "podman reports a remote service (or `podman info` failed)"
        return True, "podman service is local"
    out = run([engine, "context", "inspect", "--format", "{{.Endpoints.docker.Host}}"], timeout=30)
    host = out.stdout.strip()
    if out.returncode != 0 or not host.startswith("unix://"):
        return False, f"docker context endpoint {host or '?'} is not a local socket"
    return True, f"docker endpoint {host}"


def classify(p: dict) -> dict:
    """Returns {class, writable, evidence[], via} — `local` only when proven."""
    evidence: list[str] = []
    root = repo_root()
    declared = p.get("class", "production")

    if p["engine"] == "sqlite":
        path = Path(p.get("path", "")).expanduser()
        if not path.is_file():
            raise Failed(f"sqlite file {path} doesn't exist", EXIT_CONNECT)
        real = path.resolve()
        if inside(real, root) and not path.is_symlink():
            evidence.append(f"file inside this repository ({real.relative_to(root)})")
            return {"class": "local", "writable": True, "evidence": evidence, "via": "file"}
        evidence.append("file outside this repository — not a local copy of this project")
        return {"class": declared if declared != "local" else "production", "writable": False, "evidence": evidence, "via": "file"}

    wants_local = declared == "local" or p.get("container")
    if not wants_local:
        evidence.append(f"declared {declared} in {p.get('source', '?')}")
        return {"class": declared, "writable": False, "evidence": evidence, "via": "host"}

    def remote(reason: str) -> dict:
        evidence.append(f"NOT PROVEN LOCAL: {reason}")
        cls = declared if declared != "local" else "production"
        return {"class": cls, "writable": False, "evidence": evidence, "via": "host"}

    engine = engine_cmd()
    if not engine:
        return remote("no container engine (podman or docker) on this machine")
    ok, why = engine_endpoint_local(engine)
    if not ok:
        return remote(why)
    evidence.append(why)
    container = p.get("container")
    if not container:
        return remote("no container named for this profile")
    out = run([engine, "inspect", "--format", "{{json .}}", container], timeout=30)
    if out.returncode != 0:
        return remote(f"container {container} not found")
    info = json.loads(out.stdout)
    if isinstance(info, list):
        info = info[0]
    if not (info.get("State") or {}).get("Running"):
        return remote(f"container {container} is not running")
    image = (info.get("Config") or {}).get("Image", "")
    if image_engine(image) != p["engine"] and not (p["engine"] == "mysql" and image_engine(image) == "mariadb"):
        return remote(f"container image {image} is not a {p['engine']} server (a proxy or another engine)")
    labels = (info.get("Config") or {}).get("Labels") or {}
    wd = labels.get("com.docker.compose.project.working_dir", "")
    if not wd or not inside(Path(wd), root):
        return remote(f"container {container} wasn't started by this repository's compose project")
    evidence.append(f"container {container} ({image}) running, compose project in {Path(wd).relative_to(root) if Path(wd) != root else '.'}")
    evidence.append("reached through the container's own network namespace")
    return {"class": "local", "writable": True, "evidence": evidence, "via": "container", "container_id": info.get("Id")}


# -------------------------------------------------------- statement classes

READ_START = {"SELECT", "WITH", "SHOW", "DESCRIBE", "DESC", "EXPLAIN", "VALUES", "TABLE", "PRAGMA"}
DDL_START = {"CREATE", "ALTER", "DROP", "TRUNCATE", "RENAME", "GRANT", "REVOKE", "COMMENT"}
WRITE_WORDS = {"INSERT", "UPDATE", "DELETE", "MERGE", "UPSERT", "TRUNCATE", "DROP", "ALTER", "CREATE", "GRANT", "REVOKE"}
LOCKING = [r"\bFOR\s+(NO\s+KEY\s+)?UPDATE\b", r"\bFOR\s+(KEY\s+)?SHARE\b", r"\bLOCK\s+IN\s+SHARE\s+MODE\b", r"\bINTO\b"]
SIDE_EFFECT_FUNCS = re.compile(
    r"\b(NEXTVAL|SETVAL|TXID_CURRENT|PG_ADVISORY\w*|PG_TRY_ADVISORY\w*|PG_SLEEP\w*|PG_TERMINATE_BACKEND|PG_CANCEL_BACKEND|"
    r"PG_RELOAD_CONF|PG_ROTATE_LOGFILE|PG_READ_FILE|PG_READ_BINARY_FILE|PG_LS_DIR|PG_STAT_FILE|SET_CONFIG|LO_IMPORT|"
    r"LO_EXPORT|LO_UNLINK|DBLINK\w*|SLEEP|BENCHMARK|GET_LOCK|RELEASE_LOCK|RELEASE_ALL_LOCKS|LOAD_FILE|XP_\w+|SP_\w+|"
    r"OPENROWSET|OPENQUERY|OPENDATASOURCE|WAITFOR|UTL_\w+|SYS_EXEC|SYS_EVAL|LOAD_EXTENSION|WRITEFILE|READFILE)\s*\(",
    re.I,
)
ORACLE_PACKAGES = re.compile(r"\b(DBMS_(?!METADATA\b|XPLAN\b)\w+|UTL_\w+|OWA_\w+|HTP|HTF)\b", re.I)
READ_PRAGMAS = {"TABLE_INFO", "TABLE_XINFO", "TABLE_LIST", "INDEX_LIST", "INDEX_INFO", "INDEX_XINFO", "FOREIGN_KEY_LIST",
                "DATABASE_LIST", "COMPILE_OPTIONS", "PAGE_COUNT", "PAGE_SIZE", "FREELIST_COUNT", "FOREIGN_KEY_CHECK",
                "INTEGRITY_CHECK", "QUICK_CHECK", "COLLATION_LIST", "FUNCTION_LIST", "PRAGMA_LIST", "USER_VERSION",
                "SCHEMA_VERSION", "ENCODING", "JOURNAL_MODE", "FOREIGN_KEYS"}


def scan(sql: str, backslash: bool, brackets: bool) -> list[tuple[str, str]]:
    """Split on semicolons outside literals and comments.

    Returns (original, blanked) per statement; in `blanked`, comments are gone
    and string/quoted-identifier literals are placeholders, so keywords inside
    them can't mislead the classifier. `backslash`: backslash escapes quotes
    (MySQL); `brackets`: [..] quotes identifiers (SQL Server). A `#` is never a
    comment (it's an operator in PostgreSQL): a MySQL `#` comment only makes
    the classifier stricter.
    """
    stmts: list[tuple[str, str]] = []
    orig: list[str] = []
    blank: list[str] = []
    i, n = 0, len(sql)
    closers = {"'": "'", '"': '"', "`": "`"}
    if brackets:
        closers["["] = "]"

    def flush():
        o, b_ = "".join(orig).strip(), "".join(blank).strip()
        if b_:
            stmts.append((o, b_))
        orig.clear()
        blank.clear()

    while i < n:
        c = sql[i]
        if sql.startswith("--", i):
            j = sql.find("\n", i)
            j = n if j < 0 else j
            orig.append(sql[i:j])
            blank.append(" ")
            i = j
            continue
        if sql.startswith("/*", i):
            j = sql.find("*/", i + 2)
            j = n if j < 0 else j + 2
            orig.append(sql[i:j])
            blank.append(" ")
            i = j
            continue
        m = re.match(r"\$([A-Za-z_][A-Za-z0-9_]*)?\$", sql[i:])
        if m:
            tag = m.group(0)
            j = sql.find(tag, i + len(tag))
            j = n if j < 0 else j + len(tag)
            orig.append(sql[i:j])
            blank.append(" '' ")
            i = j
            continue
        if c in closers:
            close = closers[c]
            j = i + 1
            while j < n:
                if backslash and sql[j] == "\\" and close in "'\"":
                    j += 2
                    continue
                if sql[j] == close:
                    if close in "'\"`" and j + 1 < n and sql[j + 1] == close:
                        j += 2
                        continue
                    break
                j += 1
            orig.append(sql[i:j + 1])
            blank.append(" '' " if c == "'" else " x ")
            i = j + 1
            continue
        if c == ";":
            flush()
            i += 1
            continue
        orig.append(c)
        blank.append(c)
        i += 1
    flush()
    return stmts


def parse_modes(engine: str) -> tuple[bool, bool]:
    return engine in ("mysql", "mariadb"), engine == "sqlserver"


def blank_literals(sql: str, engine: str = "postgresql") -> str:
    backslash, brackets = parse_modes(engine)
    return " ".join(b for _, b in scan(sql, backslash, brackets))


def split_statements(sql: str, engine: str) -> list[str]:
    """The engine's own split — refused if the other escaping mode would split or
    read it differently (a quoting trick that hides a statement)."""
    backslash, brackets = parse_modes(engine)
    mine = scan(sql, backslash, brackets)
    other = scan(sql, not backslash, brackets)
    if [o for o, _ in mine] != [o for o, _ in other]:
        raise Refused("the statements split differently depending on backslash escaping; rewrite the string "
                      "literals without backslashes")
    return [o for o, _ in mine]


def classify_statement(stmt: str, engine: str) -> tuple[str, str]:
    """('read'|'write', reason). Unknown → write (deny by default)."""
    text = blank_literals(stmt, engine)
    words = re.findall(r"[A-Za-z_][A-Za-z0-9_$]*", text)
    if not words:
        return "write", "empty or unparseable statement"
    first = words[0].upper()
    upper = [w.upper() for w in words]
    if first not in READ_START:
        return "write", f"starts with {first}"
    if first == "PRAGMA":
        name = upper[1] if len(upper) > 1 else ""
        if engine != "sqlite" or name not in READ_PRAGMAS or "=" in text:
            return "write", f"PRAGMA {name.lower()} can change the database"
    if first == "EXPLAIN":
        if engine == "oracle":
            return "write", "Oracle EXPLAIN PLAN writes to PLAN_TABLE"
        if "ANALYZE" in upper or "ANALYSE" in upper:
            return "write", "EXPLAIN ANALYZE executes the statement"
    hit = WRITE_WORDS.intersection(upper[1:])
    if hit:
        return "write", f"contains {sorted(hit)[0]}"
    for pattern in LOCKING:
        if re.search(pattern, text, re.I):
            return "write", f"takes locks or writes ({re.search(pattern, text, re.I).group(0).upper()})"
    m = SIDE_EFFECT_FUNCS.search(text)
    if m:
        return "write", f"calls {m.group(1).upper()}, which has side effects"
    m = ORACLE_PACKAGES.search(text)
    if m:
        return "write", f"calls the {m.group(0).upper()} package"
    return "read", "read-only statement"


def is_ddl(stmt: str, engine: str) -> bool:
    words = re.findall(r"[A-Za-z_]+", blank_literals(stmt, engine))
    return bool(words) and words[0].upper() in DDL_START


CATALOG = re.compile(r"^(information_schema|pg_catalog|pg_\w+|sys|all_\w+|user_\w+|dba_\w+|v\$\w+|gv\$\w+|"
                     r"sqlite_\w+|pragma_\w+|performance_schema|mysql)$", re.I)


def catalog_only(text: str) -> bool:
    """Every FROM/JOIN target is a catalog object (schema.table or bare view)."""
    targets = re.findall(r"\b(?:FROM|JOIN)\s+([A-Za-z_][\w$]*)(?:\s*\.\s*([A-Za-z_][\w$]*))?", text, re.I)
    if not targets:
        return True
    return all(CATALOG.match(schema) or (not table and CATALOG.match(schema)) for schema, table in targets)


def cost_rules(stmt: str, cls: str, allow_full_count: bool, engine: str) -> str | None:
    """Remote cost guards on user tables: named columns, no unfiltered counts."""
    if cls == "local":
        return None
    text = blank_literals(stmt, engine)
    if catalog_only(text):
        return None
    if re.search(r"\bSELECT\s+(DISTINCT\s+|TOP\s*\(?\s*\d+\s*\)?\s+)?([A-Za-z_][A-Za-z0-9_]*\.)?\*", text, re.I):
        return "name the columns instead of SELECT * (safety.md § Cost)"
    if re.search(r"\bCOUNT\s*\(", text, re.I) and not re.search(r"\bWHERE\b", text, re.I) and not allow_full_count:
        return ("an unfiltered COUNT scans the whole table — use the catalog's row estimate, filter on an indexed "
                "column, or pass --allow-full-count if the user asked for an exact full count")
    return None


# ------------------------------------------------------------------ masking

MASK_RULES = [
    ("secret", re.compile(r"pass(word|wd)?$|^pwd$|senha|secret|token|api_?key|hash|salt|private_?key|credential|access_?key", re.I)),
    ("email", re.compile(r"e_?mail|^mail$|_mail$", re.I)),
    ("document", re.compile(r"(^|_)(cpf|cnpj|rg|ssn|nif|nie|cns|pis|nis|passport|passaporte|documento?|tax_?id|renavam|cnh)($|_)", re.I)),
    ("phone", re.compile(r"phone|telefone|fone|celular|mobile|whatsapp|(^|_)tel($|_)", re.I)),
    ("card", re.compile(r"(^|_)(card|cartao|pan|cc_?number)($|_)", re.I)),
    ("birth", re.compile(r"birth|nascimento|(^|_)dob($|_)|data_nasc", re.I)),
    ("address", re.compile(r"address|endereco|logradouro|street|(^|_)rua($|_)|(^|_)(cep|zip|postal)(_code)?($|_)|complemento|bairro", re.I)),
    ("ip", re.compile(r"(^|_)ip(_?addr(ess)?)?($|_)|remote_addr", re.I)),
    ("name", re.compile(r"^(nome|name|first_?name|last_?name|full_?name|surname|sobrenome|display_?name|nome_\w+|\w+_nome|mae|pai|mother|father|nome_social)$", re.I)),
]
EMAIL_RE = re.compile(r"([A-Za-z0-9._%+-])[A-Za-z0-9._%+-]*@([A-Za-z0-9])[A-Za-z0-9.-]*(\.[A-Za-z]{2,})")
CPF_RE = re.compile(r"\b\d{3}\.\d{3}\.\d{3}-\d{2}\b")
CNPJ_RE = re.compile(r"\b\d{2}\.\d{3}\.\d{3}/\d{4}-\d{2}\b")
CARD_RE = re.compile(r"\b(?:\d[ -]?){13,19}\b")


def luhn(digits: str) -> bool:
    total, alt = 0, False
    for d in reversed(digits):
        n = int(d)
        if alt:
            n = n * 2 - 9 if n > 4 else n * 2
        total, alt = total + n, not alt
    return total % 10 == 0


def keep_last(value: str, keep: int) -> str:
    digits = sum(ch.isdigit() for ch in value)
    seen = 0
    out = []
    for ch in value:
        if ch.isdigit():
            seen += 1
            out.append(ch if seen > digits - keep else "*")
        else:
            out.append(ch)
    return "".join(out)


def mask_value(kind: str, value):
    if value is None:
        return None
    s = str(value)
    if kind == "secret":
        return "[redacted]"
    if kind == "email":
        return EMAIL_RE.sub(lambda m: f"{m.group(1)}***@{m.group(2)}***{m.group(3)}", s) if "@" in s else "***"
    if kind in ("document", "phone"):
        return keep_last(s, 2)
    if kind == "card":
        return keep_last(s, 4)
    if kind == "birth":
        m = re.match(r"(\d{4})", s)
        return f"{m.group(1)}-**-**" if m else "****"
    if kind == "address":
        return "[masked]"
    if kind == "ip":
        return re.sub(r"(\d+\.\d+)\.\d+\.\d+", r"\1.*.*", s) if "." in s else "[masked]"
    if kind == "name":
        return " ".join(w[0] + "***" for w in s.split() if w) or "***"
    return s


def mask_free_text(s: str) -> str:
    s = EMAIL_RE.sub(lambda m: f"{m.group(1)}***@{m.group(2)}***{m.group(3)}", s)
    s = CPF_RE.sub(lambda m: keep_last(m.group(0), 2), s)
    s = CNPJ_RE.sub(lambda m: keep_last(m.group(0), 2), s)

    def card(m):
        digits = re.sub(r"\D", "", m.group(0))
        return keep_last(m.group(0), 4) if 13 <= len(digits) <= 19 and luhn(digits) else m.group(0)

    return CARD_RE.sub(card, s)


def mask_result(columns: list[str], rows: list[list], reveal: bool = False) -> tuple[list[list], dict[str, str]]:
    """Mask personal data; with reveal, only secrets stay redacted (they are never shown)."""
    kinds: dict[int, str] = {}
    for i, col in enumerate(columns):
        base = col.split(".")[-1].strip('"`[]').lower()
        for kind, pattern in MASK_RULES:
            if pattern.search(base):
                kinds[i] = kind
                break
    if reveal:
        kinds = {i: k for i, k in kinds.items() if k == "secret"}
    masked = []
    for row in rows:
        new = []
        for i, v in enumerate(row):
            if reveal and i not in kinds:
                new.append(v)
            elif i in kinds:
                new.append(mask_value(kinds[i], v))
            elif isinstance(v, str):
                new.append(mask_free_text(v))
            else:
                new.append(v)
        masked.append(new)
    return masked, {columns[i]: k for i, k in kinds.items()}


# ------------------------------------------------------------ redaction

REDACT: list[tuple[re.Pattern, str]] = []


def remember_secrets(p: dict) -> None:
    """Host, user and password of the profile in use are redacted from every
    message and log entry; driver errors often quote the host or the user."""
    for key, label in (("password", "<password>"), ("host", "<host>"), ("user", "<user>")):
        value = (p.get(key) or "").strip()
        if len(value) >= 2 and value not in ("127.0.0.1", "localhost"):
            REDACT.append((re.compile(r"(?<![\w.-])" + re.escape(value) + r"(?![\w-])"), label))


def redact(text: str) -> str:
    for pattern, label in REDACT:
        text = pattern.sub(label, text)
    return text


# ------------------------------------------------------------------ logging


def log_path() -> Path:
    d = state_dir() / "log"
    d.mkdir(parents=True, exist_ok=True, mode=0o700)
    return d / f"{dt.date.today().isoformat()}_{repo_root().name}.md"


def log(entry: str) -> None:
    path = log_path()
    fd = os.open(path, os.O_WRONLY | os.O_APPEND | os.O_CREAT, 0o600)
    with os.fdopen(fd, "a") as f:
        f.write(redact(entry).rstrip("\n") + "\n\n")


def log_start(action: str, p: dict, cls: dict, reason: str, statements: list[str], extra: str = "") -> str:
    entry_id = uuid.uuid4().hex[:8]
    now = dt.datetime.now().astimezone().isoformat(timespec="seconds")
    body = "\n\n".join(statements)
    log(
        f"## {now} · {action} · {p['name']} · {entry_id}\n\n"
        f"- engine: {p['engine']} · class: {cls['class']} · writable: {str(cls['writable']).lower()} · via: {cls['via']}\n"
        f"- evidence: {'; '.join(cls['evidence'])}\n"
        f"- reason: {reason}\n"
        + (f"- {extra}\n" if extra else "")
        + (f"\n```sql\n{body}\n```\n" if statements else "")
        + "\n- status: started"
    )
    return entry_id


def log_result(entry_id: str, text: str) -> None:
    log(f"- {entry_id} result: {text}")


# -------------------------------------------------------------- executor


def to_jsonable(v):
    if v is None or isinstance(v, (bool, int, float, str)):
        return v
    if isinstance(v, (dt.date, dt.datetime, dt.time)):
        return v.isoformat()
    if isinstance(v, decimal.Decimal):
        return str(v)
    if isinstance(v, (bytes, bytearray, memoryview)):
        return f"<{len(bytes(v))} bytes>"
    return str(v)


def connect(req: dict):
    eng, c, timeout_ms, read = req["engine"], req["conn"], int(req["timeout"] * 1000), req["mode"] != "write"
    if eng == "postgresql":
        import psycopg  # noqa: PLC0415

        opts = f"-c statement_timeout={timeout_ms} -c lock_timeout={LOCK_TIMEOUT_MS} -c idle_in_transaction_session_timeout=60000"
        if read:
            opts += " -c default_transaction_read_only=on"
        conn = psycopg.connect(host=c.get("host"), port=int(c.get("port") or 5432), dbname=c.get("database"),
                               user=c.get("user"), password=c.get("password"), connect_timeout=10,
                               application_name="ai-gent-dbrun", options=opts)
        return conn
    if eng in ("mysql", "mariadb"):
        import pymysql  # noqa: PLC0415

        conn = pymysql.connect(host=c.get("host"), port=int(c.get("port") or 3306), user=c.get("user"),
                               password=c.get("password") or "", database=c.get("database") or None,
                               connect_timeout=10, read_timeout=req["timeout"] + 5, write_timeout=req["timeout"] + 5,
                               autocommit=False, program_name="ai-gent-dbrun")
        cur = conn.cursor()
        cur.execute("SELECT VERSION()")
        is_maria = "mariadb" in str(cur.fetchone()[0]).lower()
        if is_maria:
            cur.execute(f"SET SESSION max_statement_time = {req['timeout']}")
        else:
            cur.execute(f"SET SESSION MAX_EXECUTION_TIME = {timeout_ms}")
        cur.execute(f"SET SESSION innodb_lock_wait_timeout = {LOCK_TIMEOUT_MS // 1000}")
        cur.execute(f"SET SESSION lock_wait_timeout = {LOCK_TIMEOUT_MS // 1000}")
        if read:
            cur.execute("SET SESSION TRANSACTION READ ONLY")
        return conn
    if eng == "sqlserver":
        import pymssql  # noqa: PLC0415

        conn = pymssql.connect(server=c.get("host"), port=str(c.get("port") or 1433), user=c.get("user"),
                               password=c.get("password"), database=c.get("database") or "master",
                               login_timeout=10, timeout=req["timeout"], appname="ai-gent-dbrun", autocommit=False)
        cur = conn.cursor()
        cur.execute(f"SET LOCK_TIMEOUT {LOCK_TIMEOUT_MS}; SET TRANSACTION ISOLATION LEVEL READ COMMITTED; SET NOCOUNT ON")
        return conn
    if eng == "oracle":
        import oracledb  # noqa: PLC0415

        conn = oracledb.connect(user=c.get("user"), password=c.get("password"),
                                dsn=f"{c.get('host')}:{c.get('port') or 1521}/{c.get('service') or c.get('database')}",
                                tcp_connect_timeout=10)
        conn.call_timeout = timeout_ms
        return conn
    if eng == "sqlite":
        import sqlite3  # noqa: PLC0415

        path = c["path"]
        uri = f"file:{path}?mode=ro" if read else f"file:{path}"
        conn = sqlite3.connect(uri, uri=True, timeout=LOCK_TIMEOUT_MS / 1000, isolation_level=None)
        deadline = time.monotonic() + req["timeout"]
        conn.set_progress_handler(lambda: 1 if time.monotonic() > deadline else 0, 10000)
        if read:
            conn.execute("PRAGMA query_only = ON")
        return conn
    raise Failed(f"unsupported engine {eng}")


IDENTITY = {
    "postgresql": "SELECT version(), COALESCE(inet_server_addr()::text, 'socket'), current_database()",
    "mysql": "SELECT VERSION(), @@hostname, DATABASE()",
    "mariadb": "SELECT VERSION(), @@hostname, DATABASE()",
    "sqlserver": "SELECT @@VERSION, CAST(SERVERPROPERTY('MachineName') AS nvarchar(256)), DB_NAME()",
    "oracle": "SELECT SYS_CONTEXT('USERENV','DB_NAME'), SYS_CONTEXT('USERENV','SERVER_HOST'), SYS_CONTEXT('USERENV','CON_NAME') FROM DUAL",
    "sqlite": "SELECT 'SQLite ' || sqlite_version(), 'file', 'main'",
}
LINKS = {
    "postgresql": "SELECT (SELECT count(*) FROM pg_extension WHERE extname IN ('postgres_fdw','dblink','mysql_fdw','oracle_fdw','tds_fdw','mongo_fdw','file_fdw')) + (SELECT count(*) FROM pg_foreign_server)",
    "mysql": "SELECT COUNT(*) FROM information_schema.TABLES WHERE ENGINE = 'FEDERATED'",
    "mariadb": "SELECT COUNT(*) FROM information_schema.TABLES WHERE ENGINE IN ('FEDERATED', 'CONNECT')",
    "sqlserver": "SELECT COUNT(*) FROM sys.servers WHERE is_linked = 1",
    "oracle": "SELECT COUNT(*) FROM ALL_DB_LINKS",
    "sqlite": "SELECT COUNT(*) - 1 FROM pragma_database_list",
}


def explain_sql(engine: str, stmt: str) -> str:
    return {"sqlite": "EXPLAIN QUERY PLAN "}.get(engine, "EXPLAIN ") + stmt


def execute(req: dict) -> dict:
    """Runs inside the driver process. Never raises: returns {ok, ...}."""
    eng = req["engine"]
    started = time.monotonic()
    try:
        conn = connect(req)
    except Exception as e:  # noqa: BLE001
        return {"ok": False, "stage": "connect", "error": f"{type(e).__name__}: {e}"}
    result = {"ok": True, "results": []}
    try:
        cur = conn.cursor()
        cur.execute(IDENTITY[eng])
        ident = cur.fetchone()
        result["server"] = [to_jsonable(x) for x in ident]
        cur.execute(LINKS[eng])
        result["links"] = int(cur.fetchone()[0] or 0)
        if eng in ("postgresql", "mysql", "mariadb", "oracle", "sqlserver"):
            conn.rollback()
        if req["mode"] == "test":
            return result
        read = req["mode"] != "write"
        if read and eng == "oracle":
            cur.execute("SET TRANSACTION READ ONLY")
        if read and eng in ("mysql", "mariadb"):
            cur.execute("START TRANSACTION READ ONLY")
        if not read and eng == "sqlite":
            cur.execute("BEGIN")
        if eng == "sqlserver" and req.get("explain"):
            cur.execute("SET SHOWPLAN_TEXT ON")
        for stmt in req["statements"]:
            sql = explain_sql(eng, stmt) if req.get("explain") and eng != "sqlserver" else stmt
            cur.execute(sql)
            entry = {"rowcount": cur.rowcount}
            if cur.description:
                entry["columns"] = [d[0] for d in cur.description]
                rows = cur.fetchmany(req["max_rows"] + 1) if read else cur.fetchall()
                entry["truncated"] = len(rows) > req["max_rows"] if read else False
                entry["rows"] = [[to_jsonable(v) for v in r] for r in rows[: req["max_rows"]]]
            result["results"].append(entry)
        if eng == "sqlserver" and req.get("explain"):
            cur.execute("SET SHOWPLAN_TEXT OFF")
        if read:
            conn.rollback() if eng != "sqlite" else None
        else:
            expect = req.get("expect") or []
            counts = [r["rowcount"] for r in result["results"]]
            mismatch = [(i + 1, e, counts[i]) for i, e in enumerate(expect) if e is not None and i < len(counts) and counts[i] != e]
            if mismatch:
                conn.rollback()
                result.update(ok=False, stage="verify", error="row counts differ from the plan: " + ", ".join(
                    f"statement {i}: expected {e}, got {g}" for i, e, g in mismatch))
            else:
                conn.commit()
                result["committed"] = True
    except Exception as e:  # noqa: BLE001
        try:
            conn.rollback()
        except Exception:  # noqa: BLE001
            pass
        result.update(ok=False, stage="execute", error=f"{type(e).__name__}: {e}")
    finally:
        try:
            conn.close()
        except Exception:  # noqa: BLE001
            pass
        result["seconds"] = round(time.monotonic() - started, 3)
    return result


def session_name(p: dict) -> str:
    return "ai-gent-dbrun-" + re.sub(r"[^a-z0-9-]", "-", p["name"].lower())


def driver_process(p: dict, cls: dict, req: dict) -> dict:
    """Run execute() in the right place and return its JSON result."""
    if p["engine"] == "sqlite":
        return execute(req)
    payload = json.dumps(req)
    driver = DRIVERS[p["engine"]]
    engine = engine_cmd()
    inner = ["uv", "run", "--no-project", "--quiet", "--with", driver, "python", "/opt/dbrun/dbrun.py", "_exec"]
    if cls["via"] == "container":
        req["conn"]["host"] = "127.0.0.1"
        req["conn"]["port"] = str(DEFAULT_PORTS[p["engine"]])
        payload = json.dumps(req)
        sess = session_name(p)
        if engine and run([engine, "inspect", "--format", "{{.State.Running}}", sess], timeout=20).stdout.strip() == "true":
            cmd = [engine, "exec", "-i", sess] + inner
        else:
            cmd = [engine, "run", "--rm", "-i", "--network", f"container:{cls['container_id']}",
                   "-v", f"{SCRIPT.parent}:/opt/dbrun:{ro_mount()}", "-v", "ai-gent-dbrun-uv-cache:/root/.cache/uv",
                   CLIENT_IMAGE] + inner
    elif shutil.which("uv"):
        cmd = ["uv", "run", "--no-project", "--quiet", "--with", driver, "python3", str(SCRIPT), "_exec"]
    elif engine:
        sess = session_name(p)
        if run([engine, "inspect", "--format", "{{.State.Running}}", sess], timeout=20).stdout.strip() == "true":
            cmd = [engine, "exec", "-i", sess] + inner
        else:
            cmd = [engine, "run", "--rm", "-i", "-v", f"{SCRIPT.parent}:/opt/dbrun:{ro_mount()}",
                   "-v", "ai-gent-dbrun-uv-cache:/root/.cache/uv", CLIENT_IMAGE] + inner
    else:
        raise Failed("neither uv nor a container engine is available to run the database driver; install uv "
                     "(https://docs.astral.sh/uv/) or Podman/Docker", EXIT_CONNECT)
    try:
        out = run(cmd, input=payload, timeout=req["timeout"] * max(1, len(req.get("statements") or [1])) + 180)
    except subprocess.TimeoutExpired:
        raise Failed("the driver process timed out", EXIT_CONNECT) from None
    try:
        return json.loads(out.stdout.strip().splitlines()[-1])
    except (json.JSONDecodeError, IndexError):
        err = (out.stderr or out.stdout).strip().splitlines()[-5:]
        raise Failed("driver process failed: " + " | ".join(err), EXIT_CONNECT) from None


def conn_of(p: dict) -> dict:
    return {k: p.get(k) for k in ("host", "port", "database", "service", "user", "password", "path")}


# ---------------------------------------------------------------- commands


def read_sql(arg: str) -> str:
    return sys.stdin.read() if arg == "-" else Path(arg).read_text()


def print_table(columns: list[str], rows: list[list], width: int = 120) -> None:
    def cell(v):
        s = "NULL" if v is None else str(v).replace("\n", "⏎")
        return s if len(s) <= width else s[: width - 1] + "…"

    table = [[cell(c) for c in columns]] + [[cell(v) for v in r] for r in rows]
    widths = [max(len(r[i]) for r in table) for i in range(len(columns))]
    print(" | ".join(h.ljust(w) for h, w in zip(table[0], widths)))
    print("-+-".join("-" * w for w in widths))
    for r in table[1:]:
        print(" | ".join(v.ljust(w) for v, w in zip(r, widths)))


def cmd_profiles(args) -> int:
    profiles = all_profiles()
    rows = [{"name": n, "engine": p.get("engine", "?"), "class": p.get("class"), "source": p.get("source")}
            for n, p in sorted(profiles.items())]
    if args.json:
        print(json.dumps(rows, indent=2))
    elif rows:
        print_table(["name", "engine", "declared class", "source"], [list(r.values()) for r in rows])
        print("\n(declared class; `dbrun classify <profile>` proves what it really is)")
    else:
        print(f"no profiles: none in {config_dir() / 'connections.env'} and no database containers of this repository running")
    return EXIT_OK


def cmd_classify(args) -> int:
    p = get_profile(args.profile)
    cls = classify(p)
    if args.json:
        print(json.dumps(cls, indent=2))
    else:
        print(f"{p['name']}: {cls['class']}{' (writable after a confirmed plan)' if cls['writable'] else ' (read only)'}")
        for e in cls["evidence"]:
            print(f"  - {e}")
    return EXIT_OK


def base_request(p: dict, mode: str, statements: list[str], args) -> dict:
    return {"engine": p["engine"], "conn": conn_of(p), "mode": mode, "statements": statements,
            "timeout": getattr(args, "timeout", 30), "max_rows": getattr(args, "max_rows", 100),
            "explain": getattr(args, "explain", False)}


def check_links(cls: dict, res: dict) -> None:
    if cls["writable"] and res.get("links"):
        cls["writable"] = False
        cls["evidence"].append(f"NOT PROVEN LOCAL: the database has {res['links']} link(s) to other databases")


def cmd_test(args) -> int:
    p = get_profile(args.profile)
    cls = classify(p)
    entry = log_start("test", p, cls, args.reason or "connection test", [])
    res = driver_process(p, cls, base_request(p, "test", [], args))
    if not res.get("ok"):
        log_result(entry, f"FAILED at {res.get('stage')}: {res.get('error')}")
        print(redact(f"FAILED ({res.get('stage')}): {res.get('error')}"))
        return EXIT_CONNECT
    check_links(cls, res)
    log_result(entry, f"SUCCESS · server {res['server'][0][:60]} · links {res['links']} · {res['seconds']} s")
    print(f"SUCCESS · {cls['class']} · {res['server'][0][:80]}")
    if res["links"]:
        print(f"  warning: {res['links']} link(s) to other databases — writes refused even if local")
    return EXIT_OK


def cmd_query(args) -> int:
    p = get_profile(args.profile)
    cls = classify(p)
    statements = split_statements(read_sql(args.sql), p["engine"])
    if not statements:
        raise Failed("no statements")
    for i, stmt in enumerate(statements, 1):
        kind, why = classify_statement(stmt, p["engine"])
        if kind != "read":
            raise Refused(f"statement {i} is not read-only ({why}). `query` only reads; writes go through "
                          f"`plan`/`apply` on a proven local database, or `script` for a person to run elsewhere")
        rule = cost_rules(stmt, cls["class"], args.allow_full_count, p["engine"])
        if rule:
            raise Refused(f"statement {i}: {rule}")
    if args.explain and p["engine"] == "oracle" and cls["class"] != "local":
        raise Refused("Oracle's EXPLAIN PLAN writes to PLAN_TABLE; not allowed on a remote database")
    extra = ("reveal: yes (the user asked)" if args.reveal else "masked") + (" · explain" if args.explain else "")
    entry = log_start("query", p, cls, args.reason, statements, extra)
    res = driver_process(p, cls, base_request(p, "read", statements, args))
    if not res.get("ok"):
        log_result(entry, f"FAILED at {res.get('stage')}: {res.get('error')}")
        print(redact(f"FAILED ({res.get('stage')}): {res.get('error')}"), file=sys.stderr)
        return EXIT_CONNECT if res.get("stage") == "connect" else EXIT_ERROR
    summary = []
    for i, r in enumerate(res["results"], 1):
        if "columns" not in r:
            summary.append(f"#{i}: no result set")
            continue
        rows, masked = mask_result(r["columns"], r["rows"], reveal=args.reveal)
        summary.append(f"#{i}: {len(rows)} row(s){' (truncated)' if r['truncated'] else ''}"
                       + (f", masked {len(masked)} column(s)" if masked else ""))
        if len(res["results"]) > 1:
            print(f"-- statement {i}")
        if args.format == "json":
            print(json.dumps({"columns": r["columns"], "rows": rows, "truncated": r["truncated"], "masked": masked}))
        elif args.format == "csv":
            import csv  # noqa: PLC0415

            w = csv.writer(sys.stdout)
            w.writerow(r["columns"])
            w.writerows(rows)
        else:
            print_table(r["columns"], rows)
            notes = [f"{len(rows)} row(s)"]
            if r["truncated"]:
                notes.append(f"truncated at --max-rows {args.max_rows}")
            if masked:
                notes.append("masked: " + ", ".join(f"{c} ({k})" for c, k in masked.items()))
            print(f"({'; '.join(notes)})")
    log_result(entry, f"ok · {'; '.join(summary)} · {res['seconds']} s")
    return EXIT_OK


def plans_dir() -> Path:
    d = state_dir() / "plans"
    d.mkdir(parents=True, exist_ok=True, mode=0o700)
    return d


def require_local(p: dict, cls: dict, action: str) -> None:
    if not cls["writable"]:
        raise Refused(f"{action} needs a proven local database; {p['name']} is {cls['class']} "
                      f"({'; '.join(cls['evidence'])}). For this database, write a change script with "
                      f"`dbrun script` for a person to run")


BACKUP_SUPPORT = {"postgresql": True, "mysql": True, "mariadb": True, "sqlite": True, "sqlserver": True, "oracle": False}


def cmd_plan(args) -> int:
    p = get_profile(args.profile)
    cls = classify(p)
    require_local(p, cls, "plan")
    statements = split_statements(read_sql(args.sql), p["engine"])
    if not statements:
        raise Failed("no statements")
    ddl = [i + 1 for i, s in enumerate(statements) if is_ddl(s, p["engine"])]
    if ddl and not BACKUP_SUPPORT[p["engine"]]:
        raise Refused(f"statements {ddl} are DDL; dbrun can't back up a local {p['engine']} database, so only DML "
                      f"inside a transaction is allowed. Use the project's migration tool, or take a backup yourself")
    expect = [int(x) if x.strip().lstrip("-").isdigit() else None for x in args.expect.split(",")] if args.expect else []
    plan_id = dt.datetime.now().strftime("%Y%m%d-%H%M%S-") + uuid.uuid4().hex[:4]
    plan = {
        "id": plan_id, "profile": p["name"], "engine": p["engine"], "reason": args.reason,
        "created": dt.datetime.now().astimezone().isoformat(timespec="seconds"),
        "statements": statements, "expect": expect,
        "ddl": ddl, "backup": "required" if BACKUP_SUPPORT[p["engine"]] else "none (DML in one transaction)",
        "sha256": hashlib.sha256("\n;\n".join(statements).encode()).hexdigest(),
        "evidence": cls["evidence"],
    }
    path = plans_dir() / f"{plan_id}.json"
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(fd, "w") as f:
        json.dump(plan, f, indent=2)
    entry = log_start("plan", p, cls, args.reason, statements, f"plan {plan_id}")
    log_result(entry, f"plan recorded at {path}")
    print(f"Plan {plan_id} — {p['name']} ({p['engine']}, proven local)\n")
    print(f"Reason: {args.reason}")
    for i, s in enumerate(statements, 1):
        exp = expect[i - 1] if i <= len(expect) and expect[i - 1] is not None else "not given"
        print(f"\n{i}. expected rows: {exp}{'  [DDL]' if i in ddl else ''}\n{s}")
    print(f"\nBackup before applying: {plan['backup']}. Everything runs in one transaction; "
          f"it commits only if every expected row count matches.")
    print(f"Restore afterwards: dbrun restore {p['name']} {plan_id} --confirmed")
    print(f"\nShow this plan to the user. Only after they confirm: dbrun apply {p['name']} {plan_id} --confirmed")
    return EXIT_OK


def load_plan(p: dict, plan_id: str) -> dict:
    if not re.fullmatch(r"[0-9]{8}-[0-9]{6}-[0-9a-f]{4}", plan_id):
        raise Failed(f"not a plan id: {plan_id}")
    path = plans_dir() / f"{plan_id}.json"
    if not path.exists():
        raise Failed(f"no plan {plan_id}")
    plan = json.loads(path.read_text())
    if plan["profile"] != p["name"]:
        raise Refused(f"plan {plan_id} was made for {plan['profile']}, not {p['name']}")
    if hashlib.sha256("\n;\n".join(plan["statements"]).encode()).hexdigest() != plan["sha256"]:
        raise Refused(f"plan {plan_id} was modified after it was recorded")
    return plan


def backups_dir() -> Path:
    d = state_dir() / "backups"
    d.mkdir(parents=True, exist_ok=True, mode=0o700)
    return d


def container_shell(p: dict, script: str, *, stdin: bytes, out_path: Path | None = None) -> None:
    """Run `sh -c script` in the local DB container; the password arrives on stdin's first line."""
    engine = engine_cmd()
    cmd = [engine, "exec", "-i", p["container"], "sh", "-c", script, "sh", p.get("user") or "", p.get("database") or ""]
    proc = subprocess.run(cmd, input=stdin, capture_output=True, timeout=1800)
    if proc.returncode != 0:
        raise Failed(f"backup/restore command failed: {proc.stderr.decode(errors='replace').strip()[-400:]}")
    if out_path is not None:
        fd = os.open(out_path, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
        with os.fdopen(fd, "wb") as f:
            f.write(proc.stdout)


def backup(p: dict, cls: dict, plan: dict) -> str:
    eng = p["engine"]
    pw = (p.get("password") or "").encode() + b"\n"
    if eng == "sqlite":
        src = Path(p["path"]).resolve()
        dst = backups_dir() / f"{plan['id']}.sqlite"
        import sqlite3  # noqa: PLC0415

        with sqlite3.connect(f"file:{src}?mode=ro", uri=True) as a, sqlite3.connect(dst) as b:
            a.backup(b)
        os.chmod(dst, 0o600)
        return str(dst)
    if eng == "postgresql":
        dst = backups_dir() / f"{plan['id']}.pgdump"
        container_shell(p, 'read -r PGPASSWORD; export PGPASSWORD; exec pg_dump -Fc -h 127.0.0.1 -U "$1" -d "$2"', stdin=pw, out_path=dst)
        return str(dst)
    if eng in ("mysql", "mariadb"):
        dst = backups_dir() / f"{plan['id']}.sql"
        dump = "mariadb-dump" if eng == "mariadb" else "mysqldump"
        container_shell(p, f'read -r MYSQL_PWD; export MYSQL_PWD; d=$(command -v {dump} || command -v mysqldump); '
                           f'exec "$d" --single-transaction --routines --triggers --events -h 127.0.0.1 -u "$1" "$2"',
                        stdin=pw, out_path=dst)
        return str(dst)
    if eng == "sqlserver":
        name = f"/var/opt/mssql/data/ai-gent-{plan['id']}.bak"
        db = p.get("database") or "master"
        req = base_request(p, "write", [f"BACKUP DATABASE [{db}] TO DISK = N'{name}' WITH INIT, COPY_ONLY"], argparse.Namespace())
        req["autocommit"] = True
        res = sqlserver_autocommit(p, cls, req)
        if not res.get("ok"):
            raise Failed(f"backup failed: {res.get('error')}")
        return f"container:{name}"
    raise Refused(f"no backup method for {eng}")


def sqlserver_autocommit(p: dict, cls: dict, req: dict) -> dict:
    """BACKUP/RESTORE can't run inside a transaction: run through a one-off autocommit script."""
    code = ("import json,sys,pymssql;r=json.load(sys.stdin);c=r['conn'];"
            "k=pymssql.connect(server=c['host'],port=str(c['port']),user=c['user'],password=c['password'],database='master',"
            "login_timeout=10,autocommit=True);cur=k.cursor()\n"
            "try:\n [cur.execute(s) for s in r['statements']];print(json.dumps({'ok':True}))\n"
            "except Exception as e:\n print(json.dumps({'ok':False,'error':str(e)}))")
    engine = engine_cmd()
    req["conn"].update(host="127.0.0.1", port=str(DEFAULT_PORTS["sqlserver"]))
    cmd = [engine, "run", "--rm", "-i", "--network", f"container:{cls['container_id']}",
           "-v", "ai-gent-dbrun-uv-cache:/root/.cache/uv", CLIENT_IMAGE,
           "uv", "run", "--no-project", "--quiet", "--with", "pymssql", "python", "-c", code]
    out = run(cmd, input=json.dumps(req), timeout=1800)
    try:
        return json.loads(out.stdout.strip().splitlines()[-1])
    except (json.JSONDecodeError, IndexError):
        return {"ok": False, "error": (out.stderr or "no output").strip()[-300:]}


def cmd_apply(args) -> int:
    if not args.confirmed:
        raise Refused("apply needs --confirmed, which means the user confirmed this exact plan")
    p = get_profile(args.profile)
    cls = classify(p)  # re-proven now, not trusted from plan time
    require_local(p, cls, "apply")
    plan = load_plan(p, args.plan_id)
    probe = driver_process(p, cls, base_request(p, "test", [], argparse.Namespace(timeout=30, max_rows=1)))
    if not probe.get("ok"):
        raise Failed(f"can't connect: {probe.get('error')}", EXIT_CONNECT)
    check_links(cls, probe)
    require_local(p, cls, "apply")
    entry = log_start("apply", p, cls, plan["reason"], plan["statements"], f"plan {plan['id']}")
    location = "none"
    if BACKUP_SUPPORT[p["engine"]]:
        location = backup(p, cls, plan)
        log_result(entry, f"backup taken: {location}")
    plan["backup_location"] = location
    (plans_dir() / f"{plan['id']}.json").write_text(json.dumps(plan, indent=2))
    req = base_request(p, "write", plan["statements"], argparse.Namespace(timeout=args.timeout, max_rows=1000))
    req["expect"] = plan["expect"]
    res = driver_process(p, cls, req)
    if not res.get("ok"):
        log_result(entry, f"ROLLED BACK at {res.get('stage')}: {res.get('error')}")
        print(redact(f"ROLLED BACK ({res.get('stage')}): {res.get('error')}"))
        if plan["ddl"] and p["engine"] in ("mysql", "mariadb"):
            print(f"MySQL commits DDL immediately: check the state, and restore with `dbrun restore {p['name']} {plan['id']} --confirmed` if needed")
        return EXIT_ERROR
    counts = [r["rowcount"] for r in res["results"]]
    log_result(entry, f"COMMITTED · row counts {counts} · {res['seconds']} s")
    print(f"COMMITTED plan {plan['id']} · row counts {counts}")
    print(f"Backup: {location}. To undo: dbrun restore {p['name']} {plan['id']} --confirmed")
    return EXIT_OK


def cmd_restore(args) -> int:
    if not args.confirmed:
        raise Refused("restore needs --confirmed, which means the user asked for this restore")
    p = get_profile(args.profile)
    cls = classify(p)
    require_local(p, cls, "restore")
    plan = load_plan(p, args.plan_id)
    location = plan.get("backup_location")
    if not location or location == "none":
        raise Failed(f"plan {plan['id']} has no backup to restore")
    entry = log_start("restore", p, cls, f"restore plan {plan['id']}", [], f"from {location}")
    eng = p["engine"]
    pw = (p.get("password") or "").encode() + b"\n"
    if eng == "sqlite":
        import sqlite3  # noqa: PLC0415

        with sqlite3.connect(location) as a, sqlite3.connect(Path(p["path"]).resolve()) as b:
            a.backup(b)
    elif eng == "postgresql":
        container_shell(p, 'read -r PGPASSWORD; export PGPASSWORD; exec pg_restore --clean --if-exists --no-owner -h 127.0.0.1 -U "$1" -d "$2"',
                        stdin=pw + Path(location).read_bytes())
    elif eng in ("mysql", "mariadb"):
        container_shell(p, 'read -r MYSQL_PWD; export MYSQL_PWD; c=$(command -v mariadb || command -v mysql); exec "$c" -h 127.0.0.1 -u "$1" "$2"',
                        stdin=pw + Path(location).read_bytes())
    elif eng == "sqlserver":
        db = p.get("database") or "master"
        name = location.split(":", 1)[1]
        req = base_request(p, "write", [f"ALTER DATABASE [{db}] SET SINGLE_USER WITH ROLLBACK IMMEDIATE",
                                        f"RESTORE DATABASE [{db}] FROM DISK = N'{name}' WITH REPLACE",
                                        f"ALTER DATABASE [{db}] SET MULTI_USER"], argparse.Namespace())
        res = sqlserver_autocommit(p, cls, req)
        if not res.get("ok"):
            raise Failed(f"restore failed: {res.get('error')}")
    log_result(entry, "restored")
    print(f"RESTORED {p['name']} from {location}")
    return EXIT_OK


def cmd_script(args) -> int:
    p = get_profile(args.profile)
    cls = classify(p)
    statements = split_statements(read_sql(args.sql), p["engine"])
    if not statements:
        raise Failed("no statements")
    out = Path(args.out)
    if out.exists():
        raise Failed(f"{out} exists; choose another --out")
    for i, stmt in enumerate(statements, 1):
        words = [w.upper() for w in re.findall(r"[A-Za-z_]+", blank_literals(stmt, p["engine"]))]
        if words and words[0] in ("UPDATE", "DELETE") and "WHERE" not in words:
            raise Refused(f"statement {i} is an {words[0]} without WHERE; a change script names the rows it changes")
    now = dt.datetime.now().astimezone().isoformat(timespec="seconds")
    kinds = [classify_statement(s, p["engine"])[0] for s in statements]
    begin, commit = {
        "postgresql": ("BEGIN;", "COMMIT;"), "sqlite": ("BEGIN;", "COMMIT;"),
        "mysql": ("START TRANSACTION;", "COMMIT;"), "mariadb": ("START TRANSACTION;", "COMMIT;"),
        "sqlserver": ("BEGIN TRANSACTION;", "COMMIT TRANSACTION;"), "oracle": ("", "COMMIT;"),
    }[p["engine"]]
    ddl_note = ("-- Note: this engine commits DDL implicitly; the backup is the rollback for DDL statements."
                if p["engine"] in ("mysql", "mariadb", "oracle") and any(is_ddl(s, p["engine"]) for s in statements) else "")
    header = [
        f"-- Change script for {p['name']} ({p['engine']}, {cls['class']})",
        f"-- Prepared by ai-gent dbrun on {now}. NOT executed: a person reviews and runs it.",
        f"-- Reason: {args.reason}",
        f"-- Statements: {len(statements)} ({kinds.count('write')} write, {kinds.count('read')} read)",
        "--",
        "-- Before running: take a backup, run the verification queries below and compare the counts,",
        "-- run inside a transaction where the engine allows it, and keep the rollback section at hand.",
        "",
        "-- == Pre-checks (run first; each must return the expected count) ==",
        "-- <SELECT COUNT(*) ... WHERE <the same condition as each change>  -- expected: N>",
        "",
        "-- == Change ==",
    ] + ([ddl_note] if ddl_note else []) + ([begin] if begin else [])
    body = [s.rstrip().rstrip(";") + ";" for s in statements] + [
        "-- Compare the affected row counts with the pre-checks; ROLLBACK instead of COMMIT if they differ.",
        commit,
    ]
    footer = ["", "-- == Verification (after the change) ==", "-- <SELECTs that prove it worked>", "",
              "-- == Rollback ==", "-- <statements that undo the change, or the backup to restore — take it first>", ""]
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text("\n".join(header + body + footer))
    entry = log_start("script", p, cls, args.reason, statements, f"written to {out} — not executed")
    log_result(entry, "script written, not executed")
    print(f"Wrote {out} (not executed). Fill in the verification and rollback sections, then hand it over.")
    return EXIT_OK


def cmd_session(args) -> int:
    p = get_profile(args.profile)
    cls = classify(p)
    engine = engine_cmd()
    name = session_name(p)
    if args.action == "status":
        state = run([engine, "inspect", "--format", "{{.State.Running}}", name], timeout=20).stdout.strip() if engine else ""
        print(f"{name}: {'running' if state == 'true' else 'not running'}")
        return EXIT_OK
    if args.action == "stop":
        if engine:
            run([engine, "rm", "-f", name], timeout=60)
        log_result(log_start("session stop", p, cls, "end of investigation", []), "stopped")
        print(f"stopped {name}")
        return EXIT_OK
    if p["engine"] == "sqlite":
        print("SQLite runs in-process; no session needed")
        return EXIT_OK
    if not engine:
        print("no container engine; queries run through uv on the host, which caches the driver — no session needed")
        return EXIT_OK
    network = ["--network", f"container:{cls['container_id']}"] if cls["via"] == "container" else []
    out = run([engine, "run", "-d", "--name", name, *network, "-v", f"{SCRIPT.parent}:/opt/dbrun:{ro_mount()}",
               "-v", "ai-gent-dbrun-uv-cache:/root/.cache/uv", CLIENT_IMAGE, "sleep", "infinity"], timeout=300)
    if out.returncode != 0:
        raise Failed(f"couldn't start the session container: {out.stderr.strip()[-300:]}")
    log_result(log_start("session start", p, cls, args.reason or "investigation", []), f"started {name}")
    print(f"started {name}; queries for {p['name']} now reuse it. Stop it with: dbrun session stop {p['name']}")
    return EXIT_OK


def cmd_log(args) -> int:
    path = log_path()
    print(f"log: {path}")
    if path.exists():
        entries = path.read_text().split("\n## ")
        for e in entries[-args.tail:]:
            print(("## " if not e.startswith("## ") else "") + e.strip() + "\n")
    return EXIT_OK


def cmd_exec(_args) -> int:
    req = json.load(sys.stdin)
    print(json.dumps(execute(req)))
    return EXIT_OK


def build_parser() -> argparse.ArgumentParser:
    ap = argparse.ArgumentParser(prog="dbrun", description="The db plugin's safe path to databases (see references/runner.md).")
    sub = ap.add_subparsers(dest="command", required=True)

    s = sub.add_parser("profiles", help="list connection profiles (never credentials)")
    s.add_argument("--json", action="store_true")
    s.set_defaults(func=cmd_profiles)

    s = sub.add_parser("classify", help="prove what a profile really is")
    s.add_argument("profile")
    s.add_argument("--json", action="store_true")
    s.set_defaults(func=cmd_classify)

    s = sub.add_parser("test", help="connect and run a trivial query")
    s.add_argument("profile")
    s.add_argument("--reason", default="")
    s.add_argument("--timeout", type=int, default=30)
    s.set_defaults(func=cmd_test)

    s = sub.add_parser("query", help="run read-only statements")
    s.add_argument("profile")
    s.add_argument("--sql", required=True, help="file with the statements, or - for stdin")
    s.add_argument("--reason", required=True)
    s.add_argument("--max-rows", type=int, default=100)
    s.add_argument("--timeout", type=int, default=30)
    s.add_argument("--reveal", action="store_true", help="unmasked values — only when the user asked")
    s.add_argument("--format", choices=("table", "csv", "json"), default="table")
    s.add_argument("--explain", action="store_true", help="plain plan, never EXPLAIN ANALYZE")
    s.add_argument("--allow-full-count", action="store_true", help="an exact unfiltered count the user asked for")
    s.set_defaults(func=cmd_query)

    s = sub.add_parser("plan", help="record a write plan (proven local only)")
    s.add_argument("profile")
    s.add_argument("--sql", required=True)
    s.add_argument("--reason", required=True)
    s.add_argument("--expect", help="expected affected rows per statement, comma-separated (blank = unchecked)")
    s.set_defaults(func=cmd_plan)

    s = sub.add_parser("apply", help="back up and apply a confirmed plan (proven local only)")
    s.add_argument("profile")
    s.add_argument("plan_id")
    s.add_argument("--confirmed", action="store_true", help="the user confirmed this exact plan")
    s.add_argument("--timeout", type=int, default=120)
    s.set_defaults(func=cmd_apply)

    s = sub.add_parser("restore", help="restore the backup taken by apply (proven local only)")
    s.add_argument("profile")
    s.add_argument("plan_id")
    s.add_argument("--confirmed", action="store_true")
    s.set_defaults(func=cmd_restore)

    s = sub.add_parser("script", help="write a change script for a person to run; never executes")
    s.add_argument("profile")
    s.add_argument("--sql", required=True)
    s.add_argument("--reason", required=True)
    s.add_argument("--out", required=True)
    s.set_defaults(func=cmd_script)

    s = sub.add_parser("session", help="warm client container for an investigation")
    s.add_argument("action", choices=("start", "stop", "status"))
    s.add_argument("profile")
    s.add_argument("--reason", default="")
    s.set_defaults(func=cmd_session)

    s = sub.add_parser("log", help="show the command log")
    s.add_argument("--tail", type=int, default=5)
    s.set_defaults(func=cmd_log)

    s = sub.add_parser("_exec", help=argparse.SUPPRESS)
    s.set_defaults(func=cmd_exec)
    return ap


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        return args.func(args)
    except Refused as e:
        print(redact(f"REFUSED: {e}"), file=sys.stderr)
        return EXIT_REFUSED
    except Failed as e:
        print(redact(f"error: {e}"), file=sys.stderr)
        return e.code
    except KeyboardInterrupt:
        return EXIT_ERROR


if __name__ == "__main__":
    sys.exit(main())
