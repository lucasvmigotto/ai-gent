#!/usr/bin/env python3
"""Guard fixtures (spec 033): count, lint and run the shared corpus.

The corpus lives in shared/guard-fixtures/ (contract: specs/033-guard-evaluator/
contracts/fixture-format.md). It was extracted from the shell guard tests and the
opencode parity vectors; every `want` and `reason` is what the real guard.sh printed.

    guard-fixtures.py count
    guard-fixtures.py lint
    guard-fixtures.py run [--guard git|db|secrets|repo|containers|all] [--impl sh|launcher|mjs|bin]
                          [--shell sh|powershell|cmd|all] [--bin PATH] [--parity]
    guard-fixtures.py failures [--guard git|db|secrets|repo|containers|all]
    guard-fixtures.py mutate [--guard ...] [--impl bin|mjs] [--oracle sh] [--bin PATH] [--limit N]

`--impl sh` is the oracle (plugins/*/hooks/guard.sh), `mjs` is opencode/guard/rules.mjs
`bin` is `ai-gent guard` through a subprocess, and `launcher` is the guard.sh of a package
install (the launcher head of shared/guard-launcher.sh, a VERSION file and the real binary
at bin/ai-gent: the path hooks use). `--parity` leaves out the cases tagged `new`, which only
the binary decides, so the three counts are equal. `failures` runs the launcher against a
renamed, failing and slow binary (T051). Output lines use the format of scripts/test-*guard.sh (`      ok    <want> <command>` or `FAIL`).
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
# the shell guards must not log these runs through a binary on PATH (shadow mode, spec 033)
os.environ.setdefault("AI_GENT_NO_SHADOW", "1")
# the secrets guard's personal allowlist must never come from the machine running the corpus
os.environ["AI_GENT_PASS_ALLOW"] = os.path.join(tempfile.gettempdir(), "ai-gent-fixtures-no-pass-allow")
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
BRANCHES = {"main": "main", "staging": "staging", "dev": "dev", "feat": "feat/x",
            "granted": "feat/x", "expired": "feat/x", "partial": "feat/x"}
# The autonomy grant (.ai-gent/autonomy) a sandbox repository carries, as the user would write it.
GRANTS = {
    "granted": "expires: 2099-12-31T23:59:59Z\nbranches: develop dev\nactions: push pr merge issues\n",
    "expired": "expires: 2000-01-01T00:00:00Z\nbranches: develop dev\nactions: push pr merge issues\n",
    "partial": "expires: 2099-12-31T23:59:59Z\nactions: issues\n",
}


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
            if k in GRANTS:
                os.makedirs(os.path.join(p, ".ai-gent"))
                with open(os.path.join(p, ".ai-gent", "autonomy"), "w", encoding="utf-8") as f:
                    f.write(GRANTS[k])

    def sub(self, s):
        return re.sub(r"\{(main|staging|dev|feat|granted|expired|partial)\}", lambda m: self.paths[m.group(1)], s)

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


class Package:
    """A package install in a temp dir: VERSION, bin/ai-gent (the real binary, or a fake) and each plugin's guard.sh."""

    def __init__(self, binpath=None):
        self.dir = tempfile.mkdtemp(prefix="ai-gent-pkg-")
        with open(os.path.join(self.dir, "VERSION"), "w", encoding="utf-8") as f:
            f.write("0.0.0\n")
        os.makedirs(os.path.join(self.dir, "bin"))
        for rel in GUARDS.values():
            hooks = os.path.join(self.dir, os.path.dirname(rel))
            os.makedirs(hooks, exist_ok=True)
            shutil.copy(os.path.join(REPO, rel), hooks)
        if binpath:
            self.set_bin(link=os.path.abspath(binpath))
        self.env = {k: v for k, v in os.environ.items() if k != "CLAUDE_PLUGIN_ROOT"}

    def set_bin(self, script=None, link=None):
        dest = os.path.join(self.dir, "bin", "ai-gent")
        if os.path.lexists(dest):
            os.remove(dest)
        if link:
            os.symlink(link, dest)
        elif script is not None:
            with open(dest, "w", encoding="utf-8") as f:
                f.write(script)
            os.chmod(dest, 0o755)

    def guard(self, g):
        return os.path.join(self.dir, GUARDS[g])

    def close(self):
        shutil.rmtree(self.dir, ignore_errors=True)


