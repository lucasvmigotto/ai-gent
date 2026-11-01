#!/usr/bin/env python3
"""Guard fixtures (spec 033): count, lint and run the shared corpus.

The corpus lives in shared/guard-fixtures/ (contract: specs/033-guard-evaluator/
contracts/fixture-format.md). It was extracted from the shell guard tests and the
opencode parity vectors; every `want` and `reason` is what the real guard.sh printed.

    guard-fixtures.py count
    guard-fixtures.py lint
    guard-fixtures.py run [--guard git|db|secrets|repo|containers|all] [--impl sh|mjs|bin]
                          [--shell sh|powershell|cmd|all] [--bin PATH]

`--impl sh` is the oracle (plugins/*/hooks/guard.sh), `mjs` is opencode/guard/rules.mjs
and `bin` is `ai-gent guard` through a subprocess (the path hooks use). Output lines use
the format of scripts/test-*guard.sh (`      ok    <want> <command>` or `FAIL`).
Stdlib only.
"""
import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
FIX = os.path.join(REPO, "shared", "guard-fixtures")
GUARDS = {
    "git": "plugins/git/hooks/guard.sh",
    "db": "plugins/db/hooks/guard.sh",
    "secrets": "plugins/secrets/hooks/guard.sh",
    "repo": "plugins/repo/hooks/guard.sh",
    "containers": "plugins/devcontainer/hooks/guard.sh",
}
SHELLS = ("sh", "powershell", "cmd")
WANTS = ("deny", "ask", "none")
BRANCHES = {"main": "main", "staging": "staging", "dev": "dev", "feat": "feat/x"}


def load(name):
    with open(os.path.join(FIX, name + ".json"), encoding="utf-8") as f:
        return json.load(f)


def all_cases(guards):
    out = []
    for g in guards:
        for c in load(g)["cases"]:
            out.append((g, c))
    return out


def expand(g, c, shell):
    """The case as run in `shell`: (command or None, skip reason or None)."""
    if "command" not in c:
        return None, None
    own = c.get("shell", "sh")
    if own != "sh":  # written for one shell: runs only there
        return (c["command"], None) if shell == own else (None, "written for " + own)
    if shell == "sh":
        return c["command"], None
    if shell in c.get("skip_in", []):
        return None, c.get("skip_reason", "skipped")
    v = c.get("variants", {}).get(shell)
    if v is None:
        return None, "no %s variant" % shell
    return v, None


def cmd_count(_a):
    total = 0
    for g in GUARDS:
        d = load(g)
        n = len(d["cases"])
        v = sum(len(c.get("variants", {})) for c in d["cases"])
        print("%-11s %3d cases %3d variants" % (g, n, v))
        total += n
    adapter = load("adapter")["cases"]
    print("%-11s %3d cases" % ("adapter", len(adapter)))
    m = json.load(open(os.path.join(FIX, "manifest.json"), encoding="utf-8"))
    print("shell-sourced vectors (expect calls): %d; fixture cases: %d" % (m["total_shell_vectors"], total + len(adapter)))
    return 0


def cmd_lint(_a):
    errs = []
    seen = set()
    files = list(GUARDS) + ["adapter"]
    for name in files:
        d = load(name)
        if d.get("schema") != 1:
            errs.append("%s: schema must be 1" % name)
        marker = d.get("marker")
        for c in d["cases"]:
            cid = c.get("id", "?")
            if cid in seen:
                errs.append("duplicate id %s" % cid)
            seen.add(cid)
            if c.get("want") not in WANTS:
                errs.append("%s: bad want" % cid)
            if not c.get("source"):
                errs.append("%s: source missing" % cid)
            if name == "adapter":
                if "event" not in c:
                    errs.append("%s: event missing" % cid)
                continue
            if ("command" in c) == ("tool_input" in c):
                errs.append("%s: exactly one of command and tool_input" % cid)
            if c.get("want") in ("deny", "ask"):
                if not c.get("reason"):
                    errs.append("%s: reason missing" % cid)
                elif not c["reason"].startswith(marker + " "):
                    errs.append("%s: reason does not start with the marker" % cid)
            elif c.get("reason"):
                errs.append("%s: reason on a none case" % cid)
            if c.get("shell", "sh") not in SHELLS:
                errs.append("%s: bad shell" % cid)
            for sh, v in c.get("variants", {}).items():
                if sh not in SHELLS or sh == "sh" or not isinstance(v, str) or not v:
                    errs.append("%s: bad variant %s" % (cid, sh))
            for sh in c.get("skip_in", []):
                if sh not in SHELLS + ("mjs", "rust"):
                    errs.append("%s: bad skip_in %s" % (cid, sh))
            if c.get("skip_in") and not c.get("skip_reason"):
                errs.append("%s: skip_in needs skip_reason" % cid)
            if c.get("expected_divergence") and "new" not in c.get("tags", []):
                errs.append("%s: expected_divergence needs tag new" % cid)
    for e in errs:
        print("lint: " + e, file=sys.stderr)
    if errs:
        return 1
    print("lint ok: %d cases" % len(seen))
    return 0


