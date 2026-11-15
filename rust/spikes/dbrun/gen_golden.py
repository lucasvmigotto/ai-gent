import ast, json, sys, os
from pathlib import Path
import inspect
import re
import unittest
from unittest.mock import patch
from contextlib import ExitStack
ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
sys.dont_write_bytecode = True
sys.path.insert(0, os.path.join(ROOT, "plugins/db/scripts"))
import dbrun
src = open(os.path.join(ROOT, "plugins/db/scripts/test_dbrun.py")).read()
strs = set()
for n in ast.walk(ast.parse(src)):
    if isinstance(n, ast.Constant) and isinstance(n.value, str):
        s = n.value
        if len(s) < 300 and (" " in s or s == "") and not s.startswith(("/", "#")) and "\n" not in s[:0]:
            strs.add(s)
extra = ["SELECT 1; DROP TABLE t", "select * from t where a = 'it''s'", "SELECT `delete` FROM t", "SELECT 1 /* unterminated",
 "SELECT 'unterminated", "select\tcount( * ) from a join b on 1=1", "WITH x AS (SELECT 1) UPDATE t SET a=1",
 "SELECT a FROM t WHERE x = '\\' OR 1=1 --'", "show tables", "describe t", "SELECT lo_import('/x')", "SELECT 1 INTO OUTFILE '/x'",
 "SELECT xp_cmdshell('dir')", "SELECT * FROM sys.objects", "SELECT TOP 5 * FROM users", "SELECT DISTINCT * FROM u", "SELECT COUNT(*) FROM v$session",
 "SELECT count(1) FROM a WHERE 1=1", "EXPLAIN (FORMAT JSON) SELECT 1", "SELECT é FROM t", "SELECT 1\n-- c\n; DELETE FROM t"]
strs.update(extra)
corp = sorted(strs)
out = {"classify": [], "split": [], "ddl": [], "cost": []}
for e in dbrun.ENGINES:
    for s in corp:
        r = dbrun.classify_statement(s, e)
        out["classify"].append([e, s, r[0], r[1]])
        try:
            sp = dbrun.split_statements(s, e); sp = ["ok", sp]
        except dbrun.Refused:
            sp = ["refused", []]
        out["split"].append([e, s, sp[0], sp[1]])
        out["ddl"].append([e, s, dbrun.is_ddl(s, e)])
        for cls in ("production", "local"):
            for allow in (False, True):
                out["cost"].append([e, s, cls, allow, dbrun.cost_rules(s, cls, allow, e)])
# mask
cols = ["id","nome","email","cpf","telefone","password_hash","birth_date","status","note","ip_address","card_number","street","senha","total"]
rows = [[7,"Maria da Silva","maria.silva@example.com","123.456.789-09","(11) 98765-4321","$2b$12$abc","1990-05-17","active",
 "call joao@corp.com.br, cpf 123.456.789-09, card 4111 1111 1111 1111 and 1234567890123","10.20.30.40","4111111111111111","Rua A 1","hunter2",99.5],
 [None,"","x","","","","","","order 1234567890123 shipped","::1","","","",None]]
out["mask"] = []
for rev in (False, True):
    m, k = dbrun.mask_result(cols, rows, reveal=rev)
    out["mask"].append({"reveal": rev, "cols": cols, "rows": rows, "masked": m, "kinds": k})
dbrun.REDACT.clear()
dbrun.remember_secrets({"host": "db.secret-host.internal", "user": "app_ro", "password": "s3cr3t!"})
msgs = ['connection to server at "db.secret-host.internal" (10.0.0.5) failed: FATAL: password authentication failed for user "app_ro" (tried s3cr3t!)',
        "user app_ro in application app_rox x-app_ro app_ro.y"]
out["redact"] = [[m, dbrun.redact(m)] for m in msgs]
out["network"] = [[{"container_id":"abc","pod":""}, dbrun.network_args({"container_id":"abc","pod":""})],
 [{"container_id":"abc","pod":"p1","pid":4242}, dbrun.network_args({"container_id":"abc","pod":"p1","pid":4242})]]
# Run the original 16 driver-free tests, retaining each test's provenance and
# every call's Python result (including refusal reasons). No copied expectations.
import test_dbrun
functions = ("classify_statement", "split_statements", "is_ddl", "cost_rules",
             "mask_result", "redact", "network_args", "require_user_database", "explain_sql")
out["unit"] = []
for cls in (test_dbrun.ClassifierTest, test_dbrun.CostTest, test_dbrun.MaskTest,
            test_dbrun.RedactTest, test_dbrun.NetworkTest):
    for name in unittest.defaultTestLoader.getTestCaseNames(cls):
        case = {"derived_from": f"{cls.__name__}::{name}", "calls": []}
        def wrap(fn, op):
            def record(*args, **kwargs):
                bound = inspect.signature(fn).bind(*args, **kwargs)
                bound.apply_defaults()
                call = {"op": op, "args": bound.arguments}
                if op == "redact":
                    call["secrets"] = [[re.sub(r"\\(.)", r"\1", pattern.pattern[len(r"(?<![\w.-])"):-len(r"(?![\w-])")]), label]
                                       for pattern, label in dbrun.REDACT]
                try:
                    value = fn(*args, **kwargs)
                except dbrun.Refused as exc:
                    call["refused"] = str(exc)
                    case["calls"].append(call)
                    raise
                call["expected"] = value
                case["calls"].append(call)
                return value
            return record
        with ExitStack() as stack:
            for op in functions:
                stack.enter_context(patch.object(dbrun, op, wrap(getattr(dbrun, op), op)))
            result = unittest.TestResult()
            cls(name).run(result)
            if not result.wasSuccessful():
                raise RuntimeError(result.errors + result.failures)
        out["unit"].append(case)
fixture = Path(ROOT) / "rust/fixtures/db-core/golden.json"
fixture.write_text(json.dumps(out, ensure_ascii=False, indent=2) + "\n")
print(len(corp), {k: len(v) for k, v in out.items()})