INSTALL = "curl -fsSL https://github.com/lucasvmigotto/ai-gent/releases/latest/download/install.sh | sh"
# fake binaries for the failure policy (FR-014): none (renamed), exits 1 with no output, sleeps over the 2 s budget
FAKES = (
    ("binary renamed (missing)", None, "it is missing"),
    ("binary exits 1 with no output", "#!/bin/sh\nexit 1\n", "it failed with exit 1"),
    ("binary sleeps over the budget", "#!/bin/sh\nexec sleep 5\n", "it took over 2 s"),
)


def cmd_failures(a):
    """The launcher's failure policy (T051, SC-006): every failure of the binary becomes the `ask` line with the install command."""
    guards = list(GUARDS) if a.guard == "all" else [a.guard]
    markers = {g: load(g)["marker"] for g in GUARDS}
    sb = Sandbox()
    pkg = Package()
    nobin = tempfile.mkdtemp(prefix="ai-gent-nobin-")  # a PATH with only the tools the launcher needs, and no ai-gent
    for t in ("sh", "timeout", "sleep"):
        p = shutil.which(t)
        if p:
            os.symlink(p, os.path.join(nobin, t))
    pkg.env["PATH"] = nobin
    has_timeout = shutil.which("timeout") is not None
    n = failures = 0
    try:
        for g in guards:
            case = next(c for c in load(g)["cases"] if "command" in c and c["want"] != "none")
            ev = json.dumps(event(case, case["command"], sb))
            for name, script, why in FAKES:
                if why.endswith("2 s") and not has_timeout:
                    print("      skip  %s: %s (no `timeout` on PATH; the hook's own timeout is the backstop)" % (g, name))
                    continue
                pkg.set_bin(script)
                r = subprocess.run(["sh", pkg.guard(g)], input=ev, capture_output=True, text=True, env=pkg.env)
                n += 1
                try:
                    got, reason = parse_hook(r.stdout)
                except (ValueError, KeyError):
                    got, reason = "unparsable", r.stdout
                ok = (r.returncode == 0 and got == "ask" and reason.startswith(markers[g] + " the ai-gent binary is unavailable (" + why + ")")
                      and "run: " + INSTALL in reason)
                if ok:
                    print("      ok    %-10s %s: ask with the install command" % (g, name))
                else:
                    failures += 1
                    print("      FAIL  %-10s %s: exit %s, %s: %s" % (g, name, r.returncode, got, reason))
    finally:
        sb.close()
        pkg.close()
        shutil.rmtree(nobin, ignore_errors=True)
    print("guard-fixtures: %d launcher failure cases, %d failed" % (n, failures))
    return 1 if failures else 0


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
            if a.impl == "launcher" and shell != "sh":
                continue
            if a.parity and "new" in c.get("tags", []):
                continue
            if a.impl == "mjs" and ("mjs" in c.get("skip_in", []) or "new" in c.get("tags", []) or shell != "sh"):
                continue
            items.append((g, c, event(c, command, sb), shell))
    results = []
    pkg = None
    try:
        if a.impl == "mjs":
            results = run_mjs(items)
        else:
            binpath = find_bin(a.bin) if a.impl in ("bin", "launcher") else None
            if a.impl == "launcher":
                pkg = Package(binpath)
            for g, c, ev, shell in items:
                if a.impl == "sh":
                    argv = ["sh", os.path.join(REPO, GUARDS[g])]
                elif a.impl == "launcher":
                    argv = ["sh", pkg.guard(g)]
                else:
                    argv = [binpath, "guard", "--guard", g, "--shell", shell]
                r = subprocess.run(argv, input=json.dumps(ev), capture_output=True, text=True, env=pkg.env if pkg else None)
                results.append(parse_hook(r.stdout))
    finally:
        sb.close()
        if pkg:
            pkg.close()
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


