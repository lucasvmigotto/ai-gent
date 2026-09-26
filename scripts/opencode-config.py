#!/usr/bin/env python3
"""opencode-config — edit the OpenCode V2 config for setup.sh.

Native V2 configuration uses an ordered `permissions` array of
{action, resource, effect} rules (the V1 grouped `permission` map and `bash`
action are legacy). This helper reads and rewrites that array, plus the
`plugins` array that loads the ai-gent guard plugin, preserving every
unrelated setting as written.

Python 3.9+ stdlib. Every subcommand prints a document to stdout unless noted,
so the caller writes it atomically after a backup.
"""

from __future__ import annotations

import json
import sys

SCHEMA = "https://opencode.ai/config.json"

# The rules mirroring plugins/git/hooks/guard.sh and plugins/db/hooks/guard.sh.
# The guard plugin (opencode/guard/) enforces the parts a pattern cannot.
GUARD_RULES: list[dict[str, str]] = [
    {"action": "shell", "resource": "git push*", "effect": "ask"},
    {"action": "shell", "resource": "git commit*--no-verify*", "effect": "deny"},
    {"action": "shell", "resource": "git branch -D*", "effect": "ask"},
    {"action": "shell", "resource": "git reset --hard*", "effect": "ask"},
    {"action": "shell", "resource": "gh pr create*", "effect": "ask"},
    {"action": "shell", "resource": "psql*", "effect": "ask"},
    {"action": "shell", "resource": "mysql*", "effect": "ask"},
    {"action": "shell", "resource": "sqlcmd*", "effect": "ask"},
    {"action": "shell", "resource": "sqlplus*", "effect": "ask"},
    {"action": "shell", "resource": "sqlite3*", "effect": "ask"},
    {"action": "shell", "resource": "pg_restore*", "effect": "deny"},
    {"action": "shell", "resource": "*connections.env*", "effect": "deny"},
    {"action": "read", "resource": "~/.config/ai-gent/*", "effect": "deny"},
    {"action": "edit", "resource": "~/.config/ai-gent/*", "effect": "deny"},
]


def skill_denies(skills: list[str]) -> list[dict[str, str]]:
    """Hide the short, colliding skill IDs OpenCode finds in ~/.claude/skills.

    OpenCode V2 scans ~/.claude/skills with no opt-out, so plugin skills show
    up under their bare directory name (`spec`, `build`, ...) next to the
    namespaced `<plugin>-<skill>` stubs. Denying the bare IDs leaves only the
    namespaced ones.
    """
    return [{"action": "skill", "resource": s, "effect": "deny"} for s in sorted(set(skills))]


def desired(skills: list[str]) -> list[dict[str, str]]:
    return GUARD_RULES + skill_denies(skills)


def key(rule: dict) -> tuple:
    return (rule.get("action"), rule.get("resource"))


def read(path: str) -> dict:
    with open(path) as f:
        doc = json.load(f)
    if not isinstance(doc, dict):
        raise ValueError("configuration is not a JSON object")
    for field in ("permissions", "plugins"):
        if field in doc and not isinstance(doc[field], list):
            raise ValueError(f"{field} is not an array")
    return doc


def emit(doc: dict) -> None:
    print(json.dumps(doc, indent=2, ensure_ascii=False))


def valid_rules(doc: dict) -> list[dict]:
    return [r for r in doc.get("permissions", []) if isinstance(r, dict)]


# ------------------------------------------------------------------ commands


def cmd_check(path: str) -> int:
    try:
        read(path)
    except (OSError, ValueError):
        return 1
    return 0


def cmd_empty(path: str) -> int:
    try:
        doc = read(path)
    except (OSError, ValueError):
        return 1
    rest = {k: v for k, v in doc.items() if k not in ("$schema", "permissions", "plugins")}
    return 0 if not rest and not doc.get("permissions") and not doc.get("plugins") else 1


def cmd_rules(skills: list[str]) -> int:
    print(json.dumps(desired(skills)))
    return 0


def cmd_missing(path: str, skills: list[str]) -> int:
    doc = read(path)
    have = {key(r) for r in valid_rules(doc)}
    print(json.dumps([r for r in desired(skills) if key(r) not in have]))
    return 0


