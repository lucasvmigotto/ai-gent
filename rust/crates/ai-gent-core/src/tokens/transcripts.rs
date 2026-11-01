//! Reading Claude Code's session transcripts: discovery, per-message usage deduplicated by message
//! id, subagent rows with their description, prompt and edit size.
use super::pyjson::{self, J};
use super::util::{OMap, expanduser, glob_children, head, home, is_word, task_ids, to_utc};
use std::path::{Path, PathBuf};

/// The four usage counters of a message.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Usage {
    /// Uncached input.
    pub input: i64,
    /// Cache writes.
    pub cache_w: i64,
    /// Cache reads (reference only).
    pub cache_r: i64,
    /// Output.
    pub output: i64,
}

impl Usage {
    /// Uncached input + cache writes + output.
    #[must_use]
    pub fn bill(&self) -> i64 {
        self.input + self.cache_w + self.output
    }

    fn from_j(u: &J) -> Usage {
        let f = |k: &str| u.get(k).and_then(J::as_i64).unwrap_or(0);
        Usage {
            input: f("input_tokens"),
            cache_w: f("cache_creation_input_tokens"),
            cache_r: f("cache_read_input_tokens"),
            output: f("output_tokens"),
        }
    }
}

/// One assistant message: UTC timestamp (`""` when absent), model, usage.
#[derive(Debug, Clone)]
pub struct Msg {
    /// `YYYY-MM-DDTHH:MM:SS` in UTC, or empty.
    pub ts: String,
    /// The model id.
    pub model: String,
    /// The usage.
    pub usage: Usage,
}

/// Message id to message, in first-seen order.
pub type MsgMap = OMap<Msg>;

/// What `collect` also gathers for a subagent.
#[derive(Debug, Default, Clone)]
pub struct Extra {
    /// The agent's first user message (8000 chars at most).
    pub prompt: Option<String>,
    /// tool_use id to (file, lines written), in first-seen order.
    pub edits: OMap<(String, i64)>,
    /// UTC timestamps of `<synthetic>` messages (a limit or error stop).
    pub stops: Vec<String>,
}

fn text_of(content: Option<&J>) -> String {
    match content {
        Some(J::Str(s)) => s.clone(),
        Some(J::Arr(a)) => a
            .iter()
            .filter(|b| b.get("type").and_then(J::as_str) == Some("text"))
            .map(|b| b.get("text").and_then(J::as_str).unwrap_or(""))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

fn str_field<'a>(o: &'a J, k: &str) -> Option<&'a str> {
    o.get(k).and_then(J::as_str).filter(|s| !s.is_empty())
}

