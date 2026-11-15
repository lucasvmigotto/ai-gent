"""Run the eight unmodified SQLite journeys with native execution only.

Python retains policy, plans, backups, restore, masking and logging. Patching
execute (not connect) ensures SQL execution cannot fall back to Python SQLite.
"""
import json
import os
from pathlib import Path
import subprocess
import sys
import unittest
from unittest.mock import patch

sys.dont_write_bytecode = True
root = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(root / "plugins/db/scripts"))
import test_dbrun

binary = os.environ["DBRUN_SQLITE_EXEC"]
executions = []


def native_execute(request):
    assert request["engine"] == "sqlite"
    proc = subprocess.run([binary], input=json.dumps(request), text=True,
                          capture_output=True, check=True, timeout=15)
    result = json.loads(proc.stdout)
    executions.append((request, result))
    if request["mode"] == "read" and request["statements"] == ["UPDATE customer SET status = 'x'"]:
        assert result["ok"] is False and "readonly" in result["error"].lower(), result
    return result


suite = unittest.defaultTestLoader.loadTestsFromTestCase(test_dbrun.SqliteEndToEndTest)
assert suite.countTestCases() == 8
with patch.object(test_dbrun.dbrun, "execute", native_execute):
    result = unittest.TextTestRunner(verbosity=2).run(suite)
assert any(req["mode"] == "write" and res.get("committed") for req, res in executions)
assert any(res.get("stage") == "verify" for _, res in executions)
sys.exit(not result.wasSuccessful())
