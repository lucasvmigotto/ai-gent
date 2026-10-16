#!/usr/bin/env python3
"""specsync — what a feature's specs say its tracker items should be.

Reads specs/NNN-<feature>/ (spec.md stories, tasks.md checkboxes),
specs/README.md (the feature's statuses) and the feature's tracker.md (the
items already created), and prints the plan `repo:sync` applies through
`reporun batch`: one item per feature, one per user story with its tasks as
a checklist, each with the state the repository implies.

  specsync plan <NNN|specs/NNN-feature> [--json]
  specsync record <NNN|specs/NNN-feature> --platform P --from batch-output.json
  specsync features [--json]

States: todo, doing, review, done. A story is todo until one of its tasks is
checked, doing while some are, review when all are, and done when the
feature is Verified. The feature follows specs/README.md (Planned → todo,
In progress/Partial → doing, Implemented → review, Verified → done; the
least advanced side wins). See plugins/repo/references/sync.md.
Python 3.9+ stdlib only.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import re
import sys
from pathlib import Path

STATES = ("todo", "doing", "review", "done")
STATUS_STATE = {"planned": "todo", "in progress": "doing", "partial": "doing", "implemented": "review",
                "verified": "done"}
MARKER = "<!-- ai-gent:sync {path} {key} -->"
TASK_RE = re.compile(r"^\s*- \[( |x|X)\] (T\d+[a-z]?)\b\s*(.*)$")
STORY_HEAD_RE = re.compile(r"^###\s+User Story\s+(\d+)\s*[-—–:]\s*(.+?)\s*(?:\(Priority:\s*(P\d+)\))?\s*$", re.I)
PHASE_STORY_RE = re.compile(r"^#{2,3}\s+.*?User Story\s+(\d+)\b", re.I)
LAYER_RE = re.compile(r"^##\s+(Frontend|Backend|QA)\b", re.I)


class Failed(Exception):
    pass


def specs_dir() -> Path:
    return Path("specs")


def feature_dir(arg: str) -> Path:
    p = Path(arg)
    if p.is_dir():
        return p
    hits = sorted(specs_dir().glob(f"{arg}-*")) if re.fullmatch(r"\d{3}", arg) else []
    if len(hits) == 1:
        return hits[0]
    raise Failed(f"no feature {arg!r} under specs/ (pass NNN or the directory)")


# --------------------------------------------------------------------- parse


def parse_spec(path: Path) -> tuple[str, list[dict]]:
    """The feature's title and its user stories with their text."""
    if not path.exists():
        return "", []
    title, stories, cur = "", [], None
    for line in path.read_text().splitlines():
        if not title and line.startswith("# "):
            title = re.sub(r"^#\s+(Feature Specification:\s*)?(\d{3}\s*[-—–:]\s*)?", "", line).strip()
        m = STORY_HEAD_RE.match(line)
        if m:
            cur = {"n": int(m.group(1)), "title": m.group(2).strip(), "priority": m.group(3) or "", "text": []}
            stories.append(cur)
            continue
        if cur is not None:
            if line.startswith("## ") or (line.startswith("### ") and not STORY_HEAD_RE.match(line)):
                cur = None
            else:
                cur["text"].append(line)
    for s in stories:
        s["text"] = "\n".join(s["text"]).strip()
    if not stories:
        stories = listed_stories(path.read_text())
    return title, stories


LISTED_RE = re.compile(r"^(\d+)\.\s+\*\*(.+?)\*\*\s*[:—–-]?\s*(.*)$")


def listed_stories(text: str) -> list[dict]:
    """`project:introspec`'s shape: a numbered list under `## Stories`
    (`1. **Google login**: as a user …`, details indented below)."""
    stories, cur, inside = [], None, False
    for line in text.splitlines():
        if line.startswith("## "):
            inside = line[3:].strip().lower().startswith(("stories", "user stories"))
            cur = None
            continue
        if not inside:
            continue
        m = LISTED_RE.match(line)
        if m:
            cur = {"n": int(m.group(1)), "title": m.group(2).strip().rstrip(":"), "priority": "",
                   "text": [m.group(3).strip()] if m.group(3).strip() else []}
            stories.append(cur)
        elif cur is not None and (line.startswith((" ", "\t")) or not line.strip()):
            cur["text"].append(line[3:] if line.startswith("   ") else line.strip())
        elif line.strip():
            cur = None
    for s in stories:
        s["text"] = "\n".join(s["text"]).strip()
    return stories


