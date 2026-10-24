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
  token_report.py estimate (--label L | --role thinker|builder|scout | --model SUBSTR) [--rollup FILE]
  token_report.py snapshot --label "..." [--at ISO8601] [--snapshots FILE]
  token_report.py backfill-git --repo PATH|- [--ref origin/dev] [--snapshots FILE]   (- = read
      `git log --first-parent --reverse --format=%cI|%s` from stdin)

`report` writes the project-wide roll-up (by stage, by label, by agent) a project keeps at
docs/product/token-usage.md. `tasks` does the same and additionally links each subagent's cost
to the Spec Kit task ids its description names, writing one specs/<feature>/tokens.md per
touched feature (a record file beside tasks.md, never edited into it) plus an "Unmapped"
section in the roll-up for cards that name no task id.

`estimate` reads that roll-up's "By agent" table for cards matching a label or a role/model
(the default role-to-model mapping is agent-roles.md's: thinker=opus, builder=sonnet,
scout=haiku; pass --model directly for a project override) and prints their average cost —
or says plainly that there's no history yet, never fabricating a number. It is a dispatch-time
aid for agents:orchestrate / agents:parallel's pacing decision, not an automated gate.

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
BILLABLE = ("input_tokens", "cache_creation_input_tokens", "output_tokens")
DEFAULT_SNAPSHOTS = "token-snapshots.json"
DEFAULT_ROLLUP = os.path.join("docs", "product", "token-usage.md")
DEFAULT_HISTORY = os.path.join("docs", "product", "token-history.jsonl")
DEFAULT_LEDGER = os.path.join(".ai-gent", "orchestrate-status.md")
TASK_ID = re.compile(r"\bT\d{3,}\b")
ROLE_MODEL = {"thinker": "opus", "builder": "sonnet", "scout": "haiku"}  # agent-roles.md's default mapping


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


def _text(content):
    if isinstance(content, str):
        return content
    if isinstance(content, list):
        return "\n".join(b.get("text", "") for b in content if isinstance(b, dict) and b.get("type") == "text")
    return ""


EDIT_TOOLS = ("Edit", "Write", "MultiEdit", "NotebookEdit")


def collect(path, msgs, extra=None):
    """msgs: id -> (utc timestamp, model, usage); keeps the most complete record per message id.
    extra (optional dict) also gets the agent's first user message as "prompt" and, as "edits",
    tool_use id -> (file, lines written) for its Edit/Write calls — a size signal for the history."""
    with open(path, encoding="utf-8") as fh:
        for line in fh:
            try:
                rec = json.loads(line)
            except json.JSONDecodeError:
                continue
            kind = rec.get("type")
            if extra is not None and kind == "user" and "prompt" not in extra:
                extra["prompt"] = _text((rec.get("message") or {}).get("content"))[:8000]
            if kind != "assistant":
                continue
            m = rec.get("message") or {}
            if extra is not None and isinstance(m.get("content"), list):
                for b in m["content"]:
                    if not (isinstance(b, dict) and b.get("type") == "tool_use" and b.get("name") in EDIT_TOOLS):
                        continue
                    inp = b.get("input") or {}
                    body = inp.get("content") or inp.get("new_string") or inp.get("new_source") or ""
                    if b.get("name") == "MultiEdit":
                        body = "\n".join(e.get("new_string", "") for e in inp.get("edits") or [])
                    extra.setdefault("edits", {})[b.get("id")] = (
                        inp.get("file_path") or inp.get("notebook_path") or "?", body.count("\n") + 1 if body else 0)
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


def family(model: str) -> str:
    return next((x for x in ("opus", "sonnet", "haiku") if x in (model or "").lower()), "other")


def project_name(slug: str) -> str:
    """The repository name behind a ~/.claude*/projects/ slug (the cwd with every non-alphanumeric
    character turned into '-'): the current directory's name when it encodes to the slug, else the
    part after a '-codes-', '-projects-', '-src-' or '-repos-' folder, else the last word."""
    cwd = os.getcwd()
    if re.sub(r"[^A-Za-z0-9-]", "-", cwd) == slug:
        return os.path.basename(cwd)
    for marker in ("-codes-", "-projects-", "-src-", "-repos-", "-work-"):
        if marker in slug:
            return slug.split(marker, 1)[1]
    return slug.rsplit("-", 1)[-1] or slug


