#!/usr/bin/env python3
"""End-to-end test of reporun against GitHub, in a throwaway private repository.

  scripts/test-reporun-github.py [--keep] [--owner NAME]

Creates <owner>/ai-gent-e2e-<timestamp> (private, cloned into a temporary
directory), then drives it only through reporun, as the repo plugin does:
detect; a write refused without --write; issues created, commented, labelled
and closed; a sub-issue; a pull request merged by rebase and one by merge
commit; a batch with references between its steps. The repository is deleted
at the end (--keep leaves it). Needs gh signed in; deleting needs the
delete_repo scope (`gh auth refresh -s delete_repo`), without which the
repository is left and named for you to delete.

Manual, like test-dbrun-engines.py: it touches the real GitHub. Not part of
scripts/check.sh. Run it before pushing a change to plugins/repo/scripts/reporun.py.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tempfile
import time
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
RUNNER = REPO / "plugins" / "repo" / "scripts" / "reporun.py"

failures = 0


def check(what: str, ok: bool, detail: str = "") -> None:
    global failures
    print(f"      {'ok  ' if ok else 'FAIL'}  {what}" + ("" if ok or not detail else f"\n            {detail[:400]}"))
    failures += not ok


def sh(*cmd: str, cwd: Path | None = None, check_rc: bool = True) -> str:
    p = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, stdin=subprocess.DEVNULL)
    if check_rc and p.returncode != 0:
        sys.exit(f"error: {' '.join(cmd)}\n{p.stdout}{p.stderr}")
    return p.stdout.strip()


def reporun(cwd: Path, *args: str) -> subprocess.CompletedProcess:
    return subprocess.run([sys.executable, str(RUNNER), *args], cwd=cwd, capture_output=True, text=True,
                          stdin=subprocess.DEVNULL)


def gh(cwd: Path, *words: str) -> subprocess.CompletedProcess:
    return reporun(cwd, "cli", "github", "--write", "--", *words)


def commit(cwd: Path, name: str) -> None:
    (cwd / f"{name}.txt").write_text(name + "\n")
    sh("git", "add", name + ".txt", cwd=cwd)
    sh("git", "commit", "-q", "-m", f"chore: add {name}", cwd=cwd)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--keep", action="store_true", help="leave the repository")
    ap.add_argument("--owner", help="the account or organisation (default: the signed-in user)")
    args = ap.parse_args()

    owner = args.owner or sh("gh", "api", "user", "-q", ".login")
    name = f"ai-gent-e2e-{int(time.time())}"
    slug = f"{owner}/{name}"
    tmp = Path(tempfile.mkdtemp())
    print(f"reporun end to end · github · {slug}")
    sh("gh", "repo", "create", slug, "--private", "--add-readme", "--clone", cwd=tmp)
    cwd = tmp / name
    sh("git", "config", "user.name", "ai-gent e2e", cwd=cwd)
    sh("git", "config", "user.email", "e2e@example.invalid", cwd=cwd)
    try:
        run_checks(cwd, slug)
    finally:
        if args.keep:
            print(f"kept {slug}")
        else:
            p = subprocess.run(["gh", "repo", "delete", slug, "--yes"], capture_output=True, text=True)
            if p.returncode == 0:
                print(f"deleted {slug}")
            else:
                print(f"NOT deleted: {slug} — run `gh auth refresh -s delete_repo` then `gh repo delete {slug} --yes`")
    print("\nall checks passed" if not failures else f"\n{failures} check(s) failed")
    return 1 if failures else 0


def run_checks(cwd: Path, slug: str) -> None:
    print("  -- detect")
    d = reporun(cwd, "detect", "--json")
    info = json.loads(d.stdout) if d.returncode == 0 else {}
    check("platform detected as github", info.get("platform") == "github", d.stdout + d.stderr)

    print("  -- reads and writes")
    check("a read needs no --write", reporun(cwd, "cli", "github", "--", "issue", "list").returncode == 0)
    w = reporun(cwd, "cli", "github", "--", "issue", "create", "--title", "refused", "--body", "x")
    check("a write without --write is refused (exit 3)", w.returncode == 3, w.stdout + w.stderr)

    print("  -- issues")
    out = gh(cwd, "issue", "create", "--title", "first", "--body", "body")
    num = out.stdout.strip().rsplit("/", 1)[-1]
    check("create", out.returncode == 0 and num.isdigit(), out.stdout + out.stderr)
    check("comment", gh(cwd, "issue", "comment", num, "--body", "a comment").returncode == 0)
    check("label", gh(cwd, "issue", "edit", num, "--add-label", "bug").returncode == 0)
    view = reporun(cwd, "cli", "github", "--", "issue", "view", num, "--json", "title,labels,comments")
    v = json.loads(view.stdout) if view.returncode == 0 else {}
    check("view shows the title, label and comment",
          v.get("title") == "first" and len(v.get("comments", [])) == 1 and v["labels"][0]["name"] == "bug", view.stdout)

    print("  -- sub-issue")
    child = gh(cwd, "issue", "create", "--title", "child", "--body", "x").stdout.strip().rsplit("/", 1)[-1]
    cid = json.loads(reporun(cwd, "api", "github", "GET", f"/repos/{slug}/issues/{child}").stdout)["id"]
    body = Path(tempfile.mkdtemp()) / "sub.json"
    body.write_text(json.dumps({"sub_issue_id": cid}))
    sub = reporun(cwd, "api", "github", "POST", f"/repos/{slug}/issues/{num}/sub_issues", "--data", str(body), "--write")
    check("sub-issue linked", sub.returncode == 0, sub.stdout + sub.stderr)
    check("close", gh(cwd, "issue", "close", num).returncode == 0)
    closed = json.loads(reporun(cwd, "cli", "github", "--", "issue", "view", num, "--json", "state").stdout)
    check("closed", closed.get("state") == "CLOSED")

    print("  -- pull requests")
    sh("git", "switch", "-q", "-c", "feat/one", cwd=cwd)
    commit(cwd, "one")
    sh("git", "push", "-q", "-u", "origin", "feat/one", cwd=cwd)
    pr = gh(cwd, "pr", "create", "--base", "main", "--head", "feat/one", "--title", "feat: one", "--body", "x")
    n1 = pr.stdout.strip().rsplit("/", 1)[-1]
    check("pr create", pr.returncode == 0 and n1.isdigit(), pr.stdout + pr.stderr)
    m = gh(cwd, "pr", "merge", n1, "--rebase", "--delete-branch")
    check("rebase merge", m.returncode == 0, m.stdout + m.stderr)

    sh("git", "switch", "-q", "main", cwd=cwd)
    sh("git", "pull", "-q", "--ff-only", cwd=cwd)
    sh("git", "switch", "-q", "-c", "feat/four", cwd=cwd)
    for part in ("a", "b", "c", "d"):
        commit(cwd, "four-" + part)
    sh("git", "push", "-q", "-u", "origin", "feat/four", cwd=cwd)
    pr = gh(cwd, "pr", "create", "--base", "main", "--head", "feat/four", "--title", "feat: four", "--body", "x")
    n4 = pr.stdout.strip().rsplit("/", 1)[-1]
    m = gh(cwd, "pr", "merge", n4, "--merge", "--delete-branch")
    check("merge commit (four commits)", m.returncode == 0, m.stdout + m.stderr)
    sh("git", "switch", "-q", "main", cwd=cwd)
    sh("git", "pull", "-q", "--ff-only", cwd=cwd)
    parents = sh("git", "log", "-1", "--format=%p", cwd=cwd).split()
    check("main's tip is a merge commit", len(parents) == 2, str(parents))

    print("  -- batch")
    plan = Path(tempfile.mkdtemp()) / "plan.json"
    plan.write_text(json.dumps({"platform": "github", "steps": [
        {"name": "list", "cli": ["issue", "list", "--json", "number"]},
        {"name": "epic", "cli": ["issue", "create", "--title", "epic", "--body", "x"]},
        {"name": "task", "cli": ["issue", "create", "--title", "task of {{epic.url}}", "--body", "x"]},
    ]}))
    dry = reporun(cwd, "batch", str(plan), "--dry-run")
    check("dry run lists the steps and creates nothing", dry.returncode == 0 and "epic" in dry.stdout, dry.stdout + dry.stderr)
    refused = reporun(cwd, "batch", str(plan))
    check("a batch with writes is refused without --write", refused.returncode == 3, refused.stdout + refused.stderr)
    done = reporun(cwd, "batch", str(plan), "--write")
    check("batch runs all steps", done.returncode == 0, done.stdout + done.stderr)
    titles = reporun(cwd, "cli", "github", "--", "issue", "list", "--json", "title", "-q", ".[].title").stdout
    check("a step's output feeds the next step's title", "task of https://github.com/" in titles, titles)


if __name__ == "__main__":
    sys.exit(main())