def parse_tasks(path: Path) -> list[dict]:
    """Tasks with their story (a [USn] tag, else the phase heading) and layer."""
    if not path.exists():
        return []
    tasks, phase_story, layer = [], None, ""
    for line in path.read_text().splitlines():
        if line.startswith("#"):
            lm = LAYER_RE.match(line)
            if lm:
                layer, phase_story = lm.group(1).capitalize() if lm.group(1).lower() != "qa" else "QA", None
                continue
            pm = PHASE_STORY_RE.match(line)
            phase_story = int(pm.group(1)) if pm else (None if line.startswith("## ") or line.startswith("### Phase") else phase_story)
            continue
        m = TASK_RE.match(line)
        if not m:
            continue
        text = m.group(3)
        us = re.search(r"\[US(\d+)\]", text)
        story = int(us.group(1)) if us else phase_story
        desc = re.sub(r"\[(P|US\d+|TUI|CLI)\]\s*", "", text).strip()
        tasks.append({"id": m.group(2), "done": m.group(1) != " ", "story": story, "layer": layer, "text": desc})
    return tasks


def readme_row(number: str) -> dict:
    """The feature's statuses from specs/README.md: {side: status}."""
    path = specs_dir() / "README.md"
    if not path.exists():
        return {}
    header = None
    for line in path.read_text().splitlines():
        if not line.startswith("|"):
            header = None if not line.strip() else header
            continue
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        if header is None:
            header = [c.lower() for c in cells]
            continue
        if set("".join(cells)) <= set("-: "):
            continue
        if cells and cells[0].lstrip("0") == number.lstrip("0") or (cells and cells[0] == number):
            row = dict(zip(header, cells))
            return {k: v for k, v in row.items() if k in ("frontend", "backend", "status", "qa", "notes")}
    return {}


RETIRED = ("retired", "superseded", "removed", "dropped", "cancelled", "canceled")


def status_word(value: str) -> str:
    """`Implemented (containers; …)` → `implemented`."""
    v = value.lower().strip("*_ ")
    for word in (*STATUS_STATE, *RETIRED):
        if v.startswith(word):
            return word
    return ""


def feature_state(statuses: dict, tasks: list[dict]) -> str:
    states = []
    for side, value in statuses.items():
        v = status_word(value)
        if v in STATUS_STATE:
            states.append(STATUS_STATE[v])
    if states:
        return min(states, key=STATES.index)
    return tasks_state(tasks)


def tasks_state(tasks: list[dict]) -> str:
    if not tasks or not any(t["done"] for t in tasks):
        return "todo"
    return "review" if all(t["done"] for t in tasks) else "doing"


# ----------------------------------------------------------------- tracker.md


def read_tracker(fdir: Path) -> dict:
    path = fdir / "tracker.md"
    out: dict = {"platform": "", "items": {}}
    if not path.exists():
        return out
    for line in path.read_text().splitlines():
        m = re.match(r"^- Platform:\s*(\S+)", line)
        if m:
            out["platform"] = m.group(1)
        if line.startswith("|") and not line.startswith("| Key") and not set(line) <= set("|-: "):
            cells = [c.strip() for c in line.strip().strip("|").split("|")]
            if len(cells) >= 6:
                out["items"][cells[0]] = {"ref": cells[1], "id": cells[2], "state": cells[3], "hash": cells[4],
                                          "url": cells[5]}
    return out


def write_tracker(fdir: Path, platform: str, where: str, items: dict) -> None:
    def order(k):
        return (0, 0) if k == "feature" else (1, int(k[2:]) if k[2:].isdigit() else 0)
    lines = [f"# Tracker — {fdir.name}", "",
             "Written by `repo:sync` (`plugins/repo/references/sync.md`): the tracker items that",
             "mirror this feature. Commits and pull requests reference the `Item` column.", "",
             f"- Platform: {platform}{f' ({where})' if where else ''}",
             f"- Last sync: {dt.date.today().isoformat()}", "",
             "| Key | Item | Id | State | Hash | URL |", "|---|---|---|---|---|---|"]
    for key in sorted(items, key=order):
        i = items[key]
        lines.append(f"| {key} | {i.get('ref', '')} | {i.get('id', '')} | {i.get('state', '')} | {i.get('hash', '')} | {i.get('url', '')} |")
    (fdir / "tracker.md").write_text("\n".join(lines) + "\n")


# ---------------------------------------------------------------------- plan