def gather(roots, sessions):
    """Returns (coords, rows, tagged, main_msgs). coords/rows are per-agent summaries (a row also
    carries its aid, session, prompt, edits and raw messages); tagged is every (who, timestamp,
    model, usage) tuple for stage windows; main_msgs is session id -> {message id: message}.
    One row per agent id: a resumed agent keeps its id, so every resume lands in the same row."""
    main_msgs, agents, meta, extras, agent_sid = defaultdict(dict), defaultdict(dict), {}, defaultdict(dict), {}
    for root in roots:
        for sid in sessions:
            main = os.path.join(root, f"{sid}.jsonl")
            if os.path.exists(main):
                collect(main, main_msgs[sid])
            for f in sorted(glob.glob(os.path.join(root, sid, "subagents", "agent-*.jsonl"))):
                aid = os.path.basename(f)[len("agent-"):-len(".jsonl")]
                collect(f, agents[aid], extras[aid])
                agent_sid.setdefault(aid, sid)
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
        mlist = list(msgs.values())
        summ = summarize(mlist)
        fams = defaultdict(int)
        for _, mdl, _u in mlist:
            fams[family(mdl)] += 1
        edits = extras[aid].get("edits", {})
        rows.append({"who": f"subagent {aid[:7]}", "aid": aid, "sid": agent_sid[aid], "desc": desc,
                     "label": label_of(desc), "prompt": extras[aid].get("prompt", ""), "msgs": mlist,
                     "family": max(fams, key=fams.get), "files": sorted({p for p, _ in edits.values()}),
                     "lines": sum(n for _, n in edits.values()), **summ})
        tagged += [(desc, t, mdl, u) for t, mdl, u in mlist]
    rows.sort(key=lambda r: r["start"])
    coords = [{"who": f"coordinator {sid[:8]} (main thread)",
               "desc": "Planning, interviews, reconciliation, reviews, merges, CI follow-up",
               "label": "Coordination", **summarize(list(main_msgs[sid].values()))}
              for sid in sessions if main_msgs.get(sid)]
    return coords, rows, tagged, main_msgs


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
    coords, rows, tagged, _ = gather(roots, sessions)
    snaps = load_snapshots(a.snapshots)
    L, grand = render(project_name(a.project), sessions, roots, coords, rows, tagged, snaps)
    with open(a.out, "w", encoding="utf-8") as fh:
        fh.write("\n".join(L))
    print(f"wrote {a.out}: {len(rows)} subagents, {len(snaps)} stage markers, "
          f"billable {k(grand['billable'])}, total {k(grand['total'])}")


# ---------------------------------------------------------------- specs

TASK_LINE = re.compile(r"^(\s*- \[[ xX]\] )(T\d{3,})\b")
TOKENS_TAG = re.compile(r"\s*·\s*Tokens:[^\n]*$")
CONFIDENCE_ORDER = {"agent": 0, "feature": 1, "exact": 2}
LAYER_HEADS = (("backend", "backend"), ("frontend", "frontend"), ("qa", "qa"), ("quality", "qa"))
EFFORT_RANK = {"low": 0, "medium": 1, "high": 2}


def parse_tok(s):
    """'~420k' -> 420000; '1.2M' -> 1200000; '391,000' -> 391000; '—' or '' -> None."""
    s = (s or "").strip().strip("`*~ ").replace(",", "").replace("_", "")
    m = re.fullmatch(r"(\d+(?:\.\d+)?)\s*([kKmM]?)", s)
    if not m:
        return None
    return int(round(float(m.group(1)) * {"": 1, "k": 1_000, "m": 1_000_000}[m.group(2).lower()]))


def fmt_tok(n):
    """The compact form task lines and cards carry: 391k, 1.24M."""
    if n is None:
        return "—"
    return f"{round(n / 1000)}k" if n < 1_000_000 else f"{n / 1_000_000:.2f}M"


def expand_ids(text):
    """Task ids named in text, ranges ('T006–T009') expanded, in order, without duplicates."""
    out = []
    spans = []
    for m in re.finditer(r"\bT(\d{3,})\s*(?:–|—|-|\.\.|\bto\b)\s*T?(\d{3,})\b", text):
        lo, hi = int(m.group(1)), int(m.group(2))
        width = len(m.group(1))
        if lo <= hi <= lo + 200:
            out += [f"T{i:0{width}d}" for i in range(lo, hi + 1)]
            spans.append(m.span())
    for m in TASK_ID.finditer(text):
        if not any(a <= m.start() < b for a, b in spans):
            out.append(m.group(0))
    seen = set()
    return [t for t in sorted(set(out), key=out.index) if not (t in seen or seen.add(t))]


