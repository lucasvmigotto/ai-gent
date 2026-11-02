import ast, json, sys, os
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
json.dump(out, open("golden.json","w"), ensure_ascii=False)
print(len(corp), {k: len(v) for k, v in out.items()})
