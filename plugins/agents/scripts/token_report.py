#!/usr/bin/env python3
"""Token tracking for a project's Claude Code sessions: what each card cost, what the next one
should, and whether the budget covers the next wave. The tool behind agents:orchestrate's and
agents:parallel's token economy (../references/token-tracking.md explains the method).

Counts billable tokens (uncached input + cache writes + output); cache reads are reported for
reference only. Reads the session JSONL transcripts in every Claude config dir
(~/.claude*/projects/<slug>/), deduplicates assistant messages by message id (streamed chunks and
account switches repeat them; local `<synthetic>` placeholders are skipped), and sums each
message's final usage - the API's own numbers, never an estimate. Python standard library only.

  token_report.py tasks    --project=SLUG [--specs-dir specs] [--dry-run]
      attribute each subagent to its card and task ids; write `used` on tasks.md task lines
      (`· Tokens: projected ~420k | used 391k`), a Tokens block in specs/NNN-*/tracker.md, a Tokens
      column in .ai-gent/orchestrate-status.md and records in docs/product/token-history.jsonl.
      Re-running updates the numbers; it never duplicates them.
  token_report.py estimate [--role builder|--model sonnet] [--effort E] [--layer L] [--tasks N]
      the projection for a planned card: median and p75 of similar history, else the starting table
  token_report.py budget   --cards ID,ID | --projected [model:]400k,...
      proceed, shrink or stop against .ai-gent/budget.yaml, less what the session already used
  token_report.py index    [--repo DIR]... [--scan ~/codes]
      rebuild ~/.ai-gent/token-history.jsonl from the repos' committed histories
  token_report.py seed     [--project SLUG]...
      seed that index from past sessions (read-only on sessions and repos), with a confidence each
  token_report.py report   --project=SLUG --out PATH     the project-wide roll-up (by stage, label, agent)
  token_report.py snapshot --label "..." [--at ISO8601]  a stage marker by hand
  token_report.py backfill-git --repo PATH|- [--ref origin/dev]   stage markers from first-parent landings

Stage markers split the coordinator's (main thread's) usage by landing window; without a snapshots
file `tasks` reads them from the repo's git history in memory. A project slug starts with '-', so
give it as --project=<slug>.

Claude Code-specific: this reads Claude Code's own on-disk transcript format and the Agent tool's
subagent .meta.json sidecars. No equivalent exists on another agent.
"""
import argparse
import glob
import json
import math
import os
import re
import subprocess
import sys
from collections import defaultdict
from datetime import datetime, timedelta, timezone

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
        for d in glob.glob(os.path.join(root, "*", "subagents")):  # a session whose main transcript is gone
            sids.add(os.path.basename(os.path.dirname(d)))
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
            if extra is not None and m.get("model") == "<synthetic>" and rec.get("timestamp"):
                extra.setdefault("stops", []).append(to_utc(rec["timestamp"]))  # a limit or error stop
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