def parse_tasks_md(path):
    """task id -> {line, layer, effort, executor} for a feature's tasks.md (first definition wins)."""
    tasks = {}
    layer = None
    with open(path, encoding="utf-8") as fh:
        for n, line in enumerate(fh):
            if line.startswith("#"):
                head = line.lstrip("# ").strip().lower()
                layer = next((v for key, v in LAYER_HEADS if head.startswith(key)), layer if line.startswith("###") else None)
                continue
            m = TASK_LINE.match(line)
            if not m or m.group(2) in tasks:
                continue
            ex = re.search(r"Executor:\s*(\w+)", line)
            ef = re.search(r"Effort:\s*(\w+)", line)
            tag = re.search(r"Tokens:\s*projected\s*([^|·\n]*)", line)
            tasks[m.group(2)] = {"line": n, "layer": layer, "executor": ex and ex.group(1), "effort": ef and ef.group(1),
                                 "projected": parse_tok(tag.group(1)) if tag else None}
    return tasks


def parse_cards_md(path):
    """card id -> {tasks, executor, reviewer, effort, projected, dispatch} from a feature's cards.md:
    the summary table gives the route, each '## Card `id`' section its Tokens line and dispatch description."""
    cards = {}
    with open(path, encoding="utf-8") as fh:
        text = fh.read()
    cols = None
    for line in text.splitlines():
        if not line.startswith("|"):
            cols = None
            continue
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        if cells and cells[0].lower() == "card":
            cols = [c.lower() for c in cells]
            continue
        if cols is None or set(line.replace("|", "").strip()) <= set("-: "):
            continue
        row = dict(zip(cols, cells))
        cid = row.get("card", "").strip("`")
        if cid:
            cards[cid] = {"tasks": expand_ids(row.get("tasks", "")), "executor": row.get("executor", "").split()[0] if row.get("executor") else None,
                          "reviewer": row.get("reviewer"), "effort": (row.get("effort") or "").split()[0] or None,
                          "projected": None, "dispatch": None}
    for m in re.finditer(r"^## Card `([^`]+)`[^\n]*\n(.*?)(?=^## |\Z)", text, re.S | re.M):
        c = cards.setdefault(m.group(1), {"tasks": expand_ids(m.group(0).splitlines()[0]), "executor": None,
                                          "reviewer": None, "effort": None, "projected": None, "dispatch": None})
        tk = re.search(r"Tokens:\s*projected\s*([^|·`\n]*)", m.group(2))
        c["projected"] = parse_tok(tk.group(1)) if tk else None
        dd = re.search(r"Dispatch description:\s*`([^`]+)`", m.group(2))
        c["dispatch"] = dd.group(1) if dd else None
        if not c["tasks"]:
            c["tasks"] = expand_ids(m.group(0).splitlines()[0])
    return cards


def load_features(specs_dir):
    """feature dir name -> {name, num, dir, tasks, cards}; only features with a tasks.md or cards.md."""
    feats = {}
    for d in sorted(glob.glob(os.path.join(specs_dir, "[0-9][0-9][0-9]-*"))):
        tasks_md, cards_md = os.path.join(d, "tasks.md"), os.path.join(d, "cards.md")
        tasks = parse_tasks_md(tasks_md) if os.path.exists(tasks_md) else {}
        cards = parse_cards_md(cards_md) if os.path.exists(cards_md) else {}
        if tasks or cards:
            name = os.path.basename(d)
            feats[name] = {"name": name, "num": name[:3], "dir": d, "tasks": tasks, "cards": cards}
    return feats


# ------------------------------------------------------------ attribution

def norm(s):
    return re.sub(r"\s+", " ", (s or "").strip().lower())


def feature_mentions(text, by_num):
    return [n for n in dict.fromkeys(re.findall(r"(?<![\w.])(\d{3})(?![\w.])", text)) if n in by_num]


def prompt_feature(prompt, by_num):
    counts = defaultdict(int)
    for n in re.findall(r"specs/(\d{3})-", prompt) + re.findall(r"(?<![\w.])(\d{3})-[a-z]", prompt):
        if n in by_num:
            counts[n] += 1
    top = sorted(counts.items(), key=lambda x: -x[1])
    return top[0][0] if top and (len(top) == 1 or top[0][1] > top[1][1]) else None


