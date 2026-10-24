#!/usr/bin/env python3
"""wt — worktrees for agents:parallel.

Every track gets its own branch in its own git worktree under .worktrees/
at the repository root, kept out of git through .git/info/exclude (no
project file changes), with its own compose project name, port offset and
ownership labels so dev stacks don't collide and can be removed by label.

  wt.py plan [--json]                 features ready to start, from specs/README.md
  wt.py add <branch> [--base BRANCH]  a worktree for a new branch off the chain's dev,
        [--min-free-gb N]             after a disk check of the container store
  wt.py list [--json]                 the worktrees: branch, ahead/behind dev, dirty
  wt.py remove <branch>               remove a merged, clean worktree, its branch and
        [--keep-containers]           its labelled containers and volumes
  wt.py baseline                      record the store's external (buildah) containers
  wt.py resources [--json]            report this project's container leftovers and disk
  wt.py finish                        end of a wave: remove the project's dangling images
                                      and the buildah containers new since the baseline

Container commands go through $CONTAINER_ENGINE (else podman, else docker),
always filtered by this project's label or compose project and removed by
ID; nothing here ever prunes. Settings (flat `key: number` lines) come from
.ai-gent/parallel.yaml: max_parallel, max_volumes, volume_estimate_gb,
disk_floor_gb. See plugins/agents/skills/parallel/SKILL.md and
shared/containers.md §9a.

Python 3.9+ stdlib only.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import time
from pathlib import Path

DIR = ".worktrees"
STATES = {"planned": 0, "in progress": 1, "partial": 1, "implemented": 2, "verified": 3}
PORT_STEP = 100
LABEL_PROJECT = "me.lucasvmigotto.ai-gent.project"
LABEL_WORKTREE = "me.lucasvmigotto.ai-gent.worktree"
COMPOSE_LABEL = "com.docker.compose.project"
GB = 10**9
DEFAULTS = {"max_parallel": 3, "volume_estimate_gb": 10}


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


# -------------------------------------------------------------- containers


def engine() -> str | None:
    """$CONTAINER_ENGINE, else podman, else docker (containers.md §1)."""
    e = os.environ.get("CONTAINER_ENGINE")
    if e:
        return e
    return next((c for c in ("podman", "docker") if shutil.which(c)), None)


def ctr(e: str, *args: str, ok: bool = False) -> str:
    out = subprocess.run([e, *args], capture_output=True, text=True)
    if out.returncode != 0 and not ok:
        raise Failed(f"{Path(e).name} {' '.join(args[:3])}: {out.stderr.strip()[-300:]}")
    return out.stdout.strip() if out.returncode == 0 else ""


def ids(e: str, *args: str) -> list[str]:
    return list(dict.fromkeys(ctr(e, *args).split()))


def settings(r: Path) -> dict[str, float]:
    """Flat `key: number` lines of .ai-gent/parallel.yaml over the defaults."""
    found: dict[str, float] = dict(DEFAULTS)
    try:
        text = (r / ".ai-gent" / "parallel.yaml").read_text()
    except OSError:
        return found
    for line in text.splitlines():
        m = re.match(r"\s*([a-z_]+)\s*:\s*([0-9.]+)\s*(#.*)?$", line)
        if m:
            found[m.group(1)] = float(m.group(2))
    return found


def store_usage(e: str) -> tuple[int, int, str]:
    """(total, free, path) of the filesystem holding the engine's storage."""
    path = (ctr(e, "info", "--format", "{{.Store.GraphRoot}}", ok=True)
            or ctr(e, "info", "--format", "{{.DockerRootDir}}", ok=True) or "/")
    try:
        u = shutil.disk_usage(path)
    except OSError:
        path = "/"
        u = shutil.disk_usage(path)
    return u.total, u.free, path


def project(r: Path) -> str:
    return slug(r.name)


def owned(e: str, r: Path, track: str) -> dict[str, list[str]]:
    """A track's containers and volumes: its worktree label, or its compose project."""
    found: dict[str, list[str]] = {"containers": [], "volumes": []}
    for f in (f"label={LABEL_WORKTREE}={project(r)}/{track}", f"label={COMPOSE_LABEL}={project(r)}-{track}"):
        found["containers"] += ids(e, "ps", "-aq", "--filter", f)
        found["volumes"] += ids(e, "volume", "ls", "-q", "--filter", f)
    return {k: list(dict.fromkeys(v)) for k, v in found.items()}


