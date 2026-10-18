#!/usr/bin/env python3
"""wt — worktrees for agents:parallel.

Every track gets its own branch in its own git worktree under .worktrees/
at the repository root, kept out of git through .git/info/exclude (no
project file changes), with its own compose project name and port offset
so dev stacks don't collide.

  wt.py plan [--json]                 features ready to start, from specs/README.md
  wt.py add <branch> [--base BRANCH]  a worktree for a new branch off the chain's dev
  wt.py list [--json]                 the worktrees: branch, ahead/behind dev, dirty
  wt.py remove <branch>               remove a merged, clean worktree and its branch

Python 3.9+ stdlib only. See plugins/agents/skills/parallel/SKILL.md.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

DIR = ".worktrees"
STATES = {"planned": 0, "in progress": 1, "partial": 1, "implemented": 2, "verified": 3}
PORT_STEP = 100


class Failed(Exception):
    pass


def git(*args: str, cwd: Path | None = None, ok: bool = False) -> str:
    out = subprocess.run(["git", *args], cwd=cwd, capture_output=True, text=True)
    if out.returncode != 0 and not ok:
        raise Failed(f"git {' '.join(args[:3])}: {out.stderr.strip()[-300:]}")
    return out.stdout.strip() if out.returncode == 0 else ""


def root() -> Path:
    """The main checkout, also when run from inside a worktree."""
    common = Path(git("rev-parse", "--path-format=absolute", "--git-common-dir"))
    return common.parent


def here() -> Path:
    """The checkout you're in — a worktree reads its own specs."""
    return Path(git("rev-parse", "--show-toplevel"))


def ports_file(r: Path) -> Path:
    return r / DIR / ".ports.json"


def load_ports(r: Path) -> dict[str, int]:
    try:
        return json.loads(ports_file(r).read_text())
    except (OSError, ValueError):
        return {}


def save_ports(r: Path, ports: dict[str, int]) -> None:
    ports_file(r).parent.mkdir(parents=True, exist_ok=True)
    ports_file(r).write_text(json.dumps(ports, indent=2, sort_keys=True) + "\n")


def slug(branch: str) -> str:
    return re.sub(r"[^a-z0-9]+", "-", branch.lower()).strip("-")


def chain_dev(r: Path) -> str:
    names = set(git("for-each-ref", "--format=%(refname:short)", "refs/heads", "refs/remotes", cwd=r).split())
    found = [b for b in ("dev", "develop") if b in names or f"origin/{b}" in names]
    if len(found) != 1:
        raise Failed("the chain's dev branch is " + ("ambiguous (dev and develop both exist)" if found else "missing")
                     + "; pass --base, or ask the user which one the flow uses")
    return found[0]


def exclude(r: Path) -> None:
    """Keep .worktrees/ and the per-worktree compose override out of git, locally."""
    path = Path(git("rev-parse", "--path-format=absolute", "--git-common-dir", cwd=r)) / "info" / "exclude"
    path.parent.mkdir(parents=True, exist_ok=True)
    lines = path.read_text().splitlines() if path.exists() else []
    added = [x for x in (f"/{DIR}/", "compose.worktree.yaml") if x not in lines]
    if added:
        path.write_text("\n".join(lines + ["# agents:parallel", *added]) + "\n")


def worktrees(r: Path) -> list[dict]:
    out, cur = [], {}
    for line in git("worktree", "list", "--porcelain", cwd=r).splitlines() + [""]:
        if not line:
            if cur.get("path", "").startswith(str(r / DIR) + "/"):
                out.append(cur)
            cur = {}
            continue
        key, _, value = line.partition(" ")
        cur[key if key != "worktree" else "path"] = value.removeprefix("refs/heads/") if key == "branch" else value
    return out


# ---------------------------------------------------------------- commands


def readme_rows(r: Path) -> list[dict]:
    path = r / "specs" / "README.md"
    if not path.exists():
        raise Failed("no specs/README.md: name the tracks yourself (wt.py add per branch)")
    rows, header = [], None
    for line in path.read_text().splitlines():
        if not line.startswith("|"):
            header = None
            continue
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        if header is None:
            header = [c.lower() for c in cells]
            continue
        if set("".join(cells)) <= set("-: ") or not re.fullmatch(r"\d{3}", cells[0]):
            continue
        row = dict(zip(header, cells))
        deps = row.get("depends on") or row.get("depends") or row.get("dependencies") or ""
        rows.append({"number": cells[0], "name": row.get("feature", ""),
                     "depends": re.findall(r"\d{3}", deps),
                     "sides": {k: v for k, v in row.items() if k in ("frontend", "backend", "status", "qa")}})
    return rows


def level(value: str) -> int | None:
    v = value.lower().strip("*_ ")
    for word, n in STATES.items():
        if v.startswith(word):
            return n
    return None  # N/A, —, retired: not a side to build