def attribute(row, feats):
    """Which feature, card and task ids an agent's usage belongs to, and how sure that is.
    exact   - its dispatch description or card id matches a card, or it names task ids of one feature;
    feature - it names only a feature;
    agent   - nothing in its description or prompt ties it to the specs (never guessed)."""
    res = {"feature": None, "card": None, "tasks": [], "confidence": "agent", "how": ""}
    if not feats:
        return res
    desc, prompt = row["desc"], row.get("prompt", "")
    by_num = {f["num"]: f for f in feats.values()}

    def hit(f, cid, how):
        c = f["cards"].get(cid, {})
        named = [t for t in expand_ids(desc) if t in c.get("tasks", [])]
        res.update(feature=f["name"], card=cid, tasks=named or c.get("tasks", []), confidence="exact", how=how)
        return res

    for f in feats.values():
        for cid, c in f["cards"].items():
            if c["dispatch"] and norm(c["dispatch"]) == norm(desc):
                return hit(f, cid, "dispatch description")
    ids_by_len = sorted(((cid, f) for f in feats.values() for cid in f["cards"]), key=lambda x: -len(x[0]))
    for cid, f in ids_by_len:
        if re.search(rf"(?<![\w-]){re.escape(cid)}(?![\w-])", desc):
            return hit(f, cid, "card id in description")

    ids = expand_ids(desc)
    if not ids:
        goal = next((ln for ln in prompt.splitlines() if ln.lstrip().lower().startswith("goal")), "")
        ids = expand_ids(goal)
    mention = feature_mentions(desc, by_num)
    num = mention[0] if mention else None
    how = "feature named in description"
    if num is None and ids:
        owners = [f for f in feats.values() if all(t in f["tasks"] for t in ids)]
        if len(owners) == 1:
            num, how = owners[0]["num"], "task ids unique to one feature"
    if num is None:
        num, how = prompt_feature(prompt, by_num), "feature named in prompt"
    if num is None:
        return res
    f = by_num[num]
    known = [t for t in ids if t in f["tasks"]]
    res.update(feature=f["name"], confidence="feature", how=how)
    if known:
        res.update(tasks=known, confidence="exact", how=how + " + task ids")
        inside = [cid for cid, c in f["cards"].items() if set(known) <= set(c["tasks"])]
        if len(inside) == 1:
            res["card"] = inside[0]
    return res


def tasks_key(ids):
    return ids[0] if len(ids) == 1 else f"{ids[0]}..{ids[-1]}"


def build_groups(rows, feats):
    """Group agents that worked on the same card (or task set); other agents stay one group each."""
    groups = {}
    for r in rows:
        a = attribute(r, feats)
        r["attr"] = a
        if a["confidence"] == "agent" or not a["feature"]:
            key = ("agent", r["aid"])
        elif a["card"]:
            key = (a["feature"], "card", a["card"])
        elif a["tasks"]:
            key = (a["feature"], "tasks", tasks_key(a["tasks"]))
        else:
            key = (a["feature"], "agent", r["aid"])
        g = groups.setdefault(key, {"key": key, "feature": a["feature"], "card": a["card"], "tasks": [],
                                    "confidence": a["confidence"], "rows": []})
        g["rows"].append(r)
        g["tasks"] = sorted(set(g["tasks"]) | set(a["tasks"]))
        if CONFIDENCE_ORDER[a["confidence"]] > CONFIDENCE_ORDER[g["confidence"]]:
            g["confidence"] = a["confidence"]
    for g in groups.values():
        rs = g["rows"]
        g["billable"] = sum(r["billable"] for r in rs)
        g["cache_read"] = sum(r["cache_read_input_tokens"] for r in rs)
        g["calls"] = sum(r["calls"] for r in rs)
        g["end"] = max(r["end"] for r in rs)
        g["start"] = min(r["start"] for r in rs)
        g["files"] = sorted({p for r in rs for p in r["files"]})
        g["lines"] = sum(r["lines"] for r in rs)
        main = [r for r in rs if not re.match(r"\s*review", r["desc"], re.I)] or rs
        lead = max(main, key=lambda r: r["billable"])
        g["model"] = lead["family"]
        g["desc"] = lead["desc"]
    return sorted(groups.values(), key=lambda g: g["start"])


def layer_of(g, feats):
    f = feats.get(g["feature"]) if g["feature"] else None
    if f:
        for t in g["tasks"]:
            if f["tasks"].get(t, {}).get("layer"):
                return f["tasks"][t]["layer"]
    probe = (g["card"] or "") + " " + g["desc"].lower()
    for word, layer in (("frontend", "frontend"), ("qa", "qa"), ("e2e", "qa"), ("-core", "backend"), ("backend", "backend")):
        if word in probe:
            return layer
    return "general"