/// Read one transcript: keeps the most complete record per message id. Bad lines are skipped.
pub fn collect(path: &Path, msgs: &mut MsgMap, mut extra: Option<&mut Extra>) {
    let Ok(bytes) = std::fs::read(path) else {
        return;
    };
    for raw in bytes.split(|&b| b == b'\n') {
        let Ok(line) = std::str::from_utf8(raw) else {
            continue;
        };
        let Some(rec) = pyjson::parse(line.trim()) else {
            continue;
        };
        let kind = rec.get("type").and_then(J::as_str);
        if let Some(ex) = extra.as_deref_mut() {
            if kind == Some("user") && ex.prompt.is_none() {
                let content = rec.get("message").and_then(|m| m.get("content"));
                ex.prompt = Some(head(&text_of(content), 8000));
            }
        }
        if kind != Some("assistant") {
            continue;
        }
        let Some(m) = rec.get("message").filter(|m| matches!(m, J::Obj(_))) else {
            continue;
        };
        let model = m.get("model").and_then(J::as_str);
        if let Some(ex) = extra.as_deref_mut() {
            if let Some(J::Arr(blocks)) = m.get("content") {
                for b in blocks {
                    let name = b.get("name").and_then(J::as_str).unwrap_or("");
                    if b.get("type").and_then(J::as_str) != Some("tool_use")
                        || !["Edit", "Write", "MultiEdit", "NotebookEdit"].contains(&name)
                    {
                        continue;
                    }
                    let empty = J::Obj(Vec::new());
                    let inp = b
                        .get("input")
                        .filter(|i| matches!(i, J::Obj(_)))
                        .unwrap_or(&empty);
                    let mut body = ["content", "new_string", "new_source"]
                        .iter()
                        .find_map(|k| str_field(inp, k))
                        .unwrap_or("")
                        .to_string();
                    if name == "MultiEdit" {
                        if let Some(J::Arr(es)) = inp.get("edits") {
                            body = es
                                .iter()
                                .map(|e| e.get("new_string").and_then(J::as_str).unwrap_or(""))
                                .collect::<Vec<_>>()
                                .join("\n");
                        } else {
                            body.clear();
                        }
                    }
                    let file = str_field(inp, "file_path")
                        .or_else(|| str_field(inp, "notebook_path"))
                        .unwrap_or("?");
                    let lines = if body.is_empty() {
                        0
                    } else {
                        body.matches('\n').count() as i64 + 1
                    };
                    let id = b.get("id").and_then(J::as_str).unwrap_or("\u{0}none");
                    ex.edits.set(id, (file.to_string(), lines));
                }
            }
            if model == Some("<synthetic>") {
                if let Some(ts) = str_field(&rec, "timestamp") {
                    if let Some(t) = to_utc(ts) {
                        ex.stops.push(t);
                    }
                }
            }
        }
        let Some(u) = m.get("usage").filter(|u| u.truthy()) else {
            continue;
        };
        let Some(id) = str_field(m, "id") else {
            continue;
        };
        if model == Some("<synthetic>") {
            continue;
        }
        let usage = Usage::from_j(u);
        if msgs
            .get(id)
            .is_none_or(|prev| usage.output >= prev.usage.output)
        {
            let ts = rec
                .get("timestamp")
                .and_then(J::as_str)
                .filter(|t| !t.is_empty());
            msgs.set(
                id,
                Msg {
                    ts: ts.and_then(to_utc).unwrap_or_default(),
                    model: model.unwrap_or("?").to_string(),
                    usage,
                },
            );
        }
    }
}

/// The session ids found in the roots: `<id>.jsonl` files and `<id>/subagents` folders.
#[must_use]
pub fn discover_sessions(roots: &[PathBuf]) -> Vec<String> {
    let mut sids = std::collections::BTreeSet::new();
    for root in roots {
        for p in glob_children(root) {
            let name = p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            if let Some(stem) = name.strip_suffix(".jsonl") {
                if !stem.is_empty() {
                    sids.insert(stem.to_string());
                }
            }
            if p.join("subagents").exists() {
                sids.insert(name);
            }
        }
    }
    sids.into_iter().collect()
}

/// A short grouping label from a subagent's description: its leading task id(s) or a short code
/// before a colon or dash, else a truncated fallback.
#[must_use]
pub fn label_of(desc: &str) -> String {
    let desc = desc.trim();
    let c: Vec<char> = desc.chars().collect();
    let ids = task_ids(desc);
    let leads_with_id =
        c.first() == Some(&'T') && c.iter().skip(1).take_while(|x| x.is_ascii_digit()).count() >= 3;
    if !ids.is_empty() && leads_with_id {
        return ids.join(", ");
    }
    if c.first().is_some_and(char::is_ascii_alphabetic) {
        for n in 1..=28usize {
            let Some(&ch) = c.get(n) else { break };
            if !(ch.is_ascii_alphanumeric() || ch == ' ' || ch == '/' || ch == '+') {
                break;
            }
            let mut p = n + 1;
            while p < c.len() && c[p].is_whitespace() {
                p += 1;
            }
            if matches!(c.get(p), Some(':' | '-')) {
                return c[..=n].iter().collect::<String>().trim().to_string();
            }
        }
    }
    let mut out = head(desc, 24);
    if c.len() > 24 {
        out.push('…');
    }
    out
}