def cmd_plan(args) -> int:
    rows = readme_rows(here())
    by = {x["number"]: x for x in rows}

    def built(n: str) -> bool:  # dependencies count once Implemented
        levels = [lv for lv in map(level, by[n]["sides"].values()) if lv is not None] if n in by else []
        return bool(levels) and min(levels) >= 2

    ready, blocked, done = [], [], []
    for x in rows:
        todo = {k: v for k, v in x["sides"].items() if level(v) is not None and level(v) < 2}
        if not todo:
            done.append(x["number"])
            continue
        waiting = [d for d in x["depends"] if not built(d)]
        (blocked if waiting else ready).append({**x, "todo": todo, "waiting": waiting})
    if args.json:
        print(json.dumps({"ready": ready, "blocked": blocked, "done": done}, indent=2))
        return 0
    print("ready to start in parallel (dependencies Implemented):")
    for x in ready:
        print(f"  {x['number']} {x['name']:<28} " + ", ".join(f"{k} {v}" for k, v in x["todo"].items()))
    if not ready:
        print("  (none)")
    for x in blocked:
        print(f"waiting  {x['number']} {x['name']:<28} on {', '.join(x['waiting'])}")
    print(f"\n{len(done)} feature(s) already Implemented or Verified. Split a ready feature into frontend, backend"
          f" and QA tracks only once its contract is settled; share nothing else without an owner.")
    return 0


def cmd_add(args) -> int:
    r = root()
    base = args.base or chain_dev(r)
    path = r / DIR / slug(args.branch)
    if path.exists():
        raise Failed(f"{path.relative_to(r)} exists")
    if git("rev-parse", "--verify", "--quiet", f"refs/heads/{args.branch}", cwd=r, ok=True):
        raise Failed(f"branch {args.branch} exists; a track starts on a new branch")
    exclude(r)
    live = {w.get("branch") for w in worktrees(r)}
    ports = {b: o for b, o in load_ports(r).items() if b in live}  # offsets of live tracks only
    offset = next(k * PORT_STEP for k in range(1, 1000) if k * PORT_STEP not in ports.values())
    git("worktree", "add", "-q", "-b", args.branch, str(path), base, cwd=r)
    ports[args.branch] = offset
    save_ports(r, ports)
    project = f"{slug(r.name)}-{slug(args.branch)}"
    print(json.dumps({"branch": args.branch, "base": base, "path": str(path),
                      "compose_project": project, "port_offset": offset}, indent=2))
    print(f"\nin this worktree: COMPOSE_PROJECT_NAME={project}; publish ports +{offset} through an "
          f"untracked compose.worktree.yaml (already excluded), never by editing the shared compose file",
          file=sys.stderr)
    return 0


def cmd_list(args) -> int:
    r = root()
    try:
        base = chain_dev(r)
    except Failed:
        base = ""
    rows = []
    for w in worktrees(r):
        p = Path(w["path"])
        dirty = bool(git("status", "--porcelain", cwd=p, ok=True))
        ahead = behind = None
        if base and w.get("branch"):
            counts = git("rev-list", "--left-right", "--count", f"{base}...{w['branch']}", cwd=r, ok=True).split()
            if len(counts) == 2:
                behind, ahead = int(counts[0]), int(counts[1])
        rows.append({"branch": w.get("branch", "(detached)"), "path": str(p.relative_to(r)), "dirty": dirty,
                     "ahead": ahead, "behind": behind})
    if args.json:
        print(json.dumps(rows, indent=2))
        return 0
    for x in rows:
        print(f"{x['branch']:<36} {x['path']:<44} +{x['ahead']}/-{x['behind']} vs {base or '?'}"
              + ("  uncommitted changes" if x["dirty"] else ""))
    if not rows:
        print(f"no worktrees under {DIR}/")
    return 0


def cmd_remove(args) -> int:
    r = root()
    match = [w for w in worktrees(r) if w.get("branch") == args.branch]
    if not match:
        raise Failed(f"no worktree under {DIR}/ for {args.branch}")
    p = Path(match[0]["path"])
    if git("status", "--porcelain", cwd=p, ok=True):
        raise Failed(f"{p.relative_to(r)} has uncommitted changes; commit or discard them first")
    base = chain_dev(r)
    # Landed means every change is on dev — by hash, or by content after the
    # platform rebased it (git cherry marks those "-").
    unlanded = [ln for ln in git("cherry", base, args.branch, cwd=r).splitlines() if ln.startswith("+")]
    if unlanded:
        raise Failed(f"{args.branch} has {len(unlanded)} change(s) {base} doesn't; land it first")
    git("worktree", "remove", str(p), cwd=r)
    if not git("branch", "-d", args.branch, cwd=r, ok=True) and git(
            "rev-parse", "--verify", "--quiet", f"refs/heads/{args.branch}", cwd=r, ok=True):
        git("branch", "-D", args.branch, cwd=r)  # its changes are on dev under new hashes (checked above)
    git("worktree", "prune", cwd=r)
    ports = load_ports(r)
    if ports.pop(args.branch, None) is not None:
        save_ports(r, ports)
    print(f"removed {p.relative_to(r)} and branch {args.branch}")
    return 0


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(prog="wt", description=__doc__.split("\n\n")[0])
    sub = ap.add_subparsers(dest="command", required=True)
    s = sub.add_parser("plan", help="features ready to start, from specs/README.md")
    s.add_argument("--json", action="store_true")
    s.set_defaults(func=cmd_plan)
    s = sub.add_parser("add", help="a worktree for a new branch off the chain's dev")
    s.add_argument("branch")
    s.add_argument("--base")
    s.set_defaults(func=cmd_add)
    s = sub.add_parser("list", help="the worktrees under .worktrees/")
    s.add_argument("--json", action="store_true")
    s.set_defaults(func=cmd_list)
    s = sub.add_parser("remove", help="remove a merged, clean worktree and its branch")
    s.add_argument("branch")
    s.set_defaults(func=cmd_remove)
    args = ap.parse_args(argv)
    try:
        return args.func(args)
    except Failed as e:
        print(f"error: {e}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