def effort_of(g, feats):
    f = feats.get(g["feature"]) if g["feature"] else None
    if not f:
        return None
    c = f["cards"].get(g["card"]) if g["card"] else None
    if c and c["effort"] in EFFORT_RANK:
        return c["effort"]
    efforts = [f["tasks"][t]["effort"] for t in g["tasks"] if t in f["tasks"] and f["tasks"][t]["effort"] in EFFORT_RANK]
    return max(efforts, key=EFFORT_RANK.get) if efforts else None


ROLE_OF_FAMILY = {"opus": "thinker", "sonnet": "builder", "haiku": "scout"}


def projected_of(g, feats):
    f = feats.get(g["feature"]) if g["feature"] else None
    if not f:
        return None
    c = f["cards"].get(g["card"]) if g["card"] else None
    if c and c["projected"] is not None:
        return c["projected"]
    first = sorted((t for t in g["tasks"] if t in f["tasks"]), key=lambda t: f["tasks"][t]["line"])
    return f["tasks"][first[0]]["projected"] if first else None


# ------------------------------------------------------ windows and split

def git_markers(ref=None):
    """Stage markers from the current repo's first-parent landings, in memory (nothing written)."""
    for r in ([ref] if ref else ["origin/dev", "dev", "main", "HEAD"]):
        try:
            out = subprocess.run(["git", "log", "--first-parent", "--reverse", "--format=%cI|%s", r], check=True,
                                 capture_output=True, text=True).stdout
        except (OSError, subprocess.CalledProcessError):
            continue
        return [{"at": to_utc(t), "label": label_from_subject(s), "source": "git"}
                for t, s in (ln.split("|", 1) for ln in out.splitlines() if "|" in ln)]
    return []


def windows(snaps):
    bounds = [(snaps[i - 1]["at"] if i else "", s["at"]) for i, s in enumerate(snaps)]
    if snaps:
        bounds.append((snaps[-1]["at"], "9999"))
    return bounds or [("", "9999")]


def split_coordinator(main_msgs, groups, snaps):
    """Each window's coordinator billable, shared among the features whose subagents ran in it
    (weighted by their billable in that window). Returns (feature -> share, unassigned, total)."""
    coord = [(t, bill(u)) for msgs in main_msgs.values() for t, _, u in msgs.values() if t]
    share, unassigned = defaultdict(float), 0
    for lo, hi in windows(snaps):
        c = sum(b for t, b in coord if lo < t <= hi)
        w = defaultdict(int)
        for g in groups:
            if g["feature"]:
                for r in g["rows"]:
                    w[g["feature"]] += sum(bill(u) for t, _, u in r["msgs"] if t and lo < t <= hi)
        tot = sum(w.values())
        if not tot:
            unassigned += c
            continue
        for f, v in w.items():
            share[f] += c * v / tot
    return dict(share), unassigned, sum(b for _, b in coord)


def bill(u):
    return sum(int(u.get(f) or 0) for f in BILLABLE)


# ---------------------------------------------------------------- writers

def make_record(project, g, feats, coord_card, source="measured"):
    f = feats.get(g["feature"]) if g["feature"] else None
    c = f["cards"].get(g["card"]) if f and g["card"] else None
    model = g["model"]
    key = g["card"] or (tasks_key(g["tasks"]) if g["tasks"] else g["rows"][0]["aid"])
    ident = f"{project}:{g['feature']}:{key}" if g["feature"] and g["confidence"] != "agent" and (g["card"] or g["tasks"]) \
        else f"{project}:agent:{g['rows'][0]['aid']}"
    if source == "seed":
        ident = "seed:" + ident
    return {"id": ident, "source": source, "confidence": g["confidence"], "project": project, "feature": g["feature"],
            "card": g["card"], "tasks": g["tasks"], "layer": layer_of(g, feats), "effort": effort_of(g, feats),
            "executor_role": (c and c["executor"]) or ROLE_OF_FAMILY.get(model, "builder"), "executor_model": model,
            "files": len(g["files"]), "lines": g["lines"], "projected": projected_of(g, feats), "used": g["billable"],
            "cache_read": g["cache_read"], "coordinator": round(coord_card) if coord_card is not None else None,
            "calls": g["calls"], "agents": sorted(r["aid"] for r in g["rows"]), "desc": g["desc"][:120],
            "date": g["end"][:10]}


def read_history(path):
    out = []
    if path and os.path.exists(path):
        with open(path, encoding="utf-8") as fh:
            for line in fh:
                line = line.strip()
                if line:
                    try:
                        out.append(json.loads(line))
                    except json.JSONDecodeError:
                        continue
    return out