def release(e: str, r: Path, track: str) -> list[str]:
    """Remove a track's labelled containers, then its volumes, by ID; verify by recount."""
    found = owned(e, r, track)
    removed = []
    for c in found["containers"]:
        ctr(e, "rm", "-f", c, ok=True)
    for v in found["volumes"]:
        ctr(e, "volume", "rm", v, ok=True)
    left = owned(e, r, track)
    for kind in ("containers", "volumes"):
        removed += [f"{kind[:-1]} {x}" for x in found[kind] if x not in left[kind]]
    if left["containers"] or left["volumes"]:
        raise Failed(f"still there after removal: {', '.join(left['containers'] + left['volumes'])} "
                     f"(in use? another store? containers.md §9); the worktree is kept, fix and re-run")
    return removed


def external(e: str) -> list[dict]:
    """Containers in the store the engine doesn't manage: buildah working containers (Podman only)."""
    out = ctr(e, "ps", "-a", "--external", "--format", "{{.ID}}\t{{.Names}}\t{{.Status}}", ok=True)
    rows = []
    for line in out.splitlines():
        parts = line.split("\t")
        if len(parts) == 3 and "storage" in parts[2].lower():
            rows.append({"id": parts[0], "name": parts[1]})
    return rows


def baseline_file(r: Path) -> Path:
    return r / DIR / ".containers-baseline.json"


def builds_running(r: Path) -> tuple[list[str], list[str]]:
    """Image builds in progress: (this project's, elsewhere). Without /proc, every build counts as ours."""
    mine, other = [], []
    procs = Path("/proc")
    if not procs.is_dir():
        out = subprocess.run(["ps", "-eo", "args"], capture_output=True, text=True).stdout
        mine = [ln for ln in out.splitlines() if re.search(r"\b(podman|docker|buildah|nerdctl)\b.*\b(build|bud)\b", ln)]
        return mine, other
    for p in procs.iterdir():
        if not p.name.isdigit():
            continue
        try:
            args = (p / "cmdline").read_bytes().split(b"\0")
            cwd = os.readlink(p / "cwd")
        except OSError:
            continue
        words = [a.decode(errors="replace") for a in args if a]
        if not words or Path(words[0]).name not in ("podman", "docker", "buildah", "nerdctl"):
            continue
        if not any(w in ("build", "bud") for w in words[1:4]):
            continue
        (mine if cwd == str(r) or cwd.startswith(str(r) + "/") else other).append(" ".join(words)[:120])
    return mine, other


def dangling(e: str, r: Path) -> list[str]:
    return ids(e, "images", "-q", "--filter", "dangling=true", "--filter", f"label={LABEL_PROJECT}={project(r)}")


def labelled_volumes(e: str, r: Path) -> list[dict]:
    names = ids(e, "volume", "ls", "-q", "--filter", f"label={LABEL_PROJECT}={project(r)}")
    names += [n for n in ids(e, "volume", "ls", "-q", "--filter", f"label={COMPOSE_LABEL}") if n not in names]
    if not names:
        return []
    rows = []
    for v in json.loads(ctr(e, "volume", "inspect", *names) or "[]"):
        labels = v.get("Labels") or {}
        compose = labels.get(COMPOSE_LABEL, "")
        if labels.get(LABEL_PROJECT) != project(r) and not compose.startswith(project(r) + "-"):
            continue  # another project's compose volume
        track = labels.get(LABEL_WORKTREE, "").partition("/")[2] or compose[len(project(r)) + 1:] or "main"
        rows.append({"name": v.get("Name", ""), "track": track})
    return rows


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
    live = {w.get("branch") for w in worktrees(r)}
    e = engine()
    if e:
        preflight(r, e, len(live), getattr(args, "min_free_gb", None))
    else:
        print("no container engine found: skipping the disk check", file=sys.stderr)
    exclude(r)
    ports = {b: o for b, o in load_ports(r).items() if b in live}  # offsets of live tracks only
    offset = next(k * PORT_STEP for k in range(1, 1000) if k * PORT_STEP not in ports.values())
    git("worktree", "add", "-q", "-b", args.branch, str(path), base, cwd=r)
    ports[args.branch] = offset
    save_ports(r, ports)
    compose = f"{project(r)}-{slug(args.branch)}"
    labels = {LABEL_PROJECT: project(r), LABEL_WORKTREE: f"{project(r)}/{slug(args.branch)}"}
    print(json.dumps({"branch": args.branch, "base": base, "path": str(path),
                      "compose_project": compose, "port_offset": offset, "labels": labels}, indent=2))
    print(f"\nin this worktree: COMPOSE_PROJECT_NAME={compose} AI_GENT_PROJECT={project(r)} "
          f"AI_GENT_WORKTREE={slug(args.branch)}; publish ports +{offset} through an untracked "
          f"compose.worktree.yaml (already excluded), never by editing the shared compose file; create "
          f"volumes and containers with both labels (containers.md §9a)", file=sys.stderr)
    return 0