def to_html(md: str) -> str:
    """Just enough Markdown → HTML for Azure DevOps fields: headings, lists, checkboxes, code, bold."""
    import html
    out: list[str] = []
    para: list[str] = []
    in_list = False

    def inline(t: str) -> str:
        t = html.escape(t)
        t = re.sub(r"`([^`]+)`", r"<code>\1</code>", t)
        return re.sub(r"\*\*([^*]+)\*\*", r"<b>\1</b>", t)

    def flush():
        nonlocal in_list
        if para:
            out.append("<p>" + " ".join(para) + "</p>")
            para.clear()
        if in_list:
            out.append("</ul>")
            in_list = False

    for line in md.splitlines():
        if line.startswith("<!--"):
            flush()
            out.append(line)
            continue
        m = re.match(r"^\s*(?:-|\d+\.) (?:\[( |x)\] )?(.*)$", line)
        if m:
            if para:
                out.append("<p>" + " ".join(para) + "</p>")
                para.clear()
            if not in_list:
                out.append("<ul>")
                in_list = True
            box = {" ": "☐ ", "x": "☑ "}.get(m.group(1) or "", "")
            out.append(f"<li>{box}{inline(m.group(2))}</li>")
            continue
        h = re.match(r"^(#{1,6})\s+(.*)$", line)
        if h or not line.strip():
            flush()
            if h:
                n = min(len(h.group(1)) + 1, 6)
                out.append(f"<h{n}>{inline(h.group(2))}</h{n}>")
            continue
        if in_list and line.startswith("  "):
            out[-1] = out[-1][:-5] + " " + inline(line.strip()) + "</li>"
            continue
        if in_list:
            out.append("</ul>")
            in_list = False
        para.append(inline(line.strip()))
    flush()
    return "\n".join(out)


def digest(*parts: str) -> str:
    return hashlib.sha1("\n".join(parts).encode()).hexdigest()[:10]


def checklist(tasks: list[dict]) -> str:
    return "\n".join(f"- [{'x' if t['done'] else ' '}] {t['id']} {t['text']}" + (f" ({t['layer']})" if t["layer"] else "")
                     for t in tasks)


def build_plan(fdir: Path) -> dict:
    number = fdir.name.split("-", 1)[0]
    name = fdir.name.split("-", 1)[1] if "-" in fdir.name else fdir.name
    title, stories = parse_spec(fdir / "spec.md")
    tasks = parse_tasks(fdir / "tasks.md")
    row = readme_row(number)
    statuses = {k: v for k, v in row.items() if k != "notes"}
    retired = bool(row) and all(status_word(v) in RETIRED or status_word(v) == "" for v in row.values()) \
        and any(status_word(v) in RETIRED for v in row.values())
    fstate = feature_state(statuses, tasks)
    tracker = read_tracker(fdir)
    rel = fdir.as_posix()
    known = {s["n"] for s in stories}
    items = []

    loose = [t for t in tasks if t["story"] not in known]
    fbody = [MARKER.format(path=rel, key="feature"),
             f"Feature **{number} {title or name}** — `{rel}/spec.md`", ""]
    if statuses:
        fbody += [" · ".join(f"{k.capitalize()}: {v}" for k, v in statuses.items()), ""]
    if stories:
        fbody += ["Stories:", *[f"- US{s['n']} {s['title']}" + (f" ({s['priority']})" if s["priority"] else "") for s in stories], ""]
    if loose:
        fbody += ["Tasks outside the stories:", checklist(loose), ""]
    ftitle = f"[{number}] {title or name}"
    items.append({"key": "feature", "kind": "feature", "title": ftitle, "state": fstate, "parent": None,
                  "body": "\n".join(fbody).strip() + "\n"})

    for s in stories:
        st = [t for t in tasks if t["story"] == s["n"]]
        state = "done" if fstate == "done" else (tasks_state(st) if st else ("todo" if fstate == "todo" else fstate))
        text = s["text"] if len(s["text"]) <= 6000 else s["text"][:6000].rsplit("\n", 1)[0] + "\n\n…"
        body = [MARKER.format(path=rel, key=f"US{s['n']}"),
                f"User story {s['n']} of **{number} {title or name}**" + (f" · {s['priority']}" if s["priority"] else "")
                + f" — `{rel}/spec.md`", "", text, ""]
        if st:
            body += ["### Tasks", checklist(st), ""]
        items.append({"key": f"US{s['n']}", "kind": "story", "title": f"[{number}] US{s['n']}: {s['title']}",
                      "state": state, "parent": "feature", "priority": s["priority"],
                      "body": "\n".join(body).strip() + "\n"})

    for item in items:
        item["hash"] = digest(item["title"], item["body"])
        item["body_html"] = to_html(item["body"])
        prev = tracker["items"].get(item["key"])
        item["existing"] = prev
        if not prev or not prev.get("ref"):
            item["action"] = "create"
        else:
            changes = []
            if prev.get("hash") != item["hash"]:
                changes.append("update")
            if prev.get("state") != item["state"]:
                changes.append("state")
            item["action"] = "+".join(changes) or "none"
    if retired:  # nothing to track; items already created are reported, never closed unasked
        items = []
    orphans = sorted(set(tracker["items"]) - {i["key"] for i in items})
    return {"feature": fdir.name, "retired": retired, "path": rel, "platform": tracker["platform"], "statuses": statuses,
            "items": items, "orphans": orphans}