def resume_events(stops, mlist):
    """(resumes, their cost). A stop (a `<synthetic>` limit message) followed by more work is a resume;
    its cost is the first message after it, which re-reads the agent's context."""
    later = sorted((t, bill(u)) for t, _, u in mlist if t)
    n = cost = 0
    for stop in sorted(set(stops)):
        nxt = next((b for t, b in later if t > stop), None)
        if nxt is not None:
            n, cost = n + 1, cost + nxt
    return n, cost


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
        resumed, resume_cost = resume_events(extras[aid].get("stops", []), mlist)
        rows.append({"resumed": resumed, "resume_cost": resume_cost, "who": f"subagent {aid[:7]}", "aid": aid, "sid": agent_sid[aid], "desc": desc,
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


# ----------------------------------------------------------------- tags

_VOCAB = None


def vocab():
    """facet -> {required, multi, values, aliases?}: shared/token-tags.json via the plugin's references/."""
    global _VOCAB
    if _VOCAB is None:
        here = os.path.dirname(os.path.abspath(__file__))
        _VOCAB = {}
        for path in (os.path.join(here, "..", "references", "token-tags.json"),
                     os.path.join(here, "..", "..", "..", "shared", "token-tags.json")):
            if os.path.exists(path):
                with open(path, encoding="utf-8") as fh:
                    _VOCAB = json.load(fh)["facets"]
                break
    return _VOCAB


def norm_stack(name):
    """A package or crate name as the vocabulary's stack value, else None."""
    st = vocab().get("stack", {})
    n = name.strip().lower()
    for cand in (n, n.replace("_", "-")):
        if cand in st.get("values", {}):
            return cand
        if cand in st.get("aliases", {}):
            return st["aliases"][cand]
    return None


def parse_tags(text):
    """'extend, backend, rust, tokio, M' -> {'kind': 'extend', ..., 'stack': ['tokio']}; unknown words are ignored."""
    tags = {}
    for word in [w.strip() for w in (text or "").split(",") if w.strip()]:
        for facet, d in vocab().items():
            value = word if word in d["values"] else (norm_stack(word) if facet == "stack" else None)
            if value is None:
                continue
            if d["multi"]:
                tags[facet] = sorted(set(tags.get(facet, [])) | {value})
            else:
                tags.setdefault(facet, value)
            break
    return tags


def format_tags(tags):
    out = []
    for facet in vocab():
        v = tags.get(facet)
        out += v if isinstance(v, list) else ([v] if v else [])
    return ", ".join(out)


EXT_LANG = {".rs": "rust", ".ts": "typescript", ".tsx": "typescript", ".js": "javascript", ".jsx": "javascript",
            ".mjs": "javascript", ".cjs": "javascript", ".py": "python", ".go": "go", ".java": "java", ".kt": "kotlin",
            ".cs": "csharp", ".php": "php", ".rb": "ruby", ".sh": "sh", ".sql": "sql", ".c": "cpp", ".h": "cpp",
            ".cc": "cpp", ".cpp": "cpp", ".hpp": "cpp"}
MANIFESTS = ("Cargo.toml", "package.json", "pyproject.toml", "go.mod", "pom.xml", "build.gradle")


def repo_info(repo_dir):
    """The repository's languages and stack, read from its manifests (Cargo.toml, package.json,
    pyproject.toml, go.mod, pom.xml/build.gradle), at the root and one folder down. Read-only."""
    langs, stack = [], set()
    if not repo_dir or not os.path.isdir(repo_dir):
        return {"lang": None, "langs": [], "stack": []}
    paths = [os.path.join(repo_dir, m) for m in MANIFESTS]
    for sub in sorted(glob.glob(os.path.join(repo_dir, "*"))):
        if os.path.isdir(sub) and os.path.basename(sub) not in ("node_modules", "target", "dist", ".git", ".worktrees"):
            paths += [os.path.join(sub, m) for m in MANIFESTS]
    for path in paths:
        if not os.path.isfile(path):
            continue
        try:
            with open(path, encoding="utf-8", errors="replace") as fh:
                text = fh.read()
        except OSError:
            continue
        base, names, lang = os.path.basename(path), [], None
        if base == "Cargo.toml":
            lang, section = "rust", ""
            for line in text.splitlines():
                m = re.match(r"\s*\[(.+?)\]", line)
                if m:
                    section = m.group(1)
                    sub_m = re.match(r"(?:\w+-)?dependencies\.(.+)", section)
                    if sub_m:
                        names.append(sub_m.group(1))
                elif "dependencies" in section and (m := re.match(r"\s*([A-Za-z0-9_\-]+)\s*=", line)):
                    names.append(m.group(1))
        elif base == "package.json":
            try:
                pkg = json.loads(text)
            except json.JSONDecodeError:
                pkg = {}
            names = [*pkg.get("dependencies", {}), *pkg.get("devDependencies", {})]
            lang = "typescript" if "typescript" in names or os.path.exists(os.path.join(os.path.dirname(path), "tsconfig.json")) else "javascript"
        elif base == "pyproject.toml":
            lang = "python"
            names = re.findall(r"[\"']([A-Za-z][A-Za-z0-9_.\-]*)\s*(?:[<>=!~\[;]|[\"'])", text)
        elif base == "go.mod":
            lang = "go"
            names = [m.group(1) for m in re.finditer(r"^\s*(?:require\s+)?([\w.\-]+/[\w./\-]+)\s+v", text, re.M)]
        else:
            lang = "java"
            names = ["spring-boot"] if "spring-boot" in text else []
        if lang and lang not in langs:
            langs.append(lang)
        stack.update(v for v in (norm_stack(n) for n in names) if v)
    return {"lang": langs[0] if langs else None, "langs": langs, "stack": sorted(stack)}


KIND_WORDS = (("fix", r"\b(fix|bug|repair|findings?|regression)\b"), ("refactor", r"\b(refactor|rename|restructure|cleanup)\b"),
              ("test-only", r"\b(tests?|e2e|charters?|journeys?|perf suites?|load)\b"), ("docs", r"\b(docs?|documentation|readme|discovery)\b"),
              ("ci", r"\b(ci|pipeline|release|workflow|dependabot)\b"),
              ("new-module", r"\b(foundation|skeleton|scaffold|bootstrap|setup|new)\b"))
SURFACE_WORDS = (("protocol", r"\b(protocol|ipc|socket|wire|openapi|contract|daemon)\b"), ("storage", r"\b(storage|store|database|sqlite|migration|schema)\b"),
                 ("ui-snapshots", r"\bsnapshots?\b"), ("session-display", r"\b(picker|tui|terminal|display|tray|render|window|capture|theme|colou?r)\b"),
                 ("workflow", r"\b(ci|pipeline|release|workflow|orchestrat\w*)\b"))
LAYER_WORDS = (("cli", r"\bcli\b"), ("tui", r"\btui\b"), ("infra", r"\b(container|docker|devcontainer|compose|terraform)\b"))


def agent_lang(files, repo):
    counts = defaultdict(int)
    for f in files:
        lang = EXT_LANG.get(os.path.splitext(f)[1].lower())
        if lang:
            counts[lang] += 1
    if counts:
        return max(counts, key=counts.get)
    if files and all(f.lower().endswith(".md") for f in files):
        return "markdown"
    return (repo or {}).get("lang")


def infer_tags(desc, layer, ntasks, files, lines, repo):
    """Coarse tags for work nobody tagged, from its description, the files it touched and the repo's
    manifests. Low confidence by design (tags_source says so); novelty and verification stay unknown."""
    d = (desc or "").lower()
    tags = {"kind": next((k for k, rx in KIND_WORDS if re.search(rx, d)), "extend")}
    lay = layer if layer in vocab().get("layer", {}).get("values", {}) else next((k for k, rx in LAYER_WORDS if re.search(rx, d)), None)
    if lay:
        tags["layer"] = lay
    tags["surface"] = next((k for k, rx in SURFACE_WORDS if re.search(rx, d)), "other")
    lang = agent_lang(files, repo)
    if lang:
        tags["lang"] = lang
        st = [v for v in (repo or {}).get("stack", []) if vocab()["stack"]["values"].get(v) == lang]
        if st:
            tags["stack"] = st
    if files or lines:
        tags["size"] = "S" if len(files) <= 3 and lines < 150 else "M" if len(files) <= 10 and lines < 600 else "L"
    elif ntasks:
        tags["size"] = size_of(ntasks)
    if layer == "qa" or re.search(r"\be2e\b", d):
        tags["verification"] = "e2e"
    return tags


# ---------------------------------------------------------------- specs

TASK_LINE = re.compile(r"^(\s*- \[[ xX]\] )(T\d{3,})\b")
TOKENS_TAG = re.compile(r"\s*·\s*Tokens:[^\n]*$")
TAGS_TAG = re.compile(r"\s*·\s*Tags:[^\n]*$")
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
            tg = re.search(r"Tags:\s*([^·|\n]*)", line)
            tasks[m.group(2)] = {"line": n, "layer": layer, "executor": ex and ex.group(1), "effort": ef and ef.group(1),
                                 "projected": parse_tok(tag.group(1)) if tag else None,
                                 "tags": parse_tags(tg.group(1)) if tg else {}}
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
                          "projected": None, "dispatch": None, "tags": {}}
    for m in re.finditer(r"^## Card `([^`]+)`[^\n]*\n(.*?)(?=^## |\Z)", text, re.S | re.M):
        c = cards.setdefault(m.group(1), {"tasks": expand_ids(m.group(0).splitlines()[0]), "executor": None,
                                          "reviewer": None, "effort": None, "projected": None, "dispatch": None, "tags": {}})
        tk = re.search(r"Tokens:\s*projected\s*([^|·`\n]*)", m.group(2))
        c["projected"] = parse_tok(tk.group(1)) if tk else None
        tg = re.search(r"Tags:\s*([^·|`\n]*)", m.group(2))
        c["tags"] = parse_tags(tg.group(1)) if tg else {}
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
    if len(mention) > 1:
        # several features named: only a description that leads with one is read as that feature's
        # card (the rest being an aside); a spec-wide agent ("Plan+tasks 001, 002, 014") stays unattributed
        if re.match(rf"\W*(?:T\d+\W+)*{mention[0]}\b", desc):
            how = "first of several features, leading the description"
        else:
            return res
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


FIX_RE = re.compile(r"\b(fix|address|rework|findings?)\b", re.I)
FLAKE_RE = re.compile(r"\b(flak\w*|ci (?:watch|fail\w*|fix)|red run)\b", re.I)


def process_kind(desc):
    """Work that is process overhead rather than the task itself: a review, a fix round after one,
    or a CI flake hunt. None for the task's own work."""
    if re.match(r"\s*review", desc, re.I):
        return "reviews"
    if FLAKE_RE.search(desc):
        return "ci_flake"
    if re.match(r"\s*(fix|address|rework)\b", desc, re.I) or re.search(r"\bfindings?\b", desc, re.I):
        return "fix_rounds"
    return None


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
        kinds = {r["aid"]: process_kind(r["desc"]) for r in rs}
        main = [r for r in rs if kinds[r["aid"]] is None] or [r for r in rs if kinds[r["aid"]] != "fix_rounds"] or rs
        lead = max(main, key=lambda r: r["billable"])
        g["model"] = lead["family"]
        g["desc"] = lead["desc"]
        ev = defaultdict(int)
        extra_cost = 0
        for r in rs:
            k = kinds[r["aid"]]
            if r is not lead and k in ("fix_rounds", "ci_flake"):
                ev[k] += 1
                extra_cost += r["billable"]
            elif k == "reviews":
                ev["reviews"] += 1
            if r["resumed"]:
                ev["resumed"] += r["resumed"]
                extra_cost += r["resume_cost"] if not (r is not lead and k in ("fix_rounds", "ci_flake")) else 0
        g["events"] = dict(ev)
        g["event_cost"] = min(extra_cost, g["billable"])
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

def planned_tags(g, feats):
    f = feats.get(g["feature"]) if g["feature"] else None
    if not f:
        return {}
    c = f["cards"].get(g["card"]) if g["card"] else None
    if c and c["tags"]:
        return c["tags"]
    first = sorted((t for t in g["tasks"] if t in f["tasks"]), key=lambda t: f["tasks"][t]["line"])
    return f["tasks"][first[0]]["tags"] if first else {}


def make_record(project, g, feats, coord_card, source="measured", repo=None):
    f = feats.get(g["feature"]) if g["feature"] else None
    c = f["cards"].get(g["card"]) if f and g["card"] else None
    model = g["model"]
    key = g["card"] or (tasks_key(g["tasks"]) if g["tasks"] else g["rows"][0]["aid"])
    ident = f"{project}:{g['feature']}:{key}" if g["feature"] and g["confidence"] != "agent" and (g["card"] or g["tasks"]) \
        else f"{project}:agent:{g['rows'][0]['aid']}"
    if source == "seed":
        ident = "seed:" + ident
    layer = layer_of(g, feats)
    planned = planned_tags(g, feats)
    guess = infer_tags(g["desc"], layer, len(g["tasks"]), g["files"], g["lines"], repo)
    tags = {**guess, **planned}
    return {"id": ident, "source": source, "confidence": g["confidence"], "project": project, "feature": g["feature"],
            "card": g["card"], "tasks": g["tasks"], "layer": layer, "effort": effort_of(g, feats),
            "tags": tags, "tags_source": ("planned+inferred" if planned and set(guess) - set(planned) else "planned") if planned else "inferred",
            "executor_role": (c and c["executor"]) or ROLE_OF_FAMILY.get(model, "builder"), "executor_model": model,
            "files": len(g["files"]), "lines": g["lines"], "projected": projected_of(g, feats), "used": g["billable"],
            "used_clean": g["billable"] - g["event_cost"], "events": g["events"],
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
            line = TOKENS_TAG.sub("", lines[n]).rstrip()
            if i == 0 and u.get("tags"):
                line = TAGS_TAG.sub("", line).rstrip() + f" · Tags: {format_tags(u['tags'])}"
            elif i and u.get("tags"):
                line = TAGS_TAG.sub("", line).rstrip()
            new = line + (tag_for(u["projected"], u["used"]) if i == 0 else tag_for(0, 0, mine[0]))
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
    repo = repo_info(os.getcwd())
    groups = build_groups(rows, feats)
    share, unassigned, coord_total = split_coordinator(main_msgs, groups, snaps)

    rollup = a.rollup_out or (None if a.no_legacy else DEFAULT_ROLLUP)
    if rollup and not a.json:
        L, grand = render(project, sessions, roots, coords, rows, tagged, snaps)
        if not a.dry_run:
            os.makedirs(os.path.dirname(os.path.abspath(rollup)), exist_ok=True)
            with open(rollup, "w", encoding="utf-8") as fh:
                fh.write("\n".join(L))
    legacy = (0, 0)
    if not a.no_legacy and not a.dry_run:
        legacy = write_legacy(rows, a.specs_dir, rollup)

    feat_bill = defaultdict(int)
    for g in groups:
        if g["feature"]:
            feat_bill[g["feature"]] += g["billable"]
    records, by_card, summary, spans = [], {}, {}, []
    for fname, feat in feats.items():
        mine = [g for g in groups if g["feature"] == fname]
        units, extras = [], []
        for cid, c in feat["cards"].items():
            if c["projected"] is not None and not any(g["card"] == cid for g in mine):
                units.append({"label": f"`{cid}`", "tasks": c["tasks"], "projected": c["projected"], "used": None, "cache_read": 0,
                              "tags": c["tags"]})
        for g in mine:
            cshare = share.get(fname, 0) * g["billable"] / feat_bill[fname] if feat_bill[fname] else 0
            rec = make_record(project, g, feats, cshare, repo=repo)
            records.append(rec)
            if g["card"]:
                spans.append({"card": g["card"], "feature": fname, "model": rec["executor_model"], "projected": rec["projected"],
                              "used": g["billable"], "start": g["start"] + "Z", "end": g["end"] + "Z",
                              "agents": len(g["rows"]), "calls": g["calls"]})
            u = {"label": f"`{g['card']}`" if g["card"] else (f"{tasks_key(g['tasks'])}" if g["tasks"] else None),
                 "tasks": g["tasks"], "projected": rec["projected"], "used": g["billable"], "cache_read": g["cache_read"],
                 "desc": g["desc"], "tags": planned_tags(g, feats)}
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
    records += [make_record(project, g, feats, None, repo=repo) for g in unattributed]
    if not a.dry_run:
        n = upsert_history(a.history, records)
        write_ledger(a.ledger, by_card, False)
    else:
        n = len(records)

    if a.json:
        un = [g for g in groups if g["confidence"] == "agent" or not g["feature"]]
        print(json.dumps({"cards": spans, "unattributed": len(un)}))
        return
    print(f"{project}: {len(rows)} subagent(s) in {len(sessions)} session(s); "
          f"{'dry run, nothing written' if a.dry_run else 'wrote ' + a.history + f' ({n} records)'}")
    print("feature | used (subagents) | coordinator share | total | cards | task lines")
    for fname, s in sorted(summary.items()):
        if s["used"] or s["cards"]:
            print(f"{fname} | {fmt_tok(s['used'])} | {fmt_tok(round(s['coordinator']))} | "
                  f"{fmt_tok(s['used'] + round(s['coordinator']))} | {s['cards']} | {s['lines']}")
    if not a.no_legacy:
        print(f"deprecated legacy output: {legacy[0]} specs/*/tokens.md, {legacy[1]} unmapped card(s) in {rollup} "
              "(--no-legacy turns it off; both go in a later major)")
    un = sum(g["billable"] for g in unattributed)
    print(f"unattributed (reported, not guessed): {len(unattributed)} agent(s), {fmt_tok(un)}")
    for g in unattributed:
        print(f"  {fmt_tok(g['billable']):>7}  {g['model']:<6} {g['desc'][:80]}")
    print(f"coordinator: {fmt_tok(round(coord_total))} billable in all, {fmt_tok(round(unassigned))} in windows "
          f"with no attributed subagents")
    agent_total = sum(r["billable"] for r in rows)
    print(f"total: {fmt_tok(round(agent_total + coord_total))} billable ({fmt_tok(agent_total)} subagents)")


# --------------------------------------------------- legacy outputs (deprecated)
# Kept for projects that read them; `tasks --no-legacy` turns the first off. Both go in a later major.

def feature_task_ids(specs_dir):
    """feature dir -> set of task ids defined in its tasks.md (the legacy attribution)."""
    out = {}
    for tasks_md in sorted(glob.glob(os.path.join(specs_dir, "[0-9][0-9][0-9]-*", "tasks.md"))):
        with open(tasks_md, encoding="utf-8") as fh:
            ids = set(TASK_ID.findall(fh.read()))
        if ids:
            out[os.path.dirname(tasks_md)] = ids
    return out


def write_legacy(rows, specs_dir, rollup_path):
    """specs/<feature>/tokens.md per touched feature, and an Unmapped section in the roll-up for
    cards whose description names no task id. Attribution is the old one: the first feature whose
    tasks.md defines a named task id. Returns (features written, unmapped cards)."""
    feature_of = {t: f for f, ids in feature_task_ids(specs_dir).items() for t in ids}
    per_feature, unmapped = defaultdict(list), []
    for r in rows:
        features = {feature_of[i] for i in TASK_ID.findall(r["desc"]) if i in feature_of}
        if features:
            per_feature[sorted(features)[0]].append(r)
        else:
            unmapped.append(r)
    for feature, feat_rows in sorted(per_feature.items()):
        out = [f"# Token usage — {os.path.basename(feature)}", "",
               f"Generated {datetime.now().strftime('%Y-%m-%d %H:%M')}. Deprecated output of `token_report.py tasks` "
               "(use the `Tokens:` tags on tasks.md, tracker.md and docs/product/token-history.jsonl); "
               "turn it off with `--no-legacy`. Linked to `tasks.md` by task id; a record file, never written into it. "
               "Cards spanning several features attribute their full cost to the first feature their task ids name.", "",
               "| Task(s) | Who | Model(s) | Calls | Billable | Total |", "|---|---|---|---|---|---|"]
        for r in sorted(feat_rows, key=lambda r: r["start"]):
            out.append(f"| {', '.join(TASK_ID.findall(r['desc'])) or '—'} | {r['who']} | {r['models']} | {r['calls']} | "
                       f"{k(r['billable'])} | {k(r['total'])} |")
        out += [f"| **Feature total** | | | | **{k(sum(r['billable'] for r in feat_rows))}** | "
                f"**{k(sum(r['total'] for r in feat_rows))}** |", ""]
        with open(os.path.join(feature, "tokens.md"), "w", encoding="utf-8") as fh:
            fh.write("\n".join(out))
    if unmapped:
        with open(rollup_path, "a", encoding="utf-8") as fh:
            fh.write("\n## Unmapped\n\nCards whose description names no task id, so their cost isn't linked "
                     "to any feature's `tasks.md`:\n\n| Who | Task | Billable | Total |\n|---|---|---|---|\n")
            for r in unmapped:
                fh.write(f"| {r['who']} | {r['desc']} | {k(r['billable'])} | {k(r['total'])} |\n")
    return len(per_feature), len(unmapped)


def parse_k(s: str) -> float:
    s = s.strip().replace(",", "").replace("*", "")
    if s.endswith("M"):
        return float(s[:-1]) * 1_000_000
    if s.endswith("k"):
        return float(s[:-1]) * 1_000
    return float(s) if s else 0.0


def read_by_agent(rollup_path):
    """Rows of a roll-up's '## By agent' table (the legacy estimate source)."""
    if not rollup_path or not os.path.exists(rollup_path):
        return []
    with open(rollup_path, encoding="utf-8") as fh:
        text = fh.read()
    m = re.search(r"## By agent\n\n(.*?)(?:\n## |\Z)", text, re.S)
    if not m:
        return []
    rows = []
    for ln in [ln for ln in m.group(1).splitlines() if ln.startswith("|")][2:]:
        cells = [c.strip() for c in ln.strip("|").split("|")]
        if len(cells) >= 8:
            rows.append({"label": cells[2], "model": cells[4], "billable": parse_k(cells[6]), "total": parse_k(cells[7])})
    return rows


def legacy_estimate(a, fam):
    """Average billable of the roll-up's cards for a label or model, or None when it has none."""
    rows = read_by_agent(a.rollup)
    sel = [r for r in rows if r["label"] == a.label] if a.label else [r for r in rows if fam in r["model"]]
    if not sel:
        return None
    how = f"label '{a.label}'" if a.label else f"model '{fam}'"
    return (f"{how}: {len(sel)} sample(s) in {a.rollup}, average billable {k(int(sum(r['billable'] for r in sel) / len(sel)))}, "
            f"average total {k(int(sum(r['total'] for r in sel) / len(sel)))} (deprecated roll-up fallback)")


# ------------------------------------------------- estimate, index, seed

MIN_SAMPLES = 5
CONF_WEIGHT = {"exact": 1.0, "feature": 0.6, "agent": 0.3}
SIZE_MULT = {"S": 0.6, "M": 1.0, "L": 1.6}
# The starting table: billable tokens per dispatched card, (median, p75), by model and effort, for a
# medium card (4-6 tasks). Calibrated on rstory's first autonomous run (2026-10): Sonnet feature
# builders 0.3-0.8M each (median about 0.6M, a high-effort card), Opus agents 0.5-0.9M each. Haiku
# is an assumption. It is used only while history has fewer than MIN_SAMPLES similar records.
STARTING = {
    "sonnet": {"low": (150_000, 250_000), "medium": (350_000, 550_000), "high": (600_000, 850_000)},
    "opus": {"low": (250_000, 400_000), "medium": (450_000, 650_000), "high": (700_000, 950_000)},
    "haiku": {"low": (60_000, 120_000), "medium": (120_000, 200_000), "high": (200_000, 320_000)},
}


def ai_gent_home():
    return os.environ.get("AI_GENT_HOME") or os.path.expanduser("~/.ai-gent")


def default_index():
    return os.path.join(ai_gent_home(), "token-history.jsonl")


def size_of(n):
    return None if not n else "S" if n <= 3 else "M" if n <= 6 else "L"


def wquantile(pairs, q):
    """Weighted quantile of (value, weight) pairs: the first value whose cumulative weight reaches q."""
    pairs = sorted(pairs)
    total = sum(w for _, w in pairs)
    acc = 0.0
    for v, w in pairs:
        acc += w
        if acc >= q * total - 1e-9:
            return v
    return pairs[-1][0]


def load_history(paths):
    """Records from the given files, by id (the first file wins); a seeded record whose agent a
    measured record already counts is dropped, so one agent never counts twice."""
    by_id = {}
    for path in paths:
        for r in read_history(path):
            by_id.setdefault(r["id"], r)
    measured = {a for r in by_id.values() if r.get("source") != "seed" for a in r.get("agents", [])}
    return [r for r in by_id.values()
            if not (r.get("source") == "seed" and measured & set(r.get("agents", [])))]


# Matching levels, strictest first. A level is the facets a record must share exactly; `stack` is never
# a filter, only a similarity bonus (Jaccard) that is switched off from the third level on.
# Back-off drops size, then stack, then verification, then layer, then novelty.
LEVELS = (("kind", "surface", "novelty", "lang", "layer", "verification", "size", "+stack"),
          ("kind", "surface", "novelty", "lang", "layer", "verification", "+stack"),
          ("kind", "surface", "novelty", "lang", "layer", "verification"),
          ("kind", "surface", "novelty", "lang", "layer"),
          ("kind", "surface", "novelty", "lang"),
          ("kind", "surface", "lang"),
          ("kind", "lang"))
FULL_FACETS = LEVELS[0][:-1]
NEIGHBOUR_BONUS = 0.25   # per dropped facet a record still shares
STACK_BONUS = 0.5        # at full stack overlap
INFERRED_WEIGHT = 0.5    # tags guessed by seed, not planned
CONTEXT_PRIOR = {"sonnet": 1.0, "opus": 1.3, "haiku": 0.5}   # relative to a Sonnet builder, until history says otherwise
CONTEXT_REF = "sonnet/builder"
BIAS_MIN, BIAS_CLAMP = 5, (0.5, 2.0)
LOGLINEAR_MIN = 300


def rec_cost(r):
    """What a record teaches about the task itself: its cost without resumes, fix rounds and flake hunts."""
    return r.get("used_clean") or r.get("used") or 0


def rec_tags(r):
    t = dict(r.get("tags") or {})
    if "size" not in t and r.get("tasks"):
        t["size"] = size_of(len(r["tasks"]))
    return t


def ctx_key(model, role):
    return f"{model}/{role}"


def learn_context(records):
    """Execution-context factors relative to a Sonnet builder, learned from history: records that share
    kind, surface, novelty, lang and layer but ran in another context are compared with the Sonnet-builder
    mean of their group. A key needs MIN_SAMPLES such comparisons; others fall back to the prior."""
    groups = defaultdict(list)
    for r in records:
        t = rec_tags(r)
        if rec_cost(r) and all(t.get(f) for f in ("kind", "surface", "novelty", "lang", "layer")):
            groups[tuple(t[f] for f in ("kind", "surface", "novelty", "lang", "layer"))].append(r)
    dev = defaultdict(list)
    for g in groups.values():
        ref = [math.log(rec_cost(r)) for r in g if ctx_key(r.get("executor_model"), r.get("executor_role")) == CONTEXT_REF]
        if not ref:
            continue
        base = sum(ref) / len(ref)
        for r in g:
            k = ctx_key(r.get("executor_model"), r.get("executor_role"))
            if k != CONTEXT_REF:
                d = math.log(rec_cost(r)) - base
                dev[k].append(d)
                dev[r.get("executor_model")].append(d)
    return {k: math.exp(sum(v) / len(v)) for k, v in dev.items() if len(v) >= MIN_SAMPLES}


def context_factor(learned, model, role):
    """(factor, 'learned'|'prior') for a model and role."""
    for k in (ctx_key(model, role), model):
        if k in learned:
            return learned[k], "learned"
    return (1.0 if ctx_key(model, role) == CONTEXT_REF else CONTEXT_PRIOR.get(model, 1.0)), "prior"


def bias_factors(records):
    """(facet, value) -> (used/projected geometric mean, records), for tags with at least BIAS_MIN
    records that carry both a projection and a result. Corrects a planner that is systematically off."""
    acc = defaultdict(list)
    for r in records:
        if r.get("projected") and rec_cost(r):
            ratio = math.log(rec_cost(r) / r["projected"])
            for f, v in rec_tags(r).items():
                for x in (v if isinstance(v, list) else [v]):
                    acc[(f, x)].append(ratio)
    return {k: (math.exp(sum(v) / len(v)), len(v)) for k, v in acc.items() if len(v) >= BIAS_MIN}


def loglinear_model(records):
    """Hook for a log-linear model (log cost ~ tag indicators + context + size) once history holds
    LOGLINEAR_MIN records. Not built: it returns None, and estimate_card falls back to the matching
    levels. To build it, return an object with predict(query_tags, model, role) -> (median, p75)."""
    return None


def jaccard(a, b):
    a, b = set(a or []), set(b or [])
    return len(a & b) / len(a | b) if a | b else 0.0


def estimate_by_tags(records, family_, role, tags):
    """Weighted nearest neighbours by shared tags with hierarchical back-off; None when no level
    reaches MIN_SAMPLES. Returns the projection, p75, the level, the records used and the factors applied."""
    q = {f: v for f, v in tags.items() if v}
    learned = learn_context(records)
    qfac, qhow = context_factor(learned, family_, role)
    seen = set()
    for idx, level in enumerate(LEVELS):
        facets = tuple(f for f in level if f != "+stack" and f in q)
        use_stack = "+stack" in level and "stack" in q
        if not facets or (facets, use_stack) in seen:
            continue
        seen.add((facets, use_stack))
        dropped = [f for f in FULL_FACETS if f in q and f not in facets]
        pairs, mix = [], defaultdict(int)
        for r in records:
            t = rec_tags(r)
            if not rec_cost(r) or any(t.get(f) != q[f] for f in facets):
                continue
            w = CONF_WEIGHT.get(r.get("confidence"), 0.3)
            if r.get("tags_source") == "inferred":
                w *= INFERRED_WEIGHT
            w *= 1 + NEIGHBOUR_BONUS * sum(1 for f in dropped if t.get(f) == q[f])
            if use_stack:
                w *= 1 + STACK_BONUS * jaccard(t.get("stack"), q.get("stack"))
            v = rec_cost(r) / context_factor(learned, r.get("executor_model"), r.get("executor_role"))[0]
            if "size" not in facets and q.get("size") in SIZE_MULT and t.get("size") in SIZE_MULT:
                v *= SIZE_MULT[q["size"]] / SIZE_MULT[t["size"]]
            pairs.append((v, w))
            mix[r.get("confidence", "agent")] += 1
        if len(pairs) < MIN_SAMPLES:
            continue
        med, p75 = wquantile(pairs, 0.5) * qfac, wquantile(pairs, 0.75) * qfac
        bias = bias_factors(records)
        used = [(f"{f}={x}", bias[(f, x)]) for f, v in q.items() for x in (v if isinstance(v, list) else [v]) if (f, x) in bias]
        corr = 1.0
        if used:
            corr = min(max(math.exp(sum(math.log(b[0]) for _, b in used) / len(used)), BIAS_CLAMP[0]), BIAS_CLAMP[1])
        label = "high" if idx <= 1 else "medium" if idx <= 4 else "low"
        return {"median": round(med * corr), "p75": round(p75 * corr), "n": len(pairs), "basis": "tags", "tier": "+".join(facets) + ("+stack" if use_stack else ""),
                "level": idx, "confidence_label": label, "mix": dict(mix), "context": [round(qfac, 2), qhow],
                "bias": {"factor": round(corr, 2), "from": [f"{k} x{b[0]:.2f} (n={b[1]})" for k, b in used]}}
    return None


def estimate_card(records, family_, effort=None, layer=None, tasks=None, tags=None, role=None):
    """The projection for a planned card: the weighted median of similar records (p75 as the upper
    bound). With tags: nearest neighbours by shared tags, backing off until MIN_SAMPLES records agree;
    otherwise (or when even that fails) effort and model tiers; else the starting table."""
    if tags and loglinear_model(records) is not None and len(records) >= LOGLINEAR_MIN:
        pass  # the hook: a fitted model would answer here
    if tags:
        got = estimate_by_tags(records, family_, role or "builder", tags)
        if got:
            return got
        layer = layer or tags.get("layer")
        tasks = tasks or None
    size = size_of(tasks) or (tags or {}).get("size")
    tiers = [("layer+effort+model+size", {"layer": layer, "effort": effort, "size": size}),
             ("effort+model+size", {"effort": effort, "size": size}),
             ("effort+model", {"effort": effort}), ("model", {})]

    def match(r, crit):
        return (rec_cost(r) and r.get("executor_model") == family_
                and all(v is None or (rec_tags(r).get("size") if k == "size" else r.get(k)) == v for k, v in crit.items()))

    seen, best = set(), 0
    for name, crit in tiers:
        key = tuple(sorted((k, v) for k, v in crit.items() if v is not None))
        if key in seen:
            continue
        seen.add(key)
        sel = [r for r in records if match(r, crit)]
        best = max(best, len(sel))
        if len(sel) >= MIN_SAMPLES:
            pairs = [(rec_cost(r), CONF_WEIGHT.get(r.get("confidence"), 0.3)) for r in sel]
            mix = defaultdict(int)
            for r in sel:
                mix[r.get("confidence", "agent")] += 1
            return {"median": wquantile(pairs, 0.5), "p75": wquantile(pairs, 0.75), "n": len(sel), "basis": "history",
                    "tier": name, "mix": dict(mix), "confidence_label": "low" if tags else None}
    med, p75 = STARTING.get(family_, STARTING["sonnet"])[effort if effort in EFFORT_RANK else "medium"]
    mult = SIZE_MULT.get(size, 1.0)
    return {"median": round(med * mult), "p75": round(p75 * mult), "n": best, "basis": "starting table", "tier": None, "mix": {},
            "confidence_label": "low"}


def history_paths(extra):
    return list(extra or []) + [p for p in (DEFAULT_HISTORY, default_index()) if os.path.exists(p)]


def cmd_estimate(a):
    fam = ROLE_MODEL.get(a.role) if a.role else family(a.model)
    tags = parse_tags(a.tags) if a.tags else None
    est = estimate_card(load_history(history_paths(a.history)), fam, a.effort, a.layer, a.tasks, tags, a.role or "builder")
    if a.label or (est["basis"] == "starting table" and not a.json):
        old = legacy_estimate(a, fam)
        if old:
            print(old)
            return
        if a.label:
            print(f"no history for label '{a.label}' in {a.rollup} — fall back to S/M/L sizing")
            return
    label = f"{fam} · effort {a.effort}" + (f" · layer {a.layer}" if a.layer else "") + (f" · {a.tasks} task(s)" if a.tasks else "")
    if tags:
        label += f" · tags {format_tags(tags)}"
    if a.json:
        print(json.dumps({"query": label, **est}))
        return
    print(f"estimate for {label}")
    print(f"  projection ~{fmt_tok(est['median'])} billable (median), upper bound ~{fmt_tok(est['p75'])} (p75)")
    mix = ", ".join(f"{n} {c}" for c, n in sorted(est["mix"].items(), key=lambda x: -CONF_WEIGHT.get(x[0], 0)))
    if est["basis"] == "tags":
        print(f"  basis: {est['n']} record(s) at match level {est['level']} ({est['tier']}), confidence {est['confidence_label']} ({mix})")
        print(f"  context: x{est['context'][0]:.2f} for {fam}/{a.role or 'builder'} ({est['context'][1]}); "
              f"bias correction x{est['bias']['factor']:.2f}"
              + (f" ({'; '.join(est['bias']['from'])})" if est["bias"]["from"] else " (fewer than 5 records per tag)"))
    elif est["basis"] == "history":
        print(f"  basis: {est['n']} similar record(s) matching {est['tier']} ({mix}); exact weighs 1.0, "
              f"feature-level {CONF_WEIGHT['feature']}, agent-only {CONF_WEIGHT['agent']}"
              + ("; tags matched too few records, so the match fell back to effort and model" if tags else ""))
    else:
        print(f"  basis: the starting table (only {est['n']} similar record(s); {MIN_SAMPLES} needed for history to take over)")
    print(f"  on the card: Tokens: projected ~{fmt_tok(est['median'])} | used —")


def project_records(records, project):
    return [r for r in records if r.get("project") == project]


def cmd_index(a):
    a.index = a.index or default_index()
    paths = list(a.repo or [])
    for d in a.scan:
        paths += sorted(glob.glob(os.path.join(os.path.expanduser(d), "*", DEFAULT_HISTORY)))
    if os.path.exists(DEFAULT_HISTORY):
        paths.append(DEFAULT_HISTORY)
    paths = [os.path.join(p, DEFAULT_HISTORY) if os.path.isdir(p) else p for p in paths]
    repo_recs = {}
    for p in dict.fromkeys(os.path.abspath(x) for x in paths):
        for r in read_history(p):
            repo_recs[r["id"]] = r
    seeds = [r for r in read_history(a.index) if r.get("source") == "seed"]
    merged = load_history_from(list(repo_recs.values()) + seeds)
    os.makedirs(os.path.dirname(os.path.abspath(a.index)), exist_ok=True)
    with open(a.index, "w", encoding="utf-8") as fh:
        for r in sorted(merged, key=lambda r: (r.get("date", ""), r["id"])):
            fh.write(json.dumps(r, ensure_ascii=False) + "\n")
    counts = defaultdict(int)
    for r in merged:
        counts[(r.get("project"), r.get("source"))] += 1
    print(f"wrote {a.index}: {len(merged)} record(s) from {len(set(paths))} repo history file(s) and {len(seeds)} seeded")
    for (proj, src), n in sorted(counts.items(), key=lambda x: str(x)):
        print(f"  {proj}: {n} {src}")


def load_history_from(records):
    by_id = {}
    for r in records:
        by_id.setdefault(r["id"], r)
    measured = {x for r in by_id.values() if r.get("source") != "seed" for x in r.get("agents", [])}
    return [r for r in by_id.values() if not (r.get("source") == "seed" and measured & set(r.get("agents", [])))]


SEED_PROJECTS = ("-home-lucas-codes-llm4glpi", "-home-lucas-codes-ai-gent", "-home-lucas-codes-secrets4claude",
                 "-home-lucas-codes-dottod", "-home-lucas-codes-rstory")


def cmd_seed(a):
    """Read-only on the sessions and on each repo's specs; writes only the local index."""
    a.index = a.index or default_index()
    projects = a.project or list(SEED_PROJECTS)
    new, report = [], {}
    for slug in projects:
        name = project_name(slug)
        roots = sorted(glob.glob(os.path.expanduser(f"~/.claude*/projects/{slug}")))
        sessions = discover_sessions(roots)
        _, rows, _, _ = gather(roots, sessions)
        repo_dir = os.path.join(os.path.expanduser(a.codes_dir), name)
        feats = load_features(os.path.join(repo_dir, "specs"))
        repo = repo_info(repo_dir)
        recs = [make_record(name, g, feats, None, source="seed", repo=repo) for g in build_groups(rows, feats) if g["billable"] > 0]
        new += recs
        c = defaultdict(int)
        for r in recs:
            c[r["confidence"]] += 1
        report[name] = (len(roots), len(sessions), len(rows), dict(c), sum(r["used"] for r in recs))
    if not a.dry_run:
        mine = {project_name(s) for s in projects}
        keep = [r for r in read_history(a.index) if not (r.get("source") == "seed" and r.get("project") in mine)]
        os.makedirs(os.path.dirname(os.path.abspath(a.index)), exist_ok=True)
        with open(a.index, "w", encoding="utf-8") as fh:
            for r in sorted(keep + new, key=lambda r: (r.get("date", ""), r["id"])):
                fh.write(json.dumps(r, ensure_ascii=False) + "\n")
    print(f"{'dry run: ' if a.dry_run else ''}seeded {len(new)} record(s) into {a.index} (sessions and specs read-only)")
    print("project | config dirs | sessions | subagents | records by confidence | billable")
    for name, (dirs, sess, agents, c, bill_) in report.items():
        conf = ", ".join(f"{c.get(x, 0)} {x}" for x in ("exact", "feature", "agent"))
        print(f"{name} | {dirs} | {sess} | {agents} | {conf} | {fmt_tok(bill_)}")


# ---------------------------------------------------------------- budget

BUDGET_DEFAULTS = {"max_opus": 1, "max_agents": 2, "reserve_pct": 10, "coordinator_overhead_pct": 35}


def parse_budget(path):
    """.ai-gent/budget.yaml: flat 'key: value' lines (12M, 800k or plain numbers; # comments).
    Caps are session_billable and week_billable; absent caps mean no cap."""
    cfg = dict(BUDGET_DEFAULTS)
    if path and os.path.exists(path):
        with open(path, encoding="utf-8") as fh:
            for line in fh:
                m = re.match(r"^\s*([a-z_]+)\s*:\s*([^#\s]+)", line)
                if m and parse_tok(m.group(2)) is not None:
                    cfg[m.group(1)] = parse_tok(m.group(2))
    return cfg


def config_dirs():
    env = os.environ.get("CLAUDE_CONFIG_DIR")
    return [os.path.expanduser(env)] if env else sorted(glob.glob(os.path.expanduser("~/.claude*")))


def usage_messages(cutoff_iso, dirs):
    """The (utc timestamp, model, usage) of every message of the given config dirs' projects from a UTC
    timestamp on, deduplicated by message id over all of them (an account switch is not counted twice)."""
    msgs = {}
    floor = datetime.fromisoformat(cutoff_iso).replace(tzinfo=timezone.utc).timestamp() - 3600
    for cfg in dirs:
        for f in (glob.glob(os.path.join(cfg, "projects", "*", "*.jsonl")) +
                  glob.glob(os.path.join(cfg, "projects", "*", "*", "subagents", "agent-*.jsonl"))):
            if os.path.getmtime(f) >= floor:
                collect(f, msgs)
    return [m for m in msgs.values() if m[0] and m[0] >= cutoff_iso]


def usage_since(cutoff_iso, dirs, now=None):
    """Billable tokens across every project of the given config dirs since a UTC timestamp."""
    return sum(bill(u) for _, _, u in usage_messages(cutoff_iso, dirs))


def roles_models(path=os.path.join(".ai-gent", "orchestrate.yaml")):
    """role -> model family: agent-roles.md's defaults, overridden by the project's orchestrate.yaml."""
    out = dict(ROLE_MODEL)
    if os.path.exists(path):
        with open(path, encoding="utf-8") as fh:
            for m in re.finditer(r"^\s{2}(\w+):\s*\{[^}]*model:\s*(\w+)", fh.read(), re.M):
                out[m.group(1)] = family(m.group(2))
    return out


def wave_cards(a, feats, history):
    """One dict per card of the wave: label, model family, role, projected, p75, how, confidence."""
    roles = roles_models()
    out = []
    for item in (a.projected or "").split(","):
        if item.strip():
            fam, _, amt = item.strip().rpartition(":")
            out.append({"label": item.strip(), "model": family(fam) if fam else "sonnet", "role": "builder",
                        "projected": parse_tok(amt) or 0, "p75": None, "how": "given", "conf": None})
    for cid in [c for c in (a.cards or "").split(",") if c.strip()]:
        cid = cid.strip()
        feat = next((f for f in feats.values() if cid in f["cards"]), None)
        if not feat:
            out.append({"label": cid, "model": "sonnet", "role": "builder", "projected": 0, "p75": None,
                        "how": "card not found", "conf": None})
            continue
        c = feat["cards"][cid]
        role = c["executor"] or "builder"
        fam = roles.get(role, "sonnet")
        layer = layer_of({"card": cid, "desc": "", "tasks": c["tasks"], "feature": feat["name"]}, feats)
        est = estimate_card(history, fam, c["effort"] or "medium", layer, len(c["tasks"]), c["tags"] or None, role)
        if c["projected"] is not None:
            out.append({"label": cid, "model": fam, "role": role, "projected": c["projected"], "p75": est["p75"],
                        "how": "cards.md", "conf": est.get("confidence_label")})
        else:
            out.append({"label": cid, "model": fam, "role": role, "projected": est["median"], "p75": est["p75"],
                        "how": f"estimated, {est['basis']}", "conf": est.get("confidence_label")})
    return out


PRIOR_EVENT_RATE = 0.08      # resumes, fix rounds and flake hunts as a share of cost, until history knows
OPUS_PARALLEL_RISK = 0.12    # per extra concurrent Opus agent: more limit stops, more resumes


def event_rate(history):
    """(share of spend that went to process events, 'history'|'prior')."""
    recs = [r for r in history if r.get("used")]
    if len(recs) >= MIN_SAMPLES and any("used_clean" in r for r in recs):
        total = sum(r["used"] for r in recs)
        return (total - sum(rec_cost(r) for r in recs)) / total, "history"
    return PRIOR_EVENT_RATE, "prior"


def pack_batches(cards, max_agents, max_opus):
    """In dependency (given) order: batches of at most max_agents subagents, at most max_opus on Opus."""
    batches, cur = [], []
    for c in cards:
        opus = sum(1 for x in cur if x["model"] == "opus")
        if cur and (len(cur) >= max_agents or (c["model"] == "opus" and opus >= max_opus)):
            batches.append(cur)
            cur = []
        cur.append(c)
    return batches + ([cur] if cur else [])


def cmd_budget(a):
    say = (lambda *args, **kwargs: None) if a.json else print
    cfg = parse_budget(a.budget)
    slug = a.project or re.sub(r"[^A-Za-z0-9-]", "-", os.getcwd())
    dirs = config_dirs()
    roots = sorted(r for d in dirs for r in glob.glob(os.path.join(d, "projects", slug)))
    sid = (a.session or [None])[0]
    if not sid and roots:
        files = [f for r in roots for f in glob.glob(os.path.join(r, "*.jsonl"))]
        sid = os.path.splitext(os.path.basename(max(files, key=os.path.getmtime)))[0] if files else None
    now = datetime.now(timezone.utc)
    if cfg.get("session_window_hours"):
        since = (now - timedelta(hours=cfg["session_window_hours"])).strftime("%Y-%m-%dT%H:%M:%S")
        used_session, scope = usage_since(since, dirs), f"the last {cfg['session_window_hours']:g}h, all projects"
    elif sid:
        coords, rows, _, _ = gather(roots, [sid])
        used_session, scope = sum(r["billable"] for r in coords + rows), f"session {sid[:8]}"
    else:
        used_session, scope = 0, "no session found"
    used_week = usage_since((now - timedelta(days=7)).strftime("%Y-%m-%dT%H:%M:%S"), dirs)
    feats = load_features(a.specs_dir)
    history = load_history(history_paths(a.history))
    cards = wave_cards(a, feats, history)
    if not cards:
        sys.exit("budget: name the wave's cards with --cards ID,ID or --projected [model:]400k,...")

    overhead = cfg["coordinator_overhead_pct"] / 100
    reserve = cfg["reserve_pct"] / 100
    max_agents, max_opus = int(cfg["max_agents"]), int(cfg["max_opus"])
    rate, rate_src = event_rate(history)
    n_opus = sum(1 for c in cards if c["model"] == "opus")
    extra_opus = max(0, min(n_opus, max_opus) - 1)
    risk = rate + OPUS_PARALLEL_RISK * extra_opus
    factor = (1 + overhead) * (1 + risk)
    caps = [(n, cfg[k], u) for n, k, u in (("session", "session_billable", used_session), ("week", "week_billable", used_week)) if k in cfg]
    room = min((c - u for _, c, u in caps), default=None)
    avail = None if room is None else max(0, room) * (1 - reserve)

    say(f"wave: {len(cards)} card(s)")
    for c in cards:
        up = f", p75 ~{fmt_tok(c['p75'])}" if c["p75"] else ""
        cf = f", confidence {c['conf']}" if c["conf"] else ""
        say(f"  {c['label']:<24} {c['model']:<7} ~{fmt_tok(c['projected']):>6}  ({c['how']}{up}{cf})")
    total = sum(c["projected"] for c in cards)
    need = total * factor
    say(f"projected {fmt_tok(total)} + coordinator {cfg['coordinator_overhead_pct']:g}% + event risk {risk * 100:.0f}% "
          f"({rate_src}: resumes, fix rounds, flake hunts" + (f"; +{OPUS_PARALLEL_RISK * extra_opus * 100:.0f}% for {extra_opus + 1} Opus in parallel" if extra_opus else "")
          + f") = {fmt_tok(round(need))} billable")
    for name, cap, used in caps:
        say(f"{name}: cap {fmt_tok(cap)}, used {fmt_tok(used)} ({scope if name == 'session' else 'last 7 days, all projects'}), "
              f"left {fmt_tok(max(0, cap - used))}")
    if not caps:
        say(f"no caps in {a.budget}: set session_billable and week_billable to enable the check")
    fit, acc = len(cards), total
    if avail is None:
        verdict, why = "proceed", "no caps set"
    elif need <= avail:
        verdict, why = "proceed", f"within the {fmt_tok(round(avail))} left after a {cfg['reserve_pct']:g}% reserve"
    else:
        fit, acc = 0, 0.0
        for c in cards:
            if (acc + c["projected"]) * factor > avail:
                break
            acc, fit = acc + c["projected"], fit + 1
        verdict = "shrink" if fit else "stop"
        why = (f"only {fmt_tok(round(avail))} usable; the first {fit} card(s) fit ({fmt_tok(round(acc * factor))})"
               if fit else f"{fmt_tok(round(avail))} usable is less than the first card needs")
    say(f"verdict: {verdict} - {why}")
    if fit:
        say(f"dispatch (at most {max_agents} subagent(s) at once, {max_opus} on Opus):")
        for i, batch in enumerate(pack_batches(cards[:fit], max_agents, max_opus), 1):
            say(f"  batch {i}: " + ", ".join(f"{c['label']} ({c['model']})" for c in batch))
    learned = learn_context(history)
    for c in cards:
        if c["model"] == "opus" and c["role"] != "reviewer" and c["conf"] in ("high", "medium") and c["projected"] >= 400_000:
            f_opus = context_factor(learned, "opus", c["role"])[0]
            say(f"  suggest: {c['label']} is expensive on Opus at confidence {c['conf']}; a Sonnet builder with the thinker "
                  f"reviewing inline would cost about ~{fmt_tok(round(c['projected'] / f_opus))}")
    say("Prefer Sonnet builders, review inline unless the card needs a fresh context.")
    if a.json:
        batches = [[c["label"] for c in b] for b in pack_batches(cards[:fit], max_agents, max_opus)] if fit else []
        print(json.dumps({
            "verdict": verdict, "why": why, "fit": fit, "total": total, "need": round(need),
            "caps": [{"name": n, "cap": cap, "used": used, "left": max(0, cap - used)} for n, cap, used in caps],
            "cards": [{"label": c["label"], "model": c["model"], "projected": c["projected"], "p75": c["p75"],
                       "confidence": c["conf"], "how": c["how"]} for c in cards],
            "batches": batches,
        }))


class Parser(argparse.ArgumentParser):
    def parse_args(self, args=None, namespace=None):
        a = super().parse_args(args, namespace)
        if getattr(a, "cmd", None) == "tasks" and a.json and not a.dry_run:
            self.error("tasks --json is read-only and needs --dry-run")
        return a


def build_parser():
    p = Parser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
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
    t.add_argument("--no-legacy", action="store_true",
                   help="skip the deprecated specs/*/tokens.md files and roll-up (docs/product/token-usage.md)")
    t.add_argument("--dry-run", action="store_true", help="read everything, write nothing, print the attribution")
    t.add_argument("--json", action="store_true",
                   help="with --dry-run: print each card's attribution (start, end, used) as one JSON object")
    t.set_defaults(fn=cmd_tasks)
    e = sub.add_parser("estimate", help="the projection for a planned card, from history or the starting table")
    g = e.add_mutually_exclusive_group()
    g.add_argument("--role", choices=sorted(ROLE_MODEL), help="agent-roles.md's default model for the role")
    g.add_argument("--model", default="sonnet", help="a model name or family (default sonnet)")
    e.add_argument("--effort", choices=sorted(EFFORT_RANK), default="medium")
    e.add_argument("--layer", help="backend, frontend, qa or general")
    e.add_argument("--tags", help="the card's tags, e.g. 'extend, backend, protocol, rust, tokio, known-pattern, integration, M'")
    e.add_argument("--tasks", type=int, help="how many tasks the card holds (S up to 3, M up to 6, L above)")
    e.add_argument("--history", action="append", help="a history file (default: this repo's and the local index)")
    e.add_argument("--label", help="deprecated: a card label in the roll-up (--rollup), averaged")
    e.add_argument("--rollup", default=DEFAULT_ROLLUP, help="deprecated fallback when there is no history")
    e.add_argument("--json", action="store_true")
    e.set_defaults(fn=cmd_estimate)
    i = sub.add_parser("index", help="rebuild ~/.ai-gent/token-history.jsonl from the repos' committed histories")
    i.add_argument("--repo", action="append", help="a repo dir or history file (repeatable)")
    i.add_argument("--scan", action="append", default=[], help="a folder whose subfolders are repos (e.g. ~/codes)")
    i.add_argument("--index", help="default: ~/.ai-gent/token-history.jsonl ($AI_GENT_HOME)")
    i.set_defaults(fn=cmd_index)
    sd = sub.add_parser("seed", help="seed the local index from past sessions (read-only on sessions and repos)")
    sd.add_argument("--project", action="append", help="a ~/.claude*/projects slug, repeatable (default: the owner's five)")
    sd.add_argument("--codes-dir", default="~/codes", help="where the repos live, to read their specs")
    sd.add_argument("--index", help="default: ~/.ai-gent/token-history.jsonl ($AI_GENT_HOME)")
    sd.add_argument("--dry-run", action="store_true")
    sd.set_defaults(fn=cmd_seed)
    bd = sub.add_parser("budget", help="proceed, shrink or stop: a wave's projections against .ai-gent/budget.yaml")
    bd.add_argument("--cards", help="card ids of the wave, comma separated (looked up in specs/*/cards.md)")
    bd.add_argument("--projected", help="or projections by hand: [model:]400k,[model:]300k")
    bd.add_argument("--specs-dir", default="specs")
    bd.add_argument("--budget", default=os.path.join(".ai-gent", "budget.yaml"))
    bd.add_argument("--project", help="the session slug (default: this directory's)")
    bd.add_argument("--session", action="append", help="the session whose usage counts (default: the newest)")
    bd.add_argument("--history", action="append")
    bd.add_argument("--json", action="store_true", help="print the verdict, the caps, the cards and the batches as one JSON object (the Claude Code token status reads it)")
    bd.set_defaults(fn=cmd_budget)
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
