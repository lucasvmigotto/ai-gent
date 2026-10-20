#!/usr/bin/env python3
"""Token usage report for a project's Claude Code sessions: the coordinator (main thread),
every subagent, and stage snapshots — the concrete tool behind orchestrate's token-economy
ledger (../skills/orchestrate/references/token-economy.md).

Reads the session JSONL transcripts in every Claude config dir (~/.claude*/projects/<project>/),
deduplicates assistant messages by message id (streamed chunks and account switches repeat
them; local `<synthetic>` placeholders are skipped), and sums each message's final usage
(input, output, cache write, cache read — the API's own numbers, never an estimate).

Sessions are auto-discovered from the project's own transcript directories unless named with
--session. Stages are markers (label + timestamp) kept in a snapshots file; a stage's usage is
every message timestamped after the previous marker and up to its own, recomputed from the
transcripts on every run so it never drifts.

  token_report.py report --project DIR [--session ID]... --out PATH [--snapshots FILE]
  token_report.py tasks   --project DIR [--session ID]... --specs-dir specs --rollup-out PATH [--snapshots FILE]
  token_report.py snapshot --label "..." [--at ISO8601] [--snapshots FILE]
  token_report.py backfill-git --repo PATH|- [--ref origin/dev] [--snapshots FILE]   (- = read
      `git log --first-parent --reverse --format=%cI|%s` from stdin)

`report` writes the project-wide roll-up (by stage, by label, by agent) a project keeps at
docs/product/token-usage.md. `tasks` does the same and additionally links each subagent's cost
to the Spec Kit task ids its description names, writing one specs/<feature>/tokens.md per
touched feature (a record file beside tasks.md, never edited into it) plus an "Unmapped"
section in the roll-up for cards that name no task id.

Claude Code-specific: this reads Claude Code's own on-disk transcript format and the Agent
tool's subagent .meta.json sidecars. No equivalent exists on another agent.
"""
import argparse
import glob
import json
import os
import re
import subprocess
import sys
from collections import defaultdict
from datetime import datetime, timezone

FIELDS = ("input_tokens", "cache_creation_input_tokens", "cache_read_input_tokens", "output_tokens")
DEFAULT_SNAPSHOTS = "token-snapshots.json"
TASK_ID = re.compile(r"\bT\d{3,}\b")


def load_snapshots(path):
    if not os.path.exists(path):
        return []
    with open(path, encoding="utf-8") as fh:
        return json.load(fh)


def save_snapshots(path, snaps):
    snaps = sorted({(s["at"], s["label"]): s for s in snaps}.values(), key=lambda s: s["at"])
    with open(path, "w", encoding="utf-8") as fh:
        json.dump(snaps, fh, indent=1, ensure_ascii=False)
    return snaps


def to_utc(ts: str) -> str:
    return datetime.fromisoformat(ts.replace("Z", "+00:00")).astimezone(timezone.utc).strftime("%Y-%m-%dT%H:%M:%S")


def label_from_subject(subject: str) -> str:
    m = re.match(r"Merge branch '([^']+)' into", subject)
    return m.group(1) if m else subject


def cmd_snapshot(a):
    at = to_utc(a.at) if a.at else datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%S")
    snaps = save_snapshots(a.snapshots, load_snapshots(a.snapshots) + [{"at": at, "label": a.label, "source": "manual"}])
    print(f"snapshot '{a.label}' at {at} ({len(snaps)} total)")


def cmd_backfill(a):
    if a.repo == "-":  # git log piped on stdin (e.g. when the container has no git)
        out = sys.stdin.read()
    else:
        out = subprocess.run(["git", "-C", a.repo, "log", "--first-parent", "--reverse", "--format=%cI|%s", a.ref],
                             check=True, capture_output=True, text=True).stdout
    new = [{"at": to_utc(t), "label": label_from_subject(s), "source": "git"}
           for t, s in (line.split("|", 1) for line in out.splitlines() if "|" in line)]
    snaps = save_snapshots(a.snapshots, load_snapshots(a.snapshots) + new)
    print(f"backfilled {len(new)} markers from {a.ref} ({len(snaps)} total)")


def discover_sessions(roots):
    sids = set()
    for root in roots:
        for f in glob.glob(os.path.join(root, "*.jsonl")):
            sids.add(os.path.splitext(os.path.basename(f))[0])
    return sorted(sids)


