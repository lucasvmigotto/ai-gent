#!/usr/bin/env python3
"""Synthetic audit probes; no agent dispatch, network, credentials, or actual patches.

Run with Python 3 and ai-gent on PATH. Each run keeps its synthetic files in /tmp.
These print observed behavior; they are not assertions that the defects should persist.
"""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time


binary = shutil.which("ai-gent")
if not binary:
    raise SystemExit("ai-gent is required on PATH")
scratch = Path(tempfile.mkdtemp(prefix="ai-gent-codex-audit-", dir="/tmp"))
env = os.environ.copy()
env["PATH"] = str(scratch / "no-tools")  # No container engine probes.
env["CLAUDE_PLUGIN_OPTION_GUARDLOG"] = "false"


def run(args, payload=None):
    result = subprocess.run(
        [binary, *args], input=payload, text=True, capture_output=True,
        env=env, cwd=scratch, timeout=8,
    )
    raw = result.stdout.strip()
    try:
        output = json.loads(raw) if raw else None
    except ValueError:
        output = raw
    return {"exit": result.returncode, "output": output, "stderr": result.stderr}


def task(ident, path, depends=(), kind="docs"):
    return {"id": ident, "kind": kind, "paths": [path], "depends": list(depends)}


def setup(name, tasks):
    root = scratch / name
    (root / ".ai-gent").mkdir(parents=True)
    source = root / "tasks.json"
    source.write_text(json.dumps(tasks))
    return root, source


results = {}
for name, items, extra in [
    ("rust-routing", [task("T001", "src/lib.rs", kind="rust")], ["--allow-agent", "codex"]),
    ("reverse-order", [task("T002", "docs/b.md", ["T001"]), task("T001", "docs/a.md")], []),
    ("cycle", [task("T001", "docs/a.md", ["T002"]), task("T002", "docs/b.md", ["T001"])], []),
]:
    root, source = setup(name, items)
    results[name] = run(["delegate", "--root", str(root), "--json", "plan", "--tasks-file", str(source), *extra])

root, source = setup("rejected", [task("T001", "docs/a.md"), task("T002", "docs/b.md", ["T001"])])
track = root / ".ai-gent/delegate/first"
track.mkdir(parents=True)
now = int(time.time())
(track / "status.json").write_text(json.dumps({
    "track": "first", "agent": "codex", "role": "builder", "tasks": ["T001"],
    "state": "done", "started": now - 5, "updated": now, "events": 1,
    "worktree": ".worktrees/first", "tokens": None,
    "verdict": {"result": "rejected", "reason": "path-outside-allowed"},
}))
results["rejected-dependency"] = run(["delegate", "--root", str(root), "--json", "idle", "--tasks-file", str(source)])

source = scratch / "negated-source.txt"
source.write_text("crate-x is not archived.\n")
results["negated-source"] = run(["delegate", "--root", str(scratch), "--json", "verify-sources", "--claim", "crate-x is archived", "--source", str(source)])

for target, label in [("connections.toml", "credential"), (".ai-gent/autonomy", "grant")]:
    for tool, arguments in [
        ("Edit", {"file_path": target}),
        ("apply_patch", {"command": f"*** Begin Patch\n*** Add File: {target}\n+synthetic-audit-only\n*** End Patch"}),
    ]:
        event = {"hook_event_name": "PreToolUse", "tool_name": tool, "cwd": str(scratch), "tool_input": arguments}
        results[f"guard-{tool}-{label}"] = run(["guard"], json.dumps(event))

print(json.dumps({"scratch": str(scratch), "results": results}, indent=2))
