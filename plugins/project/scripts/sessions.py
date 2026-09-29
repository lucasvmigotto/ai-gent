#!/usr/bin/env python3
"""Digest a repository's past agent sessions — Claude Code and OpenCode alike.

Used by project:recap. Reads Claude Code transcripts
(~/.claude/projects/<slug>/*.jsonl) and OpenCode sessions (`opencode session
list/export`), keeps only what a recap needs — the user's requests, the
agent's last words, the files it changed, the commands it ran, compaction
summaries, whether it stopped mid-task — and redacts secrets. Tool results
are never included. Standard library only.

  sessions.py list   [--repo DIR] [--json]
  sessions.py digest [--repo DIR] [--tool claude|opencode] [--session ID] [--json]

The session running this command is skipped (Claude Code: $CLAUDE_CODE_SESSION_ID;
OpenCode: $OPENCODE_SESSION_ID when set).
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

EDIT_TOOLS = {"edit", "write", "multiedit", "notebookedit", "patch", "apply_patch"}
SHELL_TOOLS = {"bash", "shell"}
SECRET_PATTERNS = [
    (re.compile(r"-----BEGIN [A-Z ]*PRIVATE KEY-----.*?-----END [A-Z ]*PRIVATE KEY-----", re.S), "<private key>"),
    (re.compile(r"(?i)\b([A-Z0-9_]*(?:PASSWORD|PASSWD|PWD|SECRET|TOKEN|API_?KEY|ACCESS_?KEY|PAT))\b(\s*[=:]\s*)(\"[^\"]*\"|'[^']*'|\S+)"),
     r"\1\2<redacted>"),
    (re.compile(r"(?i)\b(bearer|basic)\s+[A-Za-z0-9._~+/=-]{12,}"), r"\1 <redacted>"),
    (re.compile(r"\b(gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|sk-[A-Za-z0-9_-]{20,}|AKIA[0-9A-Z]{16}|xox[baprs]-[A-Za-z0-9-]{10,})"),
     "<token>"),
    (re.compile(r"(?i)\b([a-z][a-z0-9+.-]*://[^:/@\s]+):[^@\s]+@"), r"\1:<redacted>@"),
]


def redact(text: str) -> str:
    for pattern, repl in SECRET_PATTERNS:
        text = pattern.sub(repl, text)
    return text


def clip(text: str, limit: int) -> str:
    text = redact(" ".join(str(text).split()))
    return text if len(text) <= limit else text[: limit - 1] + "…"


def iso(ms_or_str) -> str:
    if isinstance(ms_or_str, (int, float)):
        return dt.datetime.fromtimestamp(ms_or_str / 1000, dt.timezone.utc).astimezone().isoformat(timespec="minutes")
    try:
        return dt.datetime.fromisoformat(str(ms_or_str).replace("Z", "+00:00")).astimezone().isoformat(timespec="minutes")
    except ValueError:
        return str(ms_or_str)


def repo_root(start: Path) -> Path:
    out = subprocess.run(["git", "-C", str(start), "rev-parse", "--show-toplevel"], capture_output=True, text=True)
    return Path(out.stdout.strip()).resolve() if out.returncode == 0 else start.resolve()


def inside(path: str, root: Path) -> bool:
    try:
        Path(path).resolve().relative_to(root)
        return True
    except (ValueError, OSError):
        return False


# ------------------------------------------------------------- Claude Code


def claude_home() -> Path:
    return Path(os.environ.get("CLAUDE_CONFIG_DIR") or Path.home() / ".claude")


def claude_slug(path: Path) -> str:
    return re.sub(r"[^A-Za-z0-9]", "-", str(path))


def claude_files(root: Path) -> list[Path]:
    """Transcripts started in the repository or one of its subdirectories."""
    projects = claude_home() / "projects"
    if not projects.is_dir():
        return []
    slug = claude_slug(root)
    files = []
    for d in projects.iterdir():
        if d.is_dir() and (d.name == slug or d.name.startswith(slug + "-")):
            files += [f for f in d.glob("*.jsonl") if d.name == slug or claude_cwd(f, root)]
    return files


def claude_cwd(path: Path, root: Path) -> bool:
    with path.open(errors="replace") as f:
        for _, line in zip(range(50), f):
            cwd = json.loads(line).get("cwd") if '"cwd"' in line else None
            if cwd:
                return inside(cwd, root)
    return False


def claude_events(path: Path) -> list[dict]:
    """Normalized events: {role, time, text | tool+input, compaction}."""
    events = []
    with path.open(errors="replace") as f:
        for line in f:
            try:
                rec = json.loads(line)
            except json.JSONDecodeError:
                continue
            if rec.get("type") not in ("user", "assistant") or rec.get("isSidechain") or rec.get("isMeta"):
                continue
            msg, when = rec.get("message") or {}, rec.get("timestamp")
            base = {"time": when, "branch": rec.get("gitBranch")}
            content = msg.get("content")
            if rec.get("isCompactSummary"):
                events.append({**base, "role": "compaction", "text": content if isinstance(content, str) else
                               " ".join(p.get("text", "") for p in content or [] if isinstance(p, dict))})
                continue
            if isinstance(content, str):
                if not content.startswith("<"):  # command wrappers and caveats
                    events.append({**base, "role": rec["type"], "text": content})
                continue
            for part in content or []:
                kind = part.get("type")
                if kind == "text" and part.get("text", "").strip() and not part["text"].startswith("<"):
                    events.append({**base, "role": rec["type"], "text": part["text"]})
                elif kind == "tool_use":
                    events.append({**base, "role": "tool", "tool": part.get("name", ""), "input": part.get("input") or {}})
                elif kind == "tool_result":
                    events.append({**base, "role": "tool_result"})
    return events


def claude_sessions(root: Path) -> list[dict]:
    current = os.environ.get("CLAUDE_CODE_SESSION_ID")
    out = []
    for f in claude_files(root):
        if f.stem == current:
            continue
        st = f.stat()
        out.append({"tool": "claude", "id": f.stem, "updated": iso(st.st_mtime * 1000), "_mtime": st.st_mtime,
                    "_path": str(f), "title": claude_title(f)})
    return out


def claude_title(path: Path) -> str:
    with path.open(errors="replace") as f:
        for line in f:
            rec = json.loads(line) if line.strip().startswith("{") else {}
            if rec.get("type") == "summary" and rec.get("summary"):
                return clip(rec["summary"], 90)
            if rec.get("type") == "user" and not rec.get("isMeta"):
                c = (rec.get("message") or {}).get("content")
                text = c if isinstance(c, str) else " ".join(p.get("text", "") for p in c or [] if isinstance(p, dict))
                if text.strip() and not text.startswith("<"):
                    return clip(text, 90)
    return ""


# ---------------------------------------------------------------- OpenCode


def opencode(args: list[str], root: Path) -> str | None:
    exe = shutil.which("opencode")
    if not exe:
        return None
    # into a file, not a pipe: OpenCode 2.0 truncates a large export written to a pipe
    with tempfile.TemporaryFile() as buf:
        try:
            out = subprocess.run([exe, *args], cwd=root, stdout=buf, stderr=subprocess.DEVNULL, timeout=60)
        except (subprocess.TimeoutExpired, OSError):
            return None
        buf.seek(0)
        return buf.read().decode(errors="replace") if out.returncode == 0 else None


def opencode_sessions(root: Path) -> list[dict]:
    raw = opencode(["session", "list", "--format", "json", "-n", "20"], root)
    try:
        items = json.loads(raw) if raw else []
    except json.JSONDecodeError:
        return []
    current = os.environ.get("OPENCODE_SESSION_ID")
    return [{"tool": "opencode", "id": s["id"], "updated": iso(s.get("updated", 0)), "_mtime": s.get("updated", 0) / 1000,
             "title": clip(s.get("title", ""), 90)}
            for s in items if s.get("id") != current and inside(s.get("directory", str(root)), root)]


def opencode_events(session_id: str, root: Path) -> list[dict]:
    raw = opencode(["session", "export", session_id], root)
    try:
        data = json.loads(raw) if raw else {}
    except json.JSONDecodeError:
        return []
    events = []
    for m in data.get("messages", []):
        when, kind = (m.get("time") or {}).get("created"), m.get("type")
        if kind == "compaction":
            events.append({"time": when, "role": "compaction", "text": m.get("summary", "")})
        elif kind == "user" and m.get("text"):
            events.append({"time": when, "role": "user", "text": m["text"]})
        elif kind == "assistant":
            for part in m.get("content") or []:
                if part.get("type") == "text" and part.get("text", "").strip():
                    events.append({"time": when, "role": "assistant", "text": part["text"]})
                elif part.get("type") == "tool":
                    state = part.get("state") or {}
                    events.append({"time": when, "role": "tool", "tool": part.get("name", ""),
                                   "input": state.get("input") if isinstance(state.get("input"), dict) else {}})
                    if state.get("status") in ("completed", "error"):
                        events.append({"time": when, "role": "tool_result"})
    return events


# ------------------------------------------------------------------ digest


def digest(session: dict, events: list[dict], root: Path) -> dict:
    users = [e for e in events if e["role"] == "user"]
    said = [e for e in events if e["role"] == "assistant"]
    tools = [e for e in events if e["role"] == "tool"]
    files: dict[str, str] = {}
    commands = []
    for e in tools:
        name, inp = e["tool"].lower(), e.get("input") or {}
        path = inp.get("file_path") or inp.get("filePath") or inp.get("path") or inp.get("notebook_path")
        if name in EDIT_TOOLS and path and "/scratchpad/" not in path:
            p = Path(path)
            files[str(p.relative_to(root)) if inside(path, root) and p.is_absolute() else path] = iso(e["time"]) if e["time"] else ""
        if name in SHELL_TOOLS and inp.get("command"):
            commands.append(clip(inp["command"], 160))
    last = events[-1] if events else {}
    if last.get("role") == "tool":
        ending = "stopped mid-task: the last tool call has no result (interrupted, out of tokens or crashed)"
    elif last.get("role") == "assistant" and last.get("text", "").rstrip().endswith("?"):
        ending = "the agent asked a question that was never answered"
    elif last.get("role") == "user":
        ending = "the last user message got no answer"
    else:
        ending = "ended normally"
    compactions = [e for e in events if e["role"] == "compaction"]
    branches = [e.get("branch") for e in events if e.get("branch") and e.get("branch") != "HEAD"]
    return {
        "tool": session["tool"], "id": session["id"], "title": session.get("title", ""),
        "started": iso(events[0]["time"]) if events and events[0].get("time") else "",
        "ended": session["updated"], "branch": branches[-1] if branches else None, "ending": ending,
        "requests": [clip(e["text"], 400) for e in users[-6:]],
        "last_words": [clip(e["text"], 1500) for e in said[-3:]],
        "compaction_summary": clip(compactions[-1]["text"], 4000) if compactions else None,
        "files_changed": sorted(files),
        "commands": commands[-20:],
        "counts": {"requests": len(users), "tool_calls": len(tools), "files_changed": len(files), "compactions": len(compactions)},
    }


def all_sessions(root: Path, tool: str | None) -> list[dict]:
    found = []
    if tool in (None, "claude"):
        found += claude_sessions(root)
    if tool in (None, "opencode"):
        found += opencode_sessions(root)
    return sorted(found, key=lambda s: s["_mtime"], reverse=True)


def public(s: dict) -> dict:
    return {k: v for k, v in s.items() if not k.startswith("_")}


def print_digest(d: dict) -> None:
    print(f"# {d['tool']} session {d['id']} — {d['title']}")
    print(f"started {d['started']} · ended {d['ended']} · branch {d['branch'] or '?'} · {d['ending']}")
    c = d["counts"]
    print(f"{c['requests']} requests · {c['tool_calls']} tool calls · {c['files_changed']} files changed · {c['compactions']} compactions")
    if d["compaction_summary"]:
        print("\n## Last compaction summary\n" + d["compaction_summary"])
    print("\n## Last requests")
    for r in d["requests"]:
        print(f"- {r}")
    print("\n## Agent's last words")
    for w in d["last_words"]:
        print(f"- {w}")
    if d["files_changed"]:
        print("\n## Files the agent changed\n" + "\n".join(f"- {f}" for f in d["files_changed"]))
    if d["commands"]:
        print("\n## Last commands\n" + "\n".join(f"- `{c}`" for c in d["commands"]))


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    for name in ("list", "digest"):
        sp = sub.add_parser(name)
        sp.add_argument("--repo", default=".")
        sp.add_argument("--tool", choices=("claude", "opencode"))
        sp.add_argument("--json", action="store_true")
        if name == "digest":
            sp.add_argument("--session", help="a session id from `list` (default: the most recent)")
    args = ap.parse_args(argv)
    root = repo_root(Path(args.repo))
    sessions = all_sessions(root, args.tool)
    if args.cmd == "list":
        if args.json:
            print(json.dumps([public(s) for s in sessions], indent=2))
        else:
            for s in sessions:
                print(f"{s['updated']}  {s['tool']:<8}  {s['id']}  {s['title']}")
            if not sessions:
                print(f"no earlier sessions for {root}")
        return 0
    chosen = next((s for s in sessions if s["id"] == args.session), None) if args.session else (sessions[0] if sessions else None)
    if not chosen:
        print(f"no {'session ' + args.session if args.session else 'earlier session'} for {root}", file=sys.stderr)
        return 1
    events = claude_events(Path(chosen["_path"])) if chosen["tool"] == "claude" else opencode_events(chosen["id"], root)
    d = digest(chosen, events, root)
    print(json.dumps(d, indent=2, ensure_ascii=False)) if args.json else print_digest(d)
    return 0


if __name__ == "__main__":
    sys.exit(main())