/// opus, sonnet or haiku when the model id names one, else `other`.
#[must_use]
pub fn family(model: &str) -> String {
    let m = model.to_lowercase();
    ["opus", "sonnet", "haiku"]
        .iter()
        .find(|x| m.contains(**x))
        .unwrap_or(&"other")
        .to_string()
}

/// The repository name behind a `~/.claude*/projects/` slug.
#[must_use]
pub fn project_name(slug: &str) -> String {
    if let Ok(cwd) = std::env::current_dir() {
        let cwd = cwd.to_string_lossy().into_owned();
        if slug_of(&cwd) == slug {
            return cwd.rsplit(['/', '\\']).next().unwrap_or("").to_string();
        }
    }
    for marker in ["-codes-", "-projects-", "-src-", "-repos-", "-work-"] {
        if let Some((_, rest)) = slug.split_once(marker) {
            return rest.to_string();
        }
    }
    let last = slug.rsplit('-').next().unwrap_or("");
    if last.is_empty() {
        slug.to_string()
    } else {
        last.to_string()
    }
}

/// `re.sub(r"[^A-Za-z0-9-]", "-", path)`: the directory name Claude Code gives a cwd.
#[must_use]
pub fn slug_of(path: &str) -> String {
    path.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

/// Totals over messages.
#[derive(Debug, Clone, Default)]
pub struct Summary {
    /// Uncached input.
    pub input: i64,
    /// Cache writes.
    pub cache_w: i64,
    /// Cache reads.
    pub cache_r: i64,
    /// Output.
    pub output: i64,
    /// All four.
    pub total: i64,
    /// Input + cache writes + output.
    pub billable: i64,
    /// Message count.
    pub calls: i64,
    /// Models by use, comma separated.
    pub models: String,
    /// Earliest timestamp.
    pub start: String,
    /// Latest timestamp.
    pub end: String,
}

/// Count of distinct keys in first-seen order.
pub fn count_in_order<'a>(items: impl Iterator<Item = &'a str>) -> Vec<(String, i64)> {
    let mut v: Vec<(String, i64)> = Vec::new();
    for x in items {
        match v.iter_mut().find(|(k, _)| k == x) {
            Some(e) => e.1 += 1,
            None => v.push((x.to_string(), 1)),
        }
    }
    v
}

/// The key with the highest count; the first one wins a tie (`max(d, key=d.get)`).
#[must_use]
pub fn max_count(v: &[(String, i64)]) -> Option<&str> {
    let mut best: Option<&(String, i64)> = None;
    for e in v {
        if best.is_none_or(|b| e.1 > b.1) {
            best = Some(e);
        }
    }
    best.map(|b| b.0.as_str())
}