def cmd_plan(args) -> int:
    plan = build_plan(feature_dir(args.feature))
    if args.json:
        print(json.dumps(plan, indent=2, ensure_ascii=False))
        return 0
    if plan["retired"]:
        print(f"{plan['feature']} · retired in specs/README.md — nothing to sync")
    print(f"{plan['feature']} · tracker: {plan['platform'] or 'not synced yet'}"
          + (" · " + ", ".join(f"{k} {v}" for k, v in plan["statuses"].items()) if plan["statuses"] else ""))
    for i in plan["items"]:
        ref = (i["existing"] or {}).get("ref", "")
        was = (i["existing"] or {}).get("state", "")
        state = f"{was} → {i['state']}" if was and was != i["state"] else i["state"]
        print(f"  {i['key']:<8} {i['action']:<13} {state:<16} {ref:<10} {i['title']}")
    for k in plan["orphans"]:
        print(f"  {k:<8} orphan        in tracker.md, gone from the specs — report it; never delete the item")
    pending = [i for i in plan["items"] if i["action"] != "none"]
    print(f"\n{len(pending)} item(s) to create or change" if pending else "\nin sync")
    return 0


def cmd_record(args) -> int:
    """Merge a `reporun batch` output's records into tracker.md."""
    fdir = feature_dir(args.feature)
    data = json.loads(Path(args.from_file).read_text())
    records = data.get("records", data) if isinstance(data, dict) else data
    tracker = read_tracker(fdir)
    items = tracker["items"]
    for r in records:
        key = r.get("key")
        if not key:
            continue
        cur = items.setdefault(key, {})
        for f in ("ref", "id", "state", "hash", "url"):
            if r.get(f) not in (None, ""):
                cur[f] = str(r[f])
    write_tracker(fdir, args.platform, args.where or "", items)
    print(f"{fdir}/tracker.md: {len(items)} item(s)")
    return 0


def cmd_features(args) -> int:
    rows = []
    for fdir in sorted(specs_dir().glob("[0-9][0-9][0-9]-*")):
        if fdir.is_dir():
            plan = build_plan(fdir)
            pending = sum(1 for i in plan["items"] if i["action"] != "none")
            rows.append({"feature": fdir.name, "synced": bool(plan["platform"]), "pending": pending,
                         "state": "retired" if plan["retired"] else plan["items"][0]["state"]})
    if args.json:
        print(json.dumps(rows, indent=2))
    else:
        for r in rows:
            print(f"{r['feature']:<40} {r['state']:<7} {'synced' if r['synced'] else 'not synced':<11} {r['pending']} pending")
    return 0


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(prog="specsync", description=__doc__.split("\n\n")[0])
    sub = ap.add_subparsers(dest="command", required=True)
    s = sub.add_parser("plan", help="the items a feature's specs imply, and what changed since the last sync")
    s.add_argument("feature")
    s.add_argument("--json", action="store_true")
    s.set_defaults(func=cmd_plan)
    s = sub.add_parser("record", help="write a batch's results into the feature's tracker.md")
    s.add_argument("feature")
    s.add_argument("--platform", required=True)
    s.add_argument("--where", help="owner/repo, org/project, Jira project key …")
    s.add_argument("--from", dest="from_file", required=True)
    s.set_defaults(func=cmd_record)
    s = sub.add_parser("features", help="every feature's state and pending changes")
    s.add_argument("--json", action="store_true")
    s.set_defaults(func=cmd_features)
    args = ap.parse_args(argv)
    try:
        return args.func(args)
    except Failed as e:
        print(f"error: {e}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