def collect(path, msgs):
    """msgs: id -> (utc timestamp, model, usage); keeps the most complete record per message id."""
    with open(path, encoding="utf-8") as fh:
        for line in fh:
            try:
                rec = json.loads(line)
            except json.JSONDecodeError:
                continue
            if rec.get("type") != "assistant":
                continue
            m = rec.get("message") or {}
            u = m.get("usage")
            if not u or not m.get("id") or m.get("model") == "<synthetic>":
                continue
            prev = msgs.get(m["id"])
            if prev is None or u.get("output_tokens", 0) >= prev[2].get("output_tokens", 0):
                ts = rec.get("timestamp", "")
                msgs[m["id"]] = (to_utc(ts) if ts else "", m.get("model", "?"), u)


def label_of(desc: str) -> str:
    """A short grouping label from a subagent's description: its leading task id(s) or
    short code before a colon/dash, else a truncated fallback. No project vocabulary is
    hardcoded — the card's own description carries the label."""
    desc = desc.strip()
    ids = TASK_ID.findall(desc)
    if ids and re.match(r"^\s*T\d{3,}", desc):
        return ", ".join(ids)
    m = re.match(r"^([A-Za-z][A-Za-z0-9 /+]{1,28}?)\s*[:\-]", desc)
    if m:
        return m.group(1).strip()
    return desc[:24] + ("…" if len(desc) > 24 else "")


def summarize(msgs):
    tot = {k: 0 for k in FIELDS}
    models = defaultdict(int)
    ts = [t for t, _, _ in msgs if t]
    for _, model, u in msgs:
        for k in FIELDS:
            tot[k] += int(u.get(k) or 0)
        models[model] += 1
    tot["total"] = sum(tot[k] for k in FIELDS)
    tot["billable"] = tot["input_tokens"] + tot["cache_creation_input_tokens"] + tot["output_tokens"]
    tot["calls"] = len(msgs)
    tot["models"] = ", ".join(m for m, _ in sorted(models.items(), key=lambda x: -x[1])) or "?"
    tot["start"], tot["end"] = (min(ts), max(ts)) if ts else ("", "")
    return tot


def k(n: int) -> str:
    return f"{n / 1000:,.1f}k" if n < 1_000_000 else f"{n / 1_000_000:,.2f}M"


def short(t: str) -> str:
    return t[:16].replace("T", " ") if t else ""


def gather(roots, sessions):
    """Returns (coords, rows, tagged) — coords/rows are per-agent summaries, tagged is every
    (description, timestamp, model, usage) tuple, for stage windows."""
    main_msgs, agents, meta = defaultdict(dict), defaultdict(dict), {}
    for root in roots:
        for sid in sessions:
            main = os.path.join(root, f"{sid}.jsonl")
            if os.path.exists(main):
                collect(main, main_msgs[sid])
            for f in glob.glob(os.path.join(root, sid, "subagents", "agent-*.jsonl")):
                aid = os.path.basename(f)[len("agent-"):-len(".jsonl")]
                collect(f, agents[aid])
                mf = f[:-len(".jsonl")] + ".meta.json"
                if os.path.exists(mf) and aid not in meta:
                    with open(mf, encoding="utf-8") as fh:
                        meta[aid] = json.load(fh)

    tagged = [(f"coordinator {sid[:8]}", t, mdl, u) for sid, msgs in main_msgs.items() for t, mdl, u in msgs.values()]
    rows = []
    for aid, msgs in agents.items():
        if not msgs:
            continue
        desc = meta.get(aid, {}).get("description") or "(no description)"
        rows.append({"who": f"subagent {aid[:7]}", "desc": desc, "label": label_of(desc),
                     **summarize(list(msgs.values()))})
        tagged += [(desc, t, mdl, u) for t, mdl, u in msgs.values()]
    rows.sort(key=lambda r: r["start"])
    coords = [{"who": f"coordinator {sid[:8]} (main thread)",
               "desc": "Planning, interviews, reconciliation, reviews, merges, CI follow-up",
               "label": "Coordination", **summarize(list(main_msgs[sid].values()))}
              for sid in sessions if main_msgs.get(sid)]
    return coords, rows, tagged


