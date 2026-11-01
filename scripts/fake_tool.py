#!/usr/bin/env python3
"""The Python twin of rust/crates/ai-gent-core/examples/fake_tool.rs (spec 037 T001): a stand-in for
pass, gopass, gh, az, glab and jira, chosen by the file name it was copied to. Used by
scripts/test-parity-repo.py so the oracle run needs no cargo build. Keep the two in step:

- `pass show E` / `gopass show -f E`: print $PASSWORD_STORE_DIR/E.gpg (or .age for gopass), else exit 1.
- `pass insert --multiline E` / `gopass insert -m E`: store stdin as E.gpg.
- other names: rules from `<dir of the tool>/<name>.rules.json` (or $FAKE_TOOL_RULES): the first rule whose
  `args` is a word prefix of the joined arguments wins; `{args}` and `{env:NAME}` are expanded in stdout and
  stderr; `capture` writes the joined arguments to that file; `exit` is the status.
"""
import json
import os
import re
import sys
import time


def main():
    prog = sys.argv[0]
    stem = os.path.splitext(os.path.basename(prog))[0]
    args = sys.argv[1:]
    if os.environ.get("FAKE_TOOL_SLEEP"):
        time.sleep(float(os.environ["FAKE_TOOL_SLEEP"]))
    if stem in ("pass", "gopass"):
        store = os.environ.get("PASSWORD_STORE_DIR", "")
        entry = args[-1] if args else ""
        if args[:1] == ["show"]:
            for ext in (("gpg",) if stem == "pass" else ("gpg", "age")):
                p = os.path.join(store, f"{entry}.{ext}")
                if os.path.isfile(p):
                    sys.stdout.write(open(p, encoding="utf-8").read())
                    return 0
            sys.stderr.write(f"Error: {entry} is not in the password store.\n")
            return 1
        if args[:1] == ["insert"]:
            if os.environ.get("FAKE_TOOL_FAIL_INSERT"):
                sys.stderr.write("insert failed\n")
                return 1
            path = os.path.join(store, f"{entry}.gpg")
            os.makedirs(os.path.dirname(path), exist_ok=True)
            open(path, "w", encoding="utf-8").write(sys.stdin.read())
            return 0
        return 2
    rules_path = os.environ.get("FAKE_TOOL_RULES") or os.path.join(os.path.dirname(os.path.abspath(prog)), f"{stem}.rules.json")
    spec = {}
    if os.path.isfile(rules_path):
        spec = json.load(open(rules_path, encoding="utf-8"))
    joined = " ".join(args)
    rule = None
    for r in spec.get("rules", []):
        a = r.get("args", "")
        if joined == a or joined.startswith(a + " "):
            rule = r
            break
    if rule is None:
        rule = spec["default"] if isinstance(spec.get("default"), dict) else {"stdout": "args: {args}\ntoken: {env:GH_TOKEN}\n"}

    def expand(t):
        t = t.replace("{args}", joined)
        return re.sub(r"\{env:([^}]*)\}", lambda m: os.environ.get(m.group(1), ""), t)

    if rule.get("capture"):
        open(rule["capture"], "w", encoding="utf-8").write(joined + "\n")
    sys.stdout.write(expand(rule.get("stdout", "")))
    sys.stdout.flush()
    sys.stderr.write(expand(rule.get("stderr", "")))
    return int(rule.get("exit", 0))


if __name__ == "__main__":
    sys.exit(main())
