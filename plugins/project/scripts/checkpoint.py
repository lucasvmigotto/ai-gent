#!/usr/bin/env python3
"""Keep a repository's hand-off file — .ai-gent/checkpoint.md — for project:checkpoint.

The checkpoint is what another agent (another account, another tool, the same
session after a compaction) reads first to pick the work back up: the user's
requests and standing rules in their own words, what is on hold, the state of
each repository involved, and the next steps. It is local working material,
kept out of git through .git/info/exclude. Standard library only.

  checkpoint.py init  [--repo DIR]   create the skeleton (if missing) and exclude it from git
  checkpoint.py show  [--repo DIR]   print it, with a staleness warning
  checkpoint.py stamp [--repo DIR]   refresh the "Updated" line after a rewrite

`show` exits 1 when there is no checkpoint, so project:recap can fall back to
transcripts.
"""

from __future__ import annotations

import argparse
import datetime as dt
import os
import re
import subprocess
import sys
from pathlib import Path

REL = Path(".ai-gent") / "checkpoint.md"
EXCLUDE_LINE = "/.ai-gent/checkpoint.md"
SECTIONS = ["Asked", "Rules in force", "Holds", "State", "Next", "Open questions", "Done"]
SKELETON = """# Checkpoint — {name}

Updated {now} · {who}

Local hand-off file (git-excluded). Rewrite it whole at each milestone; keep the
user's words verbatim and dated. No secret values, ever.

## Asked

## Rules in force

## Holds

## State

## Next

## Open questions

## Done
"""


def git(root: Path, *args: str) -> str:
    out = subprocess.run(["git", "-C", str(root), *args], capture_output=True, text=True)
    return out.stdout.strip() if out.returncode == 0 else ""


def repo_root(start: Path) -> Path:
    top = git(start, "rev-parse", "--show-toplevel")
    return Path(top).resolve() if top else start.resolve()


def now() -> str:
    return dt.datetime.now().astimezone().isoformat(timespec="minutes")


def who() -> str:
    tool = "opencode" if os.environ.get("OPENCODE_SESSION_ID") else "claude"
    session = os.environ.get("CLAUDE_CODE_SESSION_ID") or os.environ.get("OPENCODE_SESSION_ID") or "?"
    account = Path(os.environ["CLAUDE_CONFIG_DIR"]).name if os.environ.get("CLAUDE_CONFIG_DIR") else ""
    return f"{tool} session {session}" + (f" · account {account}" if account else "")


def exclude(root: Path) -> bool:
    """Add the checkpoint to .git/info/exclude unless git already ignores it. True if it changed."""
    if not git(root, "rev-parse", "--git-dir"):
        return False
    if subprocess.run(["git", "-C", str(root), "check-ignore", "-q", "--no-index", str(REL)]).returncode == 0:
        return False
    path = Path(git(root, "rev-parse", "--git-path", "info/exclude"))
    path = path if path.is_absolute() else root / path
    path.parent.mkdir(parents=True, exist_ok=True)
    text = path.read_text() if path.exists() else ""
    path.write_text(text + ("" if text.endswith("\n") or not text else "\n") + EXCLUDE_LINE + "\n")
    return True


def staleness(root: Path, path: Path) -> list[str]:
    """Why the checkpoint may be behind the repository."""
    warn = []
    mtime = path.stat().st_mtime
    last = git(root, "log", "-1", "--format=%ct")
    if last and int(last) > mtime:
        warn.append(f"the last commit ({git(root, 'log', '-1', '--format=%h %s')}) is newer than the checkpoint")
    head = git(root, "rev-parse", "--short", "HEAD")
    if head and head not in path.read_text():
        warn.append(f"HEAD {head} is not mentioned in it")
    missing = [s for s in SECTIONS if not re.search(rf"^## {re.escape(s)}\s*$", path.read_text(), re.M)]
    if missing:
        warn.append("missing sections: " + ", ".join(missing))
    return warn


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("cmd", choices=("init", "show", "stamp"))
    ap.add_argument("--repo", default=".")
    args = ap.parse_args(argv)
    root = repo_root(Path(args.repo))
    path = root / REL
    if args.cmd == "init":
        created = not path.exists()
        if created:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(SKELETON.format(name=root.name, now=now(), who=who()))
        excluded = exclude(root)
        print(f"{'created' if created else 'kept'} {REL}" + (f"; added {EXCLUDE_LINE} to .git/info/exclude" if excluded else ""))
        return 0
    if not path.exists():
        print(f"no checkpoint in {root} ({REL})", file=sys.stderr)
        return 1
    if args.cmd == "stamp":
        text = path.read_text()
        line = f"Updated {now()} · {who()}"
        text, n = re.subn(r"^Updated .*$", line, text, count=1, flags=re.M)
        path.write_text(text if n else text.replace("\n", f"\n\n{line}\n", 1))
        print(line)
        return 0
    print(path.read_text(), end="")
    for w in staleness(root, path):
        print(f"\nwarning: {w}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