def render(project, sessions, roots, coords, rows, tagged, snaps):
    everyone = rows + coords
    grand = {f: sum(r[f] for r in everyone) for f in FIELDS + ("total", "billable")}

    L = [f"# Token usage report — {project}", "",
         f"Generated {datetime.now().strftime('%Y-%m-%d %H:%M')} from session(s) "
         f"{', '.join(f'`{sid[:8]}`' for sid in sessions) if sessions else 'none'} "
         f"({len(roots)} config dirs, {len(rows)} subagents). API-reported usage, deduplicated by "
         "message id; local `<synthetic>` placeholders excluded.", "",
         "**Columns:** *In* uncached input · *Cache w* cache writes · *Cache r* cache reads · *Out* output · "
         "*Billable* = In + Cache w + Out (cache reads cost a small fraction of input, so this is the better cost proxy) · "
         "*Total* = all four. Times are UTC.", ""]

    if snaps:
        L += ["## By stage", "",
              "Each stage covers the usage after the previous marker up to its own. Markers come from "
              "git landings or manual snapshots.", "",
              "| # | Stage (ends with) | Window (UTC) | Billable | Total | Main contributors (billable) |",
              "|---|---|---|---|---|---|"]
        bounds = [(snaps[i - 1]["at"] if i else "", s["at"], s["label"]) for i, s in enumerate(snaps)]
        bounds.append((snaps[-1]["at"], "9999", "in progress (since last marker)"))
        n = 0
        for lo, hi, label in bounds:
            sel = [(who, u) for who, t, _, u in tagged if t and lo < t <= hi]
            if not sel:
                continue
            n += 1
            per_who = defaultdict(int)
            tot = bill = 0
            for who, u in sel:
                b = sum(int(u.get(f) or 0) for f in ("input_tokens", "cache_creation_input_tokens", "output_tokens"))
                tot += b + int(u.get("cache_read_input_tokens") or 0)
                bill += b
                per_who[who] += b
            top = "; ".join(f"{w} ({k(v)})" for w, v in sorted(per_who.items(), key=lambda x: -x[1])[:3])
            win = f"{short(lo) or 'start'} → {short(hi) if hi != '9999' else 'now'}"
            L.append(f"| {n} | {label} | {win} | {k(bill)} | {k(tot)} | {top} |")
        L.append("")

    L += ["## By label", "", "| Label | Agents | Model(s) | Billable | Total |", "|---|---|---|---|---|"]
    by_label = defaultdict(lambda: {"billable": 0, "total": 0, "n": 0, "models": set()})
    for r in everyone:
        p = by_label[r["label"]]
        p["billable"] += r["billable"]
        p["total"] += r["total"]
        p["n"] += 1
        p["models"].update(x.strip() for x in r["models"].split(","))
    for name, p in sorted(by_label.items(), key=lambda x: -x[1]["billable"]):
        L.append(f"| {name} | {p['n']} | {', '.join(sorted(p['models']))} | {k(p['billable'])} | {k(p['total'])} |")
    L += [f"| **All** | {len(everyone)} | | **{k(grand['billable'])}** | {k(grand['total'])} |", ""]

    L += ["## By agent", "",
          "| Started | Who | Label | Task | Model | Calls | Billable | Total |",
          "|---|---|---|---|---|---|---|---|"]
    for r in coords + rows:
        L.append(f"| {short(r['start'])} | {r['who']} | {r['label']} | {r['desc']} | {r['models']} | "
                 f"{r['calls']} | {k(r['billable'])} | {k(r['total'])} |")
    L += ["", "## Notes", "",
          "- A resumed subagent keeps its id, so its row covers every resume after a usage limit.",
          "- Each coordinator row spans one session's main thread across account switches (several config dirs).",
          "- Labels come from each card's own description (its leading task id or short code) — nothing is guessed "
          "from project-specific vocabulary.", ""]
    return L, grand


def cmd_report(a):
    roots = sorted(glob.glob(os.path.expanduser(f"~/.claude*/projects/{a.project}")))
    sessions = a.session or discover_sessions(roots)
    coords, rows, tagged = gather(roots, sessions)
    snaps = load_snapshots(a.snapshots)
    L, grand = render(a.project, sessions, roots, coords, rows, tagged, snaps)
    with open(a.out, "w", encoding="utf-8") as fh:
        fh.write("\n".join(L))
    print(f"wrote {a.out}: {len(rows)} subagents, {len(snaps)} stage markers, "
          f"billable {k(grand['billable'])}, total {k(grand['total'])}")


def feature_task_ids(specs_dir):
    """feature dir -> set of task ids defined in its tasks.md."""
    out = {}
    for tasks_md in sorted(glob.glob(os.path.join(specs_dir, "[0-9][0-9][0-9]-*", "tasks.md"))):
        feature = os.path.dirname(tasks_md)
        with open(tasks_md, encoding="utf-8") as fh:
            ids = set(TASK_ID.findall(fh.read()))
        if ids:
            out[feature] = ids
    return out