def preflight(r: Path, e: str, live: int, min_free_gb: float | None) -> None:
    """Refuse a new track when the container store can't hold one more volume per live track."""
    s = settings(r)
    cap = int(s.get("max_volumes", s["max_parallel"]))
    if live >= cap:
        raise Failed(f"{live} track(s) already hold a volume each; the cap is {cap} (max_volumes in "
                     f".ai-gent/parallel.yaml). Land or remove one first")
    total, free, path = store_usage(e)
    if min_free_gb is not None:
        floor = min_free_gb * GB
    elif "disk_floor_gb" in s:
        floor = s["disk_floor_gb"] * GB
    else:
        floor = max(20 * GB, total // 10)
    need = (live + 1) * s["volume_estimate_gb"] * GB
    if free - need < floor:
        print(ctr(e, "system", "df", ok=True), file=sys.stderr)
        raise Failed(f"{free / GB:.1f} GB free on {path}; {live + 1} track(s) at {s['volume_estimate_gb']:g} GB "
                     f"each would leave less than the {floor / GB:.0f} GB floor. `wt.py resources` lists this "
                     f"project's leftovers; never clean another project's to make room. Start fewer tracks, "
                     f"or ask (--min-free-gb overrides the floor)")


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
    e = engine()
    removed: list[str] = []
    if e and not getattr(args, "keep_containers", False):
        removed = release(e, r, slug(args.branch))
    git("worktree", "remove", str(p), cwd=r)
    if not git("branch", "-d", args.branch, cwd=r, ok=True) and git(
            "rev-parse", "--verify", "--quiet", f"refs/heads/{args.branch}", cwd=r, ok=True):
        git("branch", "-D", args.branch, cwd=r)  # its changes are on dev under new hashes (checked above)
    git("worktree", "prune", cwd=r)
    ports = load_ports(r)
    if ports.pop(args.branch, None) is not None:
        save_ports(r, ports)
    print(f"removed {p.relative_to(r)} and branch {args.branch}")
    for x in removed:
        print(f"removed {x}")
    if not e:
        print("no container engine found: the track's containers and volumes weren't checked", file=sys.stderr)
    return 0


def need_engine() -> str:
    e = engine()
    if not e:
        raise Failed("no container engine found (set CONTAINER_ENGINE)")
    return e


def cmd_baseline(args) -> int:
    r, e = root(), need_engine()
    rows = external(e)
    baseline_file(r).parent.mkdir(parents=True, exist_ok=True)
    baseline_file(r).write_text(json.dumps({"taken": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
                                            "external": [x["id"] for x in rows]}, indent=2) + "\n")
    print(f"baseline: {len(rows)} external container(s) already in the store; `wt.py finish` leaves them alone")
    return 0


def load_baseline(r: Path) -> list[str] | None:
    try:
        return json.loads(baseline_file(r).read_text())["external"]
    except (OSError, ValueError, KeyError):
        return None


def cmd_resources(args) -> int:
    r, e = root(), need_engine()
    total, free, path = store_usage(e)
    s = settings(r)
    floor = s["disk_floor_gb"] * GB if "disk_floor_gb" in s else max(20 * GB, total // 10)
    live = {slug(w.get("branch", "")) for w in worktrees(r)} | {"main"}
    base = load_baseline(r)
    report = {
        "project": project(r),
        "disk": {"path": path, "free_gb": round(free / GB, 1), "floor_gb": round(floor / GB, 1),
                 "below_floor": free < floor},
        "images": ids(e, "images", "-q", "--filter", f"label={LABEL_PROJECT}={project(r)}"),
        "dangling_images": dangling(e, r),
        "volumes": [{**v, "orphan": v["track"] not in live} for v in labelled_volumes(e, r)],
        "containers": ids(e, "ps", "-aq", "--filter", f"label={LABEL_PROJECT}={project(r)}"),
        "external_new": [x for x in external(e) if base is None or x["id"] not in base],
        "baseline": base is not None,
    }
    if args.json:
        print(json.dumps(report, indent=2))
        return 0
    d = report["disk"]
    print(f"disk: {d['free_gb']} GB free on {d['path']} (floor {d['floor_gb']} GB)"
          + ("  BELOW THE FLOOR" if d["below_floor"] else ""))
    print(f"images labelled {project(r)}: {len(report['images'])}, dangling: {len(report['dangling_images'])}")
    for v in report["volumes"]:
        print(f"volume {v['name']:<48} track {v['track']}" + ("  (worktree gone)" if v["orphan"] else ""))
    print(f"containers labelled {project(r)}: {len(report['containers'])}")
    for x in report["external_new"]:
        print(f"external {x['id']} {x['name']}" + ("" if base is not None else "  (no baseline: not attributable)"))
    print("report only; `wt.py finish` removes the dangling images and new buildah containers, "
          "`wt.py remove` a landed track's volumes; ask before anything else")
    return 0


def cmd_finish(args) -> int:
    """End of a wave: the project's dangling images and the buildah containers new since the baseline."""
    r, e = root(), need_engine()
    removed, kept = [], []
    before = dangling(e, r)
    for i in before:
        ctr(e, "rmi", i, ok=True)  # never -f: an image still in use stays
    left = dangling(e, r)
    removed += [f"image {i}" for i in before if i not in left]
    kept += [f"image {i} (in use)" for i in left]
    base = load_baseline(r)
    new = [x for x in external(e) if base is not None and x["id"] not in base]
    mine, other = builds_running(r)
    if base is None:
        kept.append("buildah containers: no baseline (`wt.py baseline` at the start of the wave)")
    elif new and (mine or other):
        kept += [f"external {x['id']} {x['name']} (a build is running: {(mine or other)[0]})" for x in new]
    else:
        for x in new:
            ctr(e, "rm", "-f", x["id"], ok=True)
        still = {x["id"] for x in external(e)}
        kept += [f"external {x['id']} {x['name']} (removal didn't take)" for x in new if x["id"] in still]
        removed += [f"external {x['id']} {x['name']}" for x in new if x["id"] not in still]
    for x in removed:
        print(f"removed {x}")
    for x in kept:
        print(f"left    {x}")
    if not removed and not kept:
        print("nothing to remove")
    return 1 if any("didn't take" in x for x in kept) else 0


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(prog="wt", description=__doc__.split("\n\n")[0])
    sub = ap.add_subparsers(dest="command", required=True)
    s = sub.add_parser("plan", help="features ready to start, from specs/README.md")
    s.add_argument("--json", action="store_true")
    s.set_defaults(func=cmd_plan)
    s = sub.add_parser("add", help="a worktree for a new branch off the chain's dev")
    s.add_argument("branch")
    s.add_argument("--base")
    s.add_argument("--min-free-gb", type=float, help="the disk floor for this call, in GB (default max(20, 10%%))")
    s.set_defaults(func=cmd_add)
    s = sub.add_parser("list", help="the worktrees under .worktrees/")
    s.add_argument("--json", action="store_true")
    s.set_defaults(func=cmd_list)
    s = sub.add_parser("remove", help="remove a merged, clean worktree, its branch, containers and volumes")
    s.add_argument("branch")
    s.add_argument("--keep-containers", action="store_true", help="leave the track's containers and volumes")
    s.set_defaults(func=cmd_remove)
    s = sub.add_parser("baseline", help="record the store's external (buildah) containers")
    s.set_defaults(func=cmd_baseline)
    s = sub.add_parser("resources", help="this project's container leftovers and the disk, report only")
    s.add_argument("--json", action="store_true")
    s.set_defaults(func=cmd_resources)
    s = sub.add_parser("finish", help="end of a wave: dangling images, new buildah containers")
    s.set_defaults(func=cmd_finish)
    args = ap.parse_args(argv)
    try:
        return args.func(args)
    except Failed as e:
        print(f"error: {e}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
