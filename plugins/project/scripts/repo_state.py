#!/usr/bin/env python3
"""A compact, read-only snapshot of a repository's git state.

Used by project:recap and project:survey. It never fetches, pulls or
changes anything: run `git fetch` first when remote refs should be fresh
(the output says how old they are).

  repo_state.py [--repo DIR] [--since DATE] [--days N] [--main NAME ...] [--json]

- working tree: branch, upstream ahead/behind, staged/unstaged/untracked,
  stashes, an operation in progress (merge, rebase, cherry-pick, bisect);
- main branches (main, master, develop, dev, homolog, staging, plus --main):
  last commit, local vs. remote, and each against the base (main or master);
- recent branches (active in the last --days, default 30) not merged into the base;
- with --since: commits on every ref since then, yours apart from others';
- tags, remotes (credentials stripped), and when the remote refs were last fetched.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import re
import subprocess
import sys
import time
from pathlib import Path

MAIN_BRANCHES = ["main", "master", "develop", "dev", "homolog", "staging"]


class Git:
    def __init__(self, root: str):
        self.root = root

    def __call__(self, *args: str, ok: bool = False) -> str:
        out = subprocess.run(["git", "-C", self.root, *args], capture_output=True, text=True)
        if out.returncode != 0 and not ok:
            raise RuntimeError(out.stderr.strip() or f"git {' '.join(args)} failed")
        return out.stdout.rstrip() if out.returncode == 0 else ""

    def exists(self, ref: str) -> bool:
        return bool(self("rev-parse", "--verify", "--quiet", ref + "^{commit}", ok=True))

    def counts(self, a: str, b: str) -> tuple[int, int]:
        """(commits in a not in b, commits in b not in a)."""
        out = self("rev-list", "--left-right", "--count", f"{a}...{b}", ok=True).split()
        return (int(out[0]), int(out[1])) if len(out) == 2 else (0, 0)


def working_tree(g: Git, gitdir: Path) -> dict:
    branch = g("branch", "--show-current", ok=True) or None
    status = g("status", "--porcelain=v1", "--untracked-files=normal", ok=True).splitlines()
    staged = [l[3:] for l in status if l[0] not in " ?"]
    unstaged = [l[3:] for l in status if l[1] not in " ?" and l[0] != "?"]
    untracked = [l[3:] for l in status if l.startswith("??")]
    upstream = g("rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{upstream}", ok=True) or None
    ahead, behind = g.counts("HEAD", upstream) if upstream else (0, 0)
    ops = {"MERGE_HEAD": "merge", "rebase-merge": "rebase", "rebase-apply": "rebase", "CHERRY_PICK_HEAD": "cherry-pick",
           "REVERT_HEAD": "revert", "BISECT_LOG": "bisect"}
    stashes = [dict(zip(("ref", "date", "subject"), l.split("\x1f"))) for l in
               g("stash", "list", "--format=%gd\x1f%cr\x1f%s", ok=True).splitlines() if l]
    return {"branch": branch, "head": g("rev-parse", "--short", "HEAD", ok=True), "upstream": upstream,
            "ahead": ahead, "behind": behind, "staged": staged, "unstaged": unstaged, "untracked": untracked,
            "stashes": stashes, "in_progress": sorted({v for k, v in ops.items() if (gitdir / k).exists()})}


def last_commit(g: Git, ref: str) -> dict:
    out = g("log", "-1", "--format=%h\x1f%cs\x1f%cr\x1f%an\x1f%s", ref, ok=True).split("\x1f")
    return dict(zip(("sha", "date", "age", "author", "subject"), out)) if len(out) == 5 else {}


def main_branches(g: Git, remote: str | None, names: list[str]) -> tuple[str | None, list[dict]]:
    rows = []
    for name in names:
        local, rem = f"refs/heads/{name}", f"refs/remotes/{remote}/{name}" if remote else None
        has_local, has_remote = g.exists(local), bool(rem and g.exists(rem))
        if not (has_local or has_remote):
            continue
        row = {"name": name, "local": has_local, "remote": has_remote,
               "last": last_commit(g, local if has_local else rem)}
        if has_local and has_remote:
            row["local_ahead"], row["local_behind"] = g.counts(local, rem)
        rows.append(row)
    base = next((r for r in rows if r["name"] in ("main", "master")), rows[0] if rows else None)
    if base:
        base_ref = best_ref(base, remote)
        for r in rows:
            if r is not base:
                r["vs_base_ahead"], r["vs_base_behind"] = g.counts(best_ref(r, remote), base_ref)
    return (base["name"] if base else None), rows


def best_ref(row: dict, remote: str | None) -> str:
    """The remote ref when there is one (what others see), else the local branch."""
    return f"refs/remotes/{remote}/{row['name']}" if row["remote"] else f"refs/heads/{row['name']}"


def recent_branches(g: Git, base_ref: str | None, main_refs: list[str], days: int, skip: set[str]) -> list[dict]:
    """Branches active in the last `days` that no main branch contains yet."""
    cutoff = time.time() - days * 86400
    out = g("for-each-ref", "--sort=-committerdate", "--format=%(refname)\x1f%(refname:short)\x1f%(committerdate:unix)\x1f"
            "%(committerdate:short)\x1f%(authorname)\x1f%(contents:subject)", "refs/heads", "refs/remotes", ok=True)
    rows = []
    for line in out.splitlines():
        ref, short, unix, date, author, subject = (line.split("\x1f") + [""] * 6)[:6]
        name = re.sub(r"^[^/]+/", "", short) if ref.startswith("refs/remotes/") else short
        if int(unix or 0) < cutoff or name in skip or short.endswith("/HEAD"):
            continue
        if any(g.counts(ref, m)[0] == 0 for m in main_refs):
            continue  # already in a main branch
        ahead, behind = g.counts(ref, base_ref) if base_ref else (0, 0)
        rows.append({"ref": short, "date": date, "author": author, "subject": subject, "ahead": ahead, "behind": behind})
    return rows[:25]


def since(g: Git, when: str, me: set[str]) -> dict:
    out = g("log", "--all", f"--since={when}", "--format=%h\x1f%cs\x1f%an\x1f%ae\x1f%D\x1f%s", ok=True)
    mine, others = [], []
    for line in out.splitlines():
        sha, date, name, email, refs, subject = (line.split("\x1f") + [""] * 6)[:6]
        row = {"sha": sha, "date": date, "author": name, "refs": refs, "subject": subject}
        (mine if (email.lower() in me or name.lower() in me) else others).append(row)
    return {"since": when, "yours": mine[:40], "others": others[:40], "total": len(mine) + len(others)}


def remotes(g: Git) -> list[dict]:
    rows = []
    for line in g("remote", "-v", ok=True).splitlines():
        parts = line.split()
        if len(parts) >= 3 and parts[2] == "(fetch)":
            rows.append({"name": parts[0], "url": re.sub(r"//[^@/]+@", "//", parts[1])})
    return rows


def snapshot(repo: str, when: str | None, days: int, extra: list[str]) -> dict:
    g = Git(repo)
    root = g("rev-parse", "--show-toplevel")
    g.root = root
    gitdir = Path(g("rev-parse", "--absolute-git-dir"))
    rems = remotes(g)
    remote = "origin" if any(r["name"] == "origin" for r in rems) else (rems[0]["name"] if rems else None)
    fetch_head = gitdir / "FETCH_HEAD"
    names = MAIN_BRANCHES + [n for n in extra if n not in MAIN_BRANCHES]
    base, mains = main_branches(g, remote, names)
    base_row = next((r for r in mains if r["name"] == base), None)
    me = {x.lower() for x in (g("config", "user.email", ok=True), g("config", "user.name", ok=True)) if x}
    tags = [dict(zip(("tag", "date"), l.split("\x1f"))) for l in
            g("for-each-ref", "--sort=-creatordate", "--count=5", "--format=%(refname:short)\x1f%(creatordate:short)",
              "refs/tags", ok=True).splitlines() if l]
    return {
        "root": root, "remote": remote, "remotes": rems,
        "last_fetch": dt.datetime.fromtimestamp(fetch_head.stat().st_mtime).astimezone().isoformat(timespec="minutes")
        if fetch_head.exists() else None,
        "working_tree": working_tree(g, gitdir), "base": base, "main_branches": mains,
        "recent_branches": recent_branches(g, best_ref(base_row, remote) if base_row else None,
                                           [best_ref(r, remote) for r in mains], days, set(names)),
        "tags": tags, "commits_since": since(g, when, me) if when else None,
        "first_commit": g("log", "--reverse", "--format=%cs", "--max-parents=0", "HEAD", ok=True).splitlines()[:1],
        "commit_count": int(g("rev-list", "--count", "HEAD", ok=True) or 0),
    }


def render(s: dict) -> None:
    w = s["working_tree"]
    print(f"repo {s['root']} · {s['commit_count']} commits since {(s['first_commit'] or ['?'])[0]} · "
          f"remote {s['remote'] or 'none'} · last fetch {s['last_fetch'] or 'never'}")
    up = f"{w['upstream']} (ahead {w['ahead']}, behind {w['behind']})" if w["upstream"] else "no upstream"
    print(f"on {w['branch'] or 'detached ' + w['head']} @ {w['head']} · {up}"
          + (f" · IN PROGRESS: {', '.join(w['in_progress'])}" if w["in_progress"] else ""))
    for label in ("staged", "unstaged", "untracked"):
        if w[label]:
            more = f" (+{len(w[label]) - 15})" if len(w[label]) > 15 else ""
            print(f"{label} {len(w[label])}: {', '.join(w[label][:15])}{more}")
    for st in w["stashes"]:
        print(f"stash {st['ref']} ({st['date']}): {st['subject']}")
    print(f"\nmain branches (base: {s['base'] or 'none'})")
    for r in s["main_branches"]:
        where = "local+remote" if r["local"] and r["remote"] else ("local only" if r["local"] else "remote only")
        sync = f" · local ahead {r['local_ahead']}, behind {r['local_behind']}" if "local_ahead" in r else ""
        vs = f" · vs {s['base']}: ahead {r['vs_base_ahead']}, behind {r['vs_base_behind']}" if "vs_base_ahead" in r else ""
        last = r["last"]
        print(f"  {r['name']:<8} {where}{sync}{vs} · {last.get('date')} {last.get('sha')} {last.get('subject', '')[:70]}")
    if s["recent_branches"]:
        print(f"\nrecent branches in no main branch (ahead/behind {s['base']})")
        for b in s["recent_branches"]:
            print(f"  {b['ref']} · {b['date']} {b['author']} · ahead {b['ahead']}, behind {b['behind']} · {b['subject'][:60]}")
    if s["tags"]:
        print("\ntags " + ", ".join(f"{t['tag']} ({t['date']})" for t in s["tags"]))
    c = s["commits_since"]
    if c:
        print(f"\ncommits since {c['since']}: {c['total']}")
        for label in ("yours", "others"):
            for row in c[label]:
                refs = f" [{row['refs']}]" if row["refs"] else ""
                print(f"  {label[:-1] if label == 'others' else 'you'}: {row['sha']} {row['date']} {row['author']}{refs} {row['subject'][:70]}")


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--repo", default=".")
    ap.add_argument("--since", help="commits on every ref since this date (git's --since syntax)")
    ap.add_argument("--days", type=int, default=30, help="how recent a branch must be to be listed (default 30)")
    ap.add_argument("--main", action="append", default=[], help="another long-lived branch name")
    ap.add_argument("--json", action="store_true")
    args = ap.parse_args(argv)
    try:
        s = snapshot(args.repo, args.since, args.days, args.main)
    except RuntimeError as e:
        print(f"error: {e}", file=sys.stderr)
        return 1
    print(json.dumps(s, indent=2, ensure_ascii=False)) if args.json else render(s)
    return 0


if __name__ == "__main__":
    sys.exit(main())