MUTATIONS = (
    ("leading spaces", lambda c: "  " + c),
    ("leading tab", lambda c: "\t" + c),
    ("sudo", lambda c: "sudo " + c),
    ("env prefix", lambda c: "FOO=1 " + c),
    ("cd and", lambda c: "cd /tmp && " + c),
    ("trailing true", lambda c: c + " ; true"),
    ("trailing line", lambda c: c + "\necho done"),
    ("leading line", lambda c: "echo start\n" + c),
    ("subshell", lambda c: "( " + c + " )"),
    ("doubled spaces", lambda c: c.replace(" ", "  ")),
    ("pipe tail", lambda c: c + " | cat"),
    ("background", lambda c: c + " &"),
)


def decide(impl, g, ev, binpath):
    if impl == "sh":
        argv = ["sh", os.path.join(REPO, GUARDS[g])]
    else:
        argv = [binpath, "guard", "--guard", g]
    r = subprocess.run(argv, input=json.dumps(ev), capture_output=True, text=True)
    return parse_hook(r.stdout)


def cmd_mutate(a):
    """Variations nobody wrote by hand: the oracle and the implementation must agree on each."""
    guards = list(GUARDS) if a.guard == "all" else [a.guard]
    binpath = find_bin(a.bin) if a.impl == "bin" else None
    sb = Sandbox()
    n = diffs = 0
    try:
        for g, c in all_cases(guards):
            if "parity" not in c.get("tags", []) or "command" not in c or c.get("shell", "sh") != "sh":
                continue
            for name, fn in MUTATIONS:
                if a.limit and n >= a.limit:
                    break
                mutated = fn(c["command"])
                ev = event(c, mutated, sb)
                want = decide(a.oracle, g, ev, None)
                if a.impl == "mjs":
                    got = run_mjs([(g, c, ev, "sh")])[0]
                else:
                    got = decide("bin", g, ev, binpath)
                n += 1
                if got != want:
                    diffs += 1
                    print("      DIFF  %s [%s] oracle %s, %s %s: %s" % (c["id"], name, want, a.impl, got, mutated.replace("\n", "\u23ce")))
    finally:
        sb.close()
    print("guard-fixtures: %d mutations, %d differences (oracle %s, impl %s)" % (n, diffs, a.oracle, a.impl))
    return 1 if diffs else 0


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    sp = ap.add_subparsers(dest="cmd", required=True)
    sp.add_parser("count").set_defaults(fn=cmd_count)
    sp.add_parser("lint").set_defaults(fn=cmd_lint)
    r = sp.add_parser("run")
    r.add_argument("--guard", default="all", choices=list(GUARDS) + ["all"])
    r.add_argument("--impl", default="sh", choices=["sh", "launcher", "mjs", "bin"])
    r.add_argument("--parity", action="store_true")
    r.add_argument("--shell", default="sh", choices=list(SHELLS) + ["all"])
    r.add_argument("--bin")
    r.set_defaults(fn=cmd_run)
    f = sp.add_parser("failures")
    f.add_argument("--guard", default="all", choices=list(GUARDS) + ["all"])
    f.set_defaults(fn=cmd_failures)
    m = sp.add_parser("mutate")
    m.add_argument("--guard", default="all", choices=list(GUARDS) + ["all"])
    m.add_argument("--impl", default="bin", choices=["mjs", "bin"])
    m.add_argument("--oracle", default="sh", choices=["sh"])
    m.add_argument("--bin")
    m.add_argument("--limit", type=int, default=0)
    m.set_defaults(fn=cmd_mutate)
    a = ap.parse_args()
    sys.exit(a.fn(a))


if __name__ == "__main__":
    main()