def upsert_history(path, records):
    """Add or replace records by id; the file stays sorted, so re-running never duplicates a line."""
    merged = {r["id"]: r for r in read_history(path)}
    merged.update({r["id"]: r for r in records})
    os.makedirs(os.path.dirname(os.path.abspath(path)), exist_ok=True)
    with open(path, "w", encoding="utf-8") as fh:
        for r in sorted(merged.values(), key=lambda r: (r.get("date", ""), r["id"])):
            fh.write(json.dumps(r, ensure_ascii=False) + "\n")
    return len(merged)


def tag_for(projected, used, with_task=None):
    if with_task:
        return f" · Tokens: with {with_task}"
    return f" · Tokens: projected {'~' + fmt_tok(projected) if projected is not None else '—'} | used {fmt_tok(used)}"


def write_task_lines(feature, units, dry):
    """Put each unit's tag on its first task line (file order) and 'with Txxx' on the others.
    A unit is {tasks, projected, used}; the tag is replaced, never added twice."""
    path = os.path.join(feature["dir"], "tasks.md")
    if not os.path.exists(path):
        return 0
    with open(path, encoding="utf-8") as fh:
        lines = fh.read().split("\n")
    changed = 0
    for u in units:
        mine = sorted((t for t in u["tasks"] if t in feature["tasks"]), key=lambda t: feature["tasks"][t]["line"])
        if not mine or (u["projected"] is None and u["used"] is None):
            continue
        for i, t in enumerate(mine):
            n = feature["tasks"][t]["line"]
            new = TOKENS_TAG.sub("", lines[n]).rstrip() + (tag_for(u["projected"], u["used"]) if i == 0 else tag_for(0, 0, mine[0]))
            if new != lines[n]:
                lines[n], changed = new, changed + 1
    if changed and not dry:
        with open(path, "w", encoding="utf-8") as fh:
            fh.write("\n".join(lines))
    return changed


TRACKER_START = "<!-- tokens:start (token_report.py tasks - regenerated on every run, do not edit) -->"
TRACKER_END = "<!-- tokens:end -->"


def pct_error(projected, used):
    return f"{(used - projected) / projected * 100:+.0f}%" if projected and used is not None else "—"


def write_tracker(feature, units, extras, coord, dry):
    """The feature's token block in tracker.md (created if missing), replaced in place on re-runs."""
    rows = ["| Card / tasks | Projected | Used | Error | Cache reads (reference) |", "|---|---|---|---|---|"]
    p_tot = u_tot = c_tot = 0
    for u in units:
        if u["used"] is None:
            continue
        rows.append(f"| {u['label']} | {'~' + fmt_tok(u['projected']) if u['projected'] is not None else '—'} | "
                    f"{fmt_tok(u['used'])} | {pct_error(u['projected'], u['used'])} | {fmt_tok(u['cache_read'])} |")
        u_tot += u["used"]
        c_tot += u["cache_read"]
        if u["projected"] is not None:
            p_tot += u["projected"]
    for e in extras:
        rows.append(f"| {e['label']} | — | {fmt_tok(e['used'])} | — | {fmt_tok(e['cache_read'])} |")
        u_tot += e["used"]
        c_tot += e["cache_read"]
    rows.append(f"| **Subagents** | ~{fmt_tok(p_tot)} | **{fmt_tok(u_tot)}** | {pct_error(p_tot, u_tot)} | {fmt_tok(c_tot)} |")
    rows.append(f"| Coordinator share | | {fmt_tok(round(coord))} | | |")
    rows.append(f"| **Feature total** | | **{fmt_tok(u_tot + round(coord))}** | | |")
    block = "\n".join([TRACKER_START, "", "## Tokens", "",
                       "Billable tokens (uncached input + cache writes + output); cache reads are reference only. "
                       "Projected is the card's estimate before the work, used what its subagents then cost. "
                       "The coordinator share is the main thread's usage in the windows this feature landed, "
                       "split by subagent share (`plugins/agents/references/token-tracking.md`).", ""] + rows + ["", TRACKER_END])
    path = os.path.join(feature["dir"], "tracker.md")
    text = f"# Tracker — {feature['name']}\n"
    if os.path.exists(path):
        with open(path, encoding="utf-8") as fh:
            text = fh.read()
    pat = re.compile(re.escape(TRACKER_START) + r".*?" + re.escape(TRACKER_END), re.S)
    new = pat.sub(lambda _m: block, text) if pat.search(text) else text.rstrip("\n") + "\n\n" + block + "\n"
    if new != text and not dry:
        with open(path, "w", encoding="utf-8") as fh:
            fh.write(new)
    return new != text