class Sandbox:
    def __init__(self):
        self.dir = tempfile.mkdtemp(prefix="ai-gent-fx-")
        self.paths = {}
        for k, b in BRANCHES.items():
            p = os.path.join(self.dir, k)
            subprocess.run(["git", "init", "-q", "-b", b, p], check=True)
            self.paths[k] = p

    def sub(self, s):
        return re.sub(r"\{(main|staging|dev|feat)\}", lambda m: self.paths[m.group(1)], s)

    def cwd(self, c):
        return self.paths[c[1:]] if c.startswith("@") else c

    def close(self):
        shutil.rmtree(self.dir, ignore_errors=True)


def event(c, command, sb):
    ti = {"command": sb.sub(command)} if command is not None else c["tool_input"]
    return {"hook_event_name": "PreToolUse", "tool_name": c["tool"], "cwd": sb.cwd(c["cwd"]), "tool_input": ti}


def parse_hook(out):
    out = out.strip()
    if not out:
        return "none", None
    o = json.loads(out)["hookSpecificOutput"]
    return o["permissionDecision"], o["permissionDecisionReason"]


def find_bin(arg):
    cands = [arg, os.environ.get("AI_GENT_BIN")] + [os.path.join(REPO, "rust", "target", p, "ai-gent" + (".exe" if os.name == "nt" else "")) for p in ("release", "debug")]
    for p in cands:
        if p and os.path.isfile(p):
            return p
    sys.exit("guard-fixtures: no ai-gent binary (cargo build in rust/, or --bin / AI_GENT_BIN)")


def run_mjs(items):
    rt = shutil.which("node") or shutil.which("bun")
    if not rt:
        sys.exit("guard-fixtures: neither node nor bun on PATH")
    payload = [{"guard": g, "tool": c["tool"], "cwd": ev["cwd"], "command": ev["tool_input"].get("command"), "tool_input": ev["tool_input"]} for g, c, ev, _ in items]
    r = subprocess.run([rt, os.path.join(REPO, "scripts", "guard-fixtures-mjs.mjs")], input=json.dumps(payload), capture_output=True, text=True)
    if r.returncode:
        sys.exit("guard-fixtures: mjs runner failed: " + r.stderr)
    return [(x["decision"], x["reason"]) for x in json.loads(r.stdout)]


def cmd_run(a):
    guards = list(GUARDS) if a.guard == "all" else [a.guard]
    shells = list(SHELLS) if a.shell == "all" else [a.shell]
    sb = Sandbox()
    items = []
    skipped = 0
    for g, c in all_cases(guards):
        for shell in shells:
            command, why = expand(g, c, shell)
            if "command" in c and command is None:
                if why and not why.startswith("written for"):
                    skipped += 1
                continue
            if "tool_input" in c and shell != "sh":
                continue
            if a.impl == "sh" and (shell != "sh" or "new" in c.get("tags", [])):
                continue
            if a.impl == "mjs" and ("mjs" in c.get("skip_in", []) or "new" in c.get("tags", []) or shell != "sh"):
                continue
            items.append((g, c, event(c, command, sb), shell))
    results = []
    try:
        if a.impl == "mjs":
            results = run_mjs(items)
        else:
            binpath = find_bin(a.bin) if a.impl == "bin" else None
            for g, c, ev, shell in items:
                if a.impl == "sh":
                    argv = ["sh", os.path.join(REPO, GUARDS[g])]
                else:
                    argv = [binpath, "guard", "--guard", g, "--shell", shell]
                r = subprocess.run(argv, input=json.dumps(ev), capture_output=True, text=True)
                results.append(parse_hook(r.stdout))
    finally:
        sb.close()
    failures = 0
    for (g, c, ev, shell), (got, reason) in zip(items, results):
        label = (ev["tool_input"].get("command") or "%s %s" % (c["tool"], json.dumps(c.get("tool_input")))).replace("\n", "⏎")
        want = c["want"]
        if got == want and (want == "none" or reason == c["reason"]):
            print("      ok    %-5s %s" % (want, label))
        else:
            failures += 1
            print("      FAIL  %s %s want %s (%s), got %s (%s): %s" % (c["id"], shell, want, c.get("reason"), got, reason, label))
    print("guard-fixtures: %d run, %d failed, %d skipped (impl %s)" % (len(items), failures, skipped, a.impl))
    return 1 if failures else 0


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    sp = ap.add_subparsers(dest="cmd", required=True)
    sp.add_parser("count").set_defaults(fn=cmd_count)
    sp.add_parser("lint").set_defaults(fn=cmd_lint)
    r = sp.add_parser("run")
    r.add_argument("--guard", default="all", choices=list(GUARDS) + ["all"])
    r.add_argument("--impl", default="sh", choices=["sh", "mjs", "bin"])
    r.add_argument("--shell", default="sh", choices=list(SHELLS) + ["all"])
    r.add_argument("--bin")
    r.set_defaults(fn=cmd_run)
    a = ap.parse_args()
    sys.exit(a.fn(a))


if __name__ == "__main__":
    main()