def cmd_conflicts(path: str, skills: list[str]) -> int:
    doc = read(path)
    want = {key(r): r.get("effect") for r in desired(skills)}
    for r in valid_rules(doc):
        k = key(r)
        if k in want and want[k] != r.get("effect"):
            print(f"{k[0]} {k[1]}: yours is {r.get('effect')}, kept")
    return 0


def cmd_plugin_missing(path: str, plugin: str) -> int:
    """Exit 1 when the plugin is absent (like grep), 0 when present."""
    doc = read(path)
    return 0 if plugin in doc.get("plugins", []) else 1


def cmd_apply(path: str, skills: list[str], plugin: str | None) -> int:
    doc = read(path)
    have = {key(r) for r in valid_rules(doc)}
    rules = valid_rules(doc)
    for r in desired(skills):
        if key(r) not in have:
            rules.append(r)
            have.add(key(r))
    if rules:
        doc["permissions"] = rules
    if plugin:
        plugins = doc.get("plugins", [])
        if plugin not in plugins:
            plugins.append(plugin)
            doc["plugins"] = plugins
    emit(doc)
    return 0


def cmd_new(skills: list[str], plugin: str | None) -> int:
    doc: dict = {"$schema": SCHEMA, "permissions": desired(skills)}
    if plugin:
        doc["plugins"] = [plugin]
    emit(doc)
    return 0


def cmd_record(state: str, rules_json: str, plugin: str | None) -> int:
    import os

    try:
        with open(state) as f:
            data = json.load(f)
    except (OSError, ValueError):
        data = {}
    rules = data.get("permissions", [])
    have = {key(r) for r in rules if isinstance(r, dict)}
    for r in json.loads(rules_json):
        if key(r) not in have:
            rules.append(r)
            have.add(key(r))
    data["permissions"] = rules
    if plugin:
        data["plugins"] = sorted(set(data.get("plugins", []) + [plugin]))
    os.makedirs(os.path.dirname(state) or ".", exist_ok=True)
    with open(state, "w") as f:
        json.dump(data, f, indent=2)
    return 0


def cmd_remove(path: str, state: str) -> int:
    doc = read(path)
    try:
        with open(state) as f:
            data = json.load(f)
    except (OSError, ValueError):
        emit(doc)
        return 0
    removed = {(r.get("action"), r.get("resource"), r.get("effect")) for r in data.get("permissions", [])}
    kept = [
        r
        for r in valid_rules(doc)
        if (r.get("action"), r.get("resource"), r.get("effect")) not in removed
    ]
    if "permissions" in doc:
        if kept:
            doc["permissions"] = kept
        else:
            del doc["permissions"]
    for p in data.get("plugins", []):
        if "plugins" in doc and p in doc["plugins"]:
            doc["plugins"].remove(p)
    if not doc.get("plugins") and "plugins" in doc:
        del doc["plugins"]
    emit(doc)
    return 0


def usage() -> int:
    print(
        "usage: opencode-config.py <check|empty|missing|rules|conflicts|plugin-missing|"
        "apply|new|record|remove> ...",
        file=sys.stderr,
    )
    return 2


def main(argv: list[str]) -> int:
    if not argv:
        return usage()
    action, args = argv[0], argv[1:]

    skills: list[str] = []
    plugin: str | None = None
    positional: list[str] = []
    i = 0
    while i < len(args):
        if args[i] == "--skills":
            skills = [s for s in args[i + 1].split() if s]
            i += 2
        elif args[i] == "--plugin":
            plugin = args[i + 1]
            i += 2
        else:
            positional.append(args[i])
            i += 1

    if action == "check":
        return cmd_check(positional[0])
    if action == "empty":
        return cmd_empty(positional[0])
    if action == "missing":
        return cmd_missing(positional[0], skills)
    if action == "rules":
        return cmd_rules(skills)
    if action == "conflicts":
        return cmd_conflicts(positional[0], skills)
    if action == "plugin-missing":
        return cmd_plugin_missing(positional[0], positional[1])
    if action == "apply":
        return cmd_apply(positional[0], skills, plugin)
    if action == "new":
        return cmd_new(skills, plugin)
    if action == "record":
        return cmd_record(positional[0], positional[1], plugin)
    if action == "remove":
        return cmd_remove(positional[0], positional[1])
    return usage()


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