/// Sum a thread's messages.
#[must_use]
pub fn summarize(msgs: &[&Msg]) -> Summary {
    let mut s = Summary::default();
    for m in msgs {
        s.input += m.usage.input;
        s.cache_w += m.usage.cache_w;
        s.cache_r += m.usage.cache_r;
        s.output += m.usage.output;
    }
    s.total = s.input + s.cache_w + s.cache_r + s.output;
    s.billable = s.input + s.cache_w + s.output;
    s.calls = msgs.len() as i64;
    let mut models = count_in_order(msgs.iter().map(|m| m.model.as_str()));
    models.sort_by_key(|(_, n)| std::cmp::Reverse(*n)); // stable: ties keep first-seen order
    s.models = if models.is_empty() {
        "?".into()
    } else {
        models
            .iter()
            .map(|(m, _)| m.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };
    let ts: Vec<&str> = msgs
        .iter()
        .map(|m| m.ts.as_str())
        .filter(|t| !t.is_empty())
        .collect();
    s.start = ts
        .iter()
        .min()
        .map(|t| (*t).to_string())
        .unwrap_or_default();
    s.end = ts
        .iter()
        .max()
        .map(|t| (*t).to_string())
        .unwrap_or_default();
    s
}

/// One agent: a subagent (with an id) or a session's main thread.
#[derive(Debug, Clone, Default)]
pub struct Row {
    /// Resumes after a limit stop.
    pub resumed: i64,
    /// What the resumes cost.
    pub resume_cost: i64,
    /// `subagent a1b2c3d` or `coordinator 1a2b3c4d (main thread)`.
    pub who: String,
    /// The agent id (empty for coordinators).
    pub aid: String,
    /// The session the agent ran in.
    pub sid: String,
    /// The dispatch description.
    pub desc: String,
    /// The grouping label.
    pub label: String,
    /// The first user message.
    pub prompt: String,
    /// The agent's messages, first-seen order.
    pub msgs: Vec<Msg>,
    /// opus, sonnet, haiku or other.
    pub family: String,
    /// Files written.
    pub files: Vec<String>,
    /// Lines written.
    pub lines: i64,
    /// Totals.
    pub s: Summary,
}

/// Everything read for a set of sessions.
#[derive(Debug, Default)]
pub struct Gathered {
    /// One row per session main thread that has messages.
    pub coords: Vec<Row>,
    /// One row per subagent, by start time.
    pub rows: Vec<Row>,
    /// (who, timestamp, model, usage) for stage windows.
    pub tagged: Vec<(String, String, String, Usage)>,
    /// Session id to its main-thread messages.
    pub main_msgs: Vec<(String, MsgMap)>,
}

/// (resumes, their cost): a stop followed by more work is a resume; its cost is the first message
/// after it, which re-reads the agent's context.
#[must_use]
pub fn resume_events(stops: &[String], msgs: &[Msg]) -> (i64, i64) {
    let mut later: Vec<(&str, i64)> = msgs
        .iter()
        .filter(|m| !m.ts.is_empty())
        .map(|m| (m.ts.as_str(), m.usage.bill()))
        .collect();
    later.sort_unstable();
    let uniq: std::collections::BTreeSet<&String> = stops.iter().collect();
    let (mut n, mut cost) = (0, 0);
    for stop in uniq {
        if let Some((_, b)) = later.iter().find(|(t, _)| *t > stop.as_str()) {
            n += 1;
            cost += b;
        }
    }
    (n, cost)
}

/// Read the sessions of the given roots.
#[must_use]
pub fn gather(roots: &[PathBuf], sessions: &[String]) -> Gathered {
    let mut main: Vec<(String, MsgMap)> = Vec::new();
    let mut agents: OMap<MsgMap> = OMap::new();
    let mut meta: OMap<J> = OMap::new();
    let mut extras: OMap<Extra> = OMap::new();
    let mut agent_sid: OMap<String> = OMap::new();
    for root in roots {
        for sid in sessions {
            let m = root.join(format!("{sid}.jsonl"));
            if m.exists() {
                let pos = if let Some(p) = main.iter().position(|(s, _)| s == sid) {
                    p
                } else {
                    main.push((sid.clone(), MsgMap::new()));
                    main.len() - 1
                };
                collect(&m, &mut main[pos].1, None);
            }
            let sub = root.join(sid).join("subagents");
            let mut files: Vec<PathBuf> = std::fs::read_dir(&sub)
                .map(|rd| rd.filter_map(Result::ok).map(|e| e.path()).collect())
                .unwrap_or_default();
            files.sort();
            for f in files {
                let name = f
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let Some(aid) = name
                    .strip_prefix("agent-")
                    .and_then(|n| n.strip_suffix(".jsonl"))
                else {
                    continue;
                };
                collect(
                    &f,
                    agents.entry(aid, MsgMap::new),
                    Some(extras.entry(aid, Extra::default)),
                );
                if !agent_sid.contains_key(aid) {
                    agent_sid.set(aid, sid.clone());
                }
                let mf = sub.join(format!("agent-{aid}.meta.json"));
                if mf.exists() && !meta.contains_key(aid) {
                    if let Some(j) = std::fs::read_to_string(&mf)
                        .ok()
                        .and_then(|t| pyjson::parse(&t))
                    {
                        meta.set(aid, j);
                    }
                }
            }
        }
    }
    let mut g = Gathered::default();
    for (sid, msgs) in &main {
        for m in msgs.values() {
            g.tagged.push((
                format!("coordinator {}", head(sid, 8)),
                m.ts.clone(),
                m.model.clone(),
                m.usage,
            ));
        }
    }
    for (aid, msgs) in agents.iter() {
        if msgs.is_empty() {
            continue;
        }
        let desc = meta
            .get(aid)
            .and_then(|m| m.get("description"))
            .and_then(J::as_str)
            .filter(|d| !d.is_empty())
            .unwrap_or("(no description)")
            .to_string();
        let mlist: Vec<Msg> = msgs.values().cloned().collect();
        let refs: Vec<&Msg> = mlist.iter().collect();
        let s = summarize(&refs);
        let fams = count_in_order(
            mlist
                .iter()
                .map(|m| family(&m.model))
                .collect::<Vec<_>>()
                .iter()
                .map(String::as_str),
        );
        let ex = extras.get(aid).cloned().unwrap_or_default();
        let mut files: Vec<String> = ex.edits.values().map(|(p, _)| p.clone()).collect();
        files.sort();
        files.dedup();
        let (resumed, resume_cost) = resume_events(&ex.stops, &mlist);
        for m in &mlist {
            g.tagged
                .push((desc.clone(), m.ts.clone(), m.model.clone(), m.usage));
        }
        g.rows.push(Row {
            resumed,
            resume_cost,
            who: format!("subagent {}", head(aid, 7)),
            aid: aid.clone(),
            sid: agent_sid.get(aid).cloned().unwrap_or_default(),
            label: label_of(&desc),
            desc,
            prompt: ex.prompt.clone().unwrap_or_default(),
            msgs: mlist,
            family: max_count(&fams).unwrap_or("other").to_string(),
            files,
            lines: ex.edits.values().map(|(_, n)| n).sum(),
            s,
        });
    }
    g.rows.sort_by(|a, b| a.s.start.cmp(&b.s.start));
    for sid in sessions {
        let Some((_, msgs)) = main.iter().find(|(s, _)| s == sid) else {
            continue;
        };
        if msgs.is_empty() {
            continue;
        }
        let refs: Vec<&Msg> = msgs.values().collect();
        g.coords.push(Row {
            who: format!("coordinator {} (main thread)", head(sid, 8)),
            desc: "Planning, interviews, reconciliation, reviews, merges, CI follow-up".into(),
            label: "Coordination".into(),
            s: summarize(&refs),
            ..Row::default()
        });
    }
    g.main_msgs = main;
    g
}

/// The `~/.claude*/projects/<slug>` folders that exist, sorted.
#[must_use]
pub fn project_roots(slug: &str) -> Vec<PathBuf> {
    let mut v = Vec::new();
    for d in claude_homes() {
        let p = d.join("projects").join(slug);
        if p.exists() {
            v.push(p);
        }
    }
    v.sort();
    v
}

/// The `~/.claude*` entries (any file or folder whose name starts with `.claude`), sorted.
#[must_use]
pub fn claude_homes() -> Vec<PathBuf> {
    let Some(h) = home() else { return Vec::new() };
    let mut v: Vec<PathBuf> = std::fs::read_dir(&h)
        .map(|rd| {
            rd.filter_map(Result::ok)
                .filter(|e| e.file_name().to_string_lossy().starts_with(".claude"))
                .map(|e| e.path())
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

/// Config dirs to scan for usage: `$CLAUDE_CONFIG_DIR`, else every `~/.claude*`.
#[must_use]
pub fn config_dirs() -> Vec<PathBuf> {
    match std::env::var("CLAUDE_CONFIG_DIR")
        .ok()
        .filter(|v| !v.is_empty())
    {
        Some(e) => vec![PathBuf::from(expanduser(&e))],
        None => claude_homes(),
    }
}

/// `(count, cache-write tokens)` of the calls after an idle gap over `gap` seconds that wrote at
/// least `floor` tokens to the cache: the cache had expired, so the context was rebuilt.
#[must_use]
pub fn idle_rebuilds(msgs: &[&Msg], gap: i64, floor: i64) -> (i64, i64) {
    let mut seq: Vec<(&str, i64)> = msgs
        .iter()
        .filter(|m| !m.ts.is_empty())
        .map(|m| (m.ts.as_str(), m.usage.cache_w))
        .collect();
    seq.sort_unstable();
    let (mut n, mut written) = (0, 0);
    for w in seq.windows(2) {
        let (Some(a), Some(b)) = (
            super::util::parse_iso(w[0].0),
            super::util::parse_iso(w[1].0),
        ) else {
            continue;
        };
        if b - a > gap && w[1].1 >= floor {
            n += 1;
            written += w[1].1;
        }
    }
    (n, written)
}

/// Whether a word character check is needed by callers building patterns.
#[doc(hidden)]
#[must_use]
pub fn _is_word(c: char) -> bool {
    is_word(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_of_prefers_leading_task_ids() {
        assert_eq!(label_of("T001, T002: build widget"), "T001, T002");
        assert_eq!(label_of("C4: Oracle Free and runner placeholder"), "C4");
        assert_eq!(label_of("Survey unrelated repo"), "Survey unrelated repo");
        assert_eq!(
            label_of("A fairly long description that goes on and on"),
            "A fairly long descriptio…"
        );
    }

    #[test]
    fn project_name_comes_from_the_slug_not_a_hardcoded_project() {
        assert_eq!(project_name("-home-lucas-codes-rstory"), "rstory");
        assert_eq!(project_name("-home-x-codes-my-repo"), "my-repo");
        assert_eq!(project_name("-tmp-widgets"), "widgets");
    }

    fn write(p: &Path, text: &str) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    }

    const MSG: &str = r#"{"type": "assistant", "timestamp": "2026-10-20T10:00:00Z", "message": {"id": "m1", "model": "claude-opus-5-5", "usage": {"input_tokens": 1, "cache_creation_input_tokens": 10, "cache_read_input_tokens": 100, "output_tokens": 2}}}"#;

    #[test]
    fn dedup_by_message_id_counts_once_and_discovery() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path().join("proj");
        write(
            &root.join("sess1.jsonl"),
            &format!("{MSG}\n{MSG}\nnot json\n"),
        );
        write(&root.join("sess1/subagents/agent-a1.jsonl"), MSG);
        let mut msgs = MsgMap::new();
        collect(&root.join("sess1.jsonl"), &mut msgs, None);
        assert_eq!(msgs.len(), 1);
        assert_eq!(discover_sessions(std::slice::from_ref(&root)), ["sess1"]);
        let g = gather(&[root], &["sess1".to_string()]);
        assert_eq!(g.rows.len(), 1);
        assert_eq!(g.rows[0].desc, "(no description)");
        assert_eq!(g.coords[0].s.billable, 13);
    }

    #[test]
    fn idle_rebuilds_counts_cache_writes_after_gaps_over_five_minutes() {
        let mk = |ts: &str, cw: i64| Msg {
            ts: ts.into(),
            model: "m".into(),
            usage: Usage {
                input: 1,
                cache_w: cw,
                cache_r: 100,
                output: 2,
            },
        };
        let ms = [
            mk("2026-10-20T10:00:00", 10),
            mk("2026-10-20T10:02:00", 60_000),
            mk("2026-10-20T10:20:00", 60_000),
            mk("2026-10-20T10:21:00", 60_000),
        ];
        let refs: Vec<&Msg> = ms.iter().collect();
        assert_eq!(idle_rebuilds(&refs, 300, 20_000), (1, 60_000));
    }
}