def cmd_tasks(a):
    roots = sorted(glob.glob(os.path.expanduser(f"~/.claude*/projects/{a.project}")))
    sessions = a.session or discover_sessions(roots)
    coords, rows, tagged = gather(roots, sessions)
    snaps = load_snapshots(a.snapshots)
    L, grand = render(a.project, sessions, roots, coords, rows, tagged, snaps)
    with open(a.rollup_out, "w", encoding="utf-8") as fh:
        fh.write("\n".join(L))

    by_feature = feature_task_ids(a.specs_dir)
    feature_of = {tid: feature for feature, ids in by_feature.items() for tid in ids}
    per_feature = defaultdict(list)
    unmapped = []
    for r in rows:
        ids = TASK_ID.findall(r["desc"])
        features = {feature_of[i] for i in ids if i in feature_of}
        if not features:
            unmapped.append(r)
            continue
        feature = sorted(features)[0]  # a card spanning features attributes its full cost to the first
        per_feature[feature].append(r)

    for feature, feat_rows in sorted(per_feature.items()):
        bill = sum(r["billable"] for r in feat_rows)
        tot = sum(r["total"] for r in feat_rows)
        out = [f"# Token usage — {os.path.basename(feature)}", "",
               f"Generated {datetime.now().strftime('%Y-%m-%d %H:%M')}. Linked to `tasks.md` by task id; "
               "a record file, never written into it. Cards spanning several features attribute their "
               "full cost to the first feature their task ids name.", "",
               "| Task(s) | Who | Model(s) | Calls | Billable | Total |",
               "|---|---|---|---|---|---|"]
        for r in sorted(feat_rows, key=lambda r: r["start"]):
            ids = ", ".join(TASK_ID.findall(r["desc"])) or "—"
            out.append(f"| {ids} | {r['who']} | {r['models']} | {r['calls']} | {k(r['billable'])} | {k(r['total'])} |")
        out += [f"| **Feature total** | | | | **{k(bill)}** | **{k(tot)}** |", ""]
        with open(os.path.join(feature, "tokens.md"), "w", encoding="utf-8") as fh:
            fh.write("\n".join(out))

    if unmapped:
        with open(a.rollup_out, "a", encoding="utf-8") as fh:
            fh.write("\n## Unmapped\n\nCards whose description names no task id, so their cost isn't linked "
                      "to any feature's `tasks.md`:\n\n| Who | Task | Billable | Total |\n|---|---|---|---|\n")
            for r in unmapped:
                fh.write(f"| {r['who']} | {r['desc']} | {k(r['billable'])} | {k(r['total'])} |\n")

    print(f"wrote {a.rollup_out} and {len(per_feature)} specs/*/tokens.md "
          f"({len(unmapped)} unmapped cards); billable {k(grand['billable'])}, total {k(grand['total'])}")


def build_parser():
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = p.add_subparsers(dest="cmd", required=True)
    r = sub.add_parser("report")
    r.add_argument("--project", required=True, help="the session directory slug under ~/.claude*/projects/ "
                   "(starts with '-': pass it as --project=<slug>)")
    r.add_argument("--session", action="append", help="repeat to restrict to specific sessions; default: all found")
    r.add_argument("--out", required=True)
    r.add_argument("--snapshots", default=DEFAULT_SNAPSHOTS)
    r.set_defaults(fn=cmd_report)
    t = sub.add_parser("tasks")
    t.add_argument("--project", required=True)
    t.add_argument("--session", action="append")
    t.add_argument("--specs-dir", default="specs")
    t.add_argument("--rollup-out", default=os.path.join("docs", "product", "token-usage.md"))
    t.add_argument("--snapshots", default=DEFAULT_SNAPSHOTS)
    t.set_defaults(fn=cmd_tasks)
    s = sub.add_parser("snapshot")
    s.add_argument("--label", required=True)
    s.add_argument("--at")
    s.add_argument("--snapshots", default=DEFAULT_SNAPSHOTS)
    s.set_defaults(fn=cmd_snapshot)
    b = sub.add_parser("backfill-git")
    b.add_argument("--repo", required=True)
    b.add_argument("--ref", default="origin/dev")
    b.add_argument("--snapshots", default=DEFAULT_SNAPSHOTS)
    b.set_defaults(fn=cmd_backfill)
    return p


p = build_parser()


def main(argv=None):
    args = p.parse_args(argv)
    args.fn(args)
    return 0


if __name__ == "__main__":
    sys.exit(main())