def ledger_cell(u):
    if not u or (u["projected"] is None and u["used"] is None):
        return None
    p = f"~{fmt_tok(u['projected'])}" if u["projected"] is not None else "—"
    return f"{p} / {fmt_tok(u['used'])}"


def write_ledger(path, by_card, dry):
    """Add or refresh a Tokens column (projected / used) on the orchestration ledger's card table."""
    if not os.path.exists(path):
        return False
    with open(path, encoding="utf-8") as fh:
        lines = fh.read().split("\n")
    out, state, idx = [], None, 0
    for ln in lines:
        if not ln.startswith("|"):
            state = None
            out.append(ln)
            continue
        cells = [c.strip() for c in ln.strip().strip("|").split("|")]
        if state is None:
            if any(c.lower() == "card" for c in cells):
                if "Tokens" not in cells:
                    cells.append("Tokens")
                idx, state = cells.index("Tokens"), "sep"
                ln = "| " + " | ".join(cells) + " |"
            out.append(ln)
            continue
        if state == "sep":
            out.append("|" + "|".join(["---"] * max(len(cells), idx + 1)) + "|")
            state = "body"
            continue
        cid = next((c.strip("`") for c in cells if c.startswith("`") and c.endswith("`")), None)
        cells += [""] * (idx + 1 - len(cells))
        cells[idx] = ledger_cell(by_card.get(cid)) or cells[idx] or "—"
        out.append("| " + " | ".join(cells) + " |")
    new = "\n".join(out)
    if new != "\n".join(lines) and not dry:
        with open(path, "w", encoding="utf-8") as fh:
            fh.write(new)
    return new != "\n".join(lines)


def cmd_tasks(a):
    roots = sorted(glob.glob(os.path.expanduser(f"~/.claude*/projects/{a.project}")))
    sessions = a.session or discover_sessions(roots)
    coords, rows, tagged, main_msgs = gather(roots, sessions)
    snaps = load_snapshots(a.snapshots) or git_markers()
    project = a.name or project_name(a.project)
    feats = load_features(a.specs_dir)
    groups = build_groups(rows, feats)
    share, unassigned, coord_total = split_coordinator(main_msgs, groups, snaps)

    if a.rollup_out:
        L, grand = render(project, sessions, roots, coords, rows, tagged, snaps)
        if not a.dry_run:
            with open(a.rollup_out, "w", encoding="utf-8") as fh:
                fh.write("\n".join(L))

    feat_bill = defaultdict(int)
    for g in groups:
        if g["feature"]:
            feat_bill[g["feature"]] += g["billable"]
    records, by_card, summary = [], {}, {}
    for fname, feat in feats.items():
        mine = [g for g in groups if g["feature"] == fname]
        units, extras = [], []
        for cid, c in feat["cards"].items():
            if c["projected"] is not None and not any(g["card"] == cid for g in mine):
                units.append({"label": f"`{cid}`", "tasks": c["tasks"], "projected": c["projected"], "used": None, "cache_read": 0})
        for g in mine:
            cshare = share.get(fname, 0) * g["billable"] / feat_bill[fname] if feat_bill[fname] else 0
            rec = make_record(project, g, feats, cshare)
            records.append(rec)
            u = {"label": f"`{g['card']}`" if g["card"] else (f"{tasks_key(g['tasks'])}" if g["tasks"] else None),
                 "tasks": g["tasks"], "projected": rec["projected"], "used": g["billable"], "cache_read": g["cache_read"],
                 "desc": g["desc"]}
            if u["label"] is None:
                extras.append({"label": f"{g['desc'][:50]} (feature-level)", "used": g["billable"], "cache_read": g["cache_read"]})
                continue
            units.append(u)
            if g["card"]:
                by_card[g["card"]] = u
        for u in units:
            if u["used"] is None and u["tasks"]:
                by_card[u["label"].strip("`")] = u
        used_units = [u for u in units if u["used"] is not None]
        changed = write_task_lines(feat, units, a.dry_run)
        if used_units or extras:
            write_tracker(feat, units, extras, share.get(fname, 0), a.dry_run)
        summary[fname] = {"used": feat_bill[fname], "coordinator": share.get(fname, 0), "cards": len(used_units),
                          "extras": len(extras), "lines": changed}
    unattributed = [g for g in groups if g["confidence"] == "agent" or not g["feature"]]
    records += [make_record(project, g, feats, None) for g in unattributed]
    if not a.dry_run:
        n = upsert_history(a.history, records)
        write_ledger(a.ledger, by_card, False)
    else:
        n = len(records)

    print(f"{project}: {len(rows)} subagent(s) in {len(sessions)} session(s); "
          f"{'dry run, nothing written' if a.dry_run else 'wrote ' + a.history + f' ({n} records)'}")
    print("feature | used (subagents) | coordinator share | total | cards | task lines")
    for fname, s in sorted(summary.items()):
        if s["used"] or s["cards"]:
            print(f"{fname} | {fmt_tok(s['used'])} | {fmt_tok(round(s['coordinator']))} | "
                  f"{fmt_tok(s['used'] + round(s['coordinator']))} | {s['cards']} | {s['lines']}")
    un = sum(g["billable"] for g in unattributed)
    print(f"unattributed (reported, not guessed): {len(unattributed)} agent(s), {fmt_tok(un)}")
    for g in unattributed:
        print(f"  {fmt_tok(g['billable']):>7}  {g['model']:<6} {g['desc'][:80]}")
    print(f"coordinator: {fmt_tok(round(coord_total))} billable in all, {fmt_tok(round(unassigned))} in windows "
          f"with no attributed subagents")
    agent_total = sum(r["billable"] for r in rows)
    print(f"total: {fmt_tok(round(agent_total + coord_total))} billable ({fmt_tok(agent_total)} subagents)")


def parse_k(s: str) -> float:
    s = s.strip().replace(",", "").replace("*", "")
    if s.endswith("M"):
        return float(s[:-1]) * 1_000_000
    if s.endswith("k"):
        return float(s[:-1]) * 1_000
    return float(s) if s else 0.0


def read_by_agent(rollup_path):
    """Rows of the roll-up's '## By agent' table — the one place every card's own cost lands,
    one row each. Parses our own generated table back; the markdown IS the record, there is
    no separate structured ledger."""
    if not os.path.exists(rollup_path):
        return []
    with open(rollup_path, encoding="utf-8") as fh:
        text = fh.read()
    m = re.search(r"## By agent\n\n(.*?)(?:\n## |\Z)", text, re.S)
    if not m:
        return []
    lines = [ln for ln in m.group(1).splitlines() if ln.startswith("|")]
    rows = []
    for ln in lines[2:]:  # skip the header and the --- separator
        cells = [c.strip() for c in ln.strip("|").split("|")]
        if len(cells) < 8:
            continue
        started, who, label, task, model, calls, billable, total = cells[:8]
        rows.append({"started": started, "who": who, "label": label, "task": task, "model": model,
                     "calls": calls, "billable": parse_k(billable), "total": parse_k(total)})
    return rows


def cmd_estimate(a):
    rows = read_by_agent(a.rollup)
    if a.label:
        sel = [r for r in rows if r["label"] == a.label]
        how = f"label '{a.label}'"
    else:
        needle = ROLE_MODEL[a.role] if a.role else a.model
        sel = [r for r in rows if needle in r["model"]]
        how = f"role '{a.role}' ({needle})" if a.role else f"model '{a.model}'"
    if not sel:
        print(f"no history for {how} in {a.rollup} — fall back to S/M/L sizing")
        return
    avg_bill = sum(r["billable"] for r in sel) / len(sel)
    avg_tot = sum(r["total"] for r in sel) / len(sel)
    print(f"{how}: {len(sel)} sample(s) in {a.rollup}, average billable {k(int(avg_bill))}, "
          f"average total {k(int(avg_tot))}")


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
    t.add_argument("--rollup-out", help="also write the project-wide report here (docs/product/token-usage.md)")
    t.add_argument("--history", default=DEFAULT_HISTORY)
    t.add_argument("--ledger", default=DEFAULT_LEDGER)
    t.add_argument("--name", help="project name for the history (default: derived from --project)")
    t.add_argument("--snapshots", default=DEFAULT_SNAPSHOTS,
                   help="stage markers; without the file they are read from the repo's first-parent git landings")
    t.add_argument("--dry-run", action="store_true", help="read everything, write nothing, print the attribution")
    t.set_defaults(fn=cmd_tasks)
    e = sub.add_parser("estimate")
    g = e.add_mutually_exclusive_group(required=True)
    g.add_argument("--label", help="exact match on the roll-up's Label column (a card's own task ids)")
    g.add_argument("--role", choices=sorted(ROLE_MODEL), help="agent-roles.md's default model for the role")
    g.add_argument("--model", help="a model-name substring, for a project that overrode the default mapping")
    e.add_argument("--rollup", default=DEFAULT_ROLLUP)
    e.set_defaults(fn=cmd_estimate)
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
