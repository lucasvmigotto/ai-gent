//! The budget: `.ai-gent/budget.yaml`, usage across projects, the pace, and the wave of cards.
use super::attribute::Group;
use super::attribute::layer_of;
use super::estimate::estimate_card;
use super::pyjson::J;
use super::specs::Feature;
use super::transcripts::{Msg, MsgMap, collect, family};
use super::util::{OMap, fmt_epoch, parse_iso, parse_tok, round_i};
use std::path::{Path, PathBuf};

/// Fewer messages in the rate window read as noise, not as a pace.
pub const MIN_RATE_MESSAGES: i64 = 3;
/// Resumes, fix rounds and flake hunts as a share of cost, until history knows.
pub const OPUS_PARALLEL_RISK: f64 = 0.12;

/// `.ai-gent/budget.yaml` as numbers; caps absent mean no cap.
#[must_use]
pub fn parse_budget(path: &Path) -> OMap<i64> {
    let mut cfg: OMap<i64> = OMap::new();
    for (k, v) in [
        ("max_opus", 1),
        ("max_agents", 2),
        ("reserve_pct", 10),
        ("coordinator_overhead_pct", 35),
        ("rate_window_minutes", 30),
    ] {
        cfg.set(k, v);
    }
    let Ok(text) = super::util::read_text(path) else {
        return cfg;
    };
    for line in text.split('\n') {
        let t = line.trim_start();
        let key: String = t
            .chars()
            .take_while(|c| c.is_ascii_lowercase() || *c == '_')
            .collect();
        if key.is_empty() {
            continue;
        }
        let Some(rest) = t[key.len()..].trim_start().strip_prefix(':') else {
            continue;
        };
        let val: String = rest
            .trim_start()
            .chars()
            .take_while(|c| *c != '#' && !c.is_whitespace())
            .collect();
        if let Some(n) = parse_tok(&val).filter(|_| !val.is_empty()) {
            cfg.set(&key, n);
        }
    }
    cfg
}

fn jsonl_files(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = super::util::glob_children(dir)
        .into_iter()
        .filter(|p| p.extension().is_some_and(|e| e == "jsonl") && p.is_file())
        .collect();
    v.sort();
    v
}

/// Every message of the given config dirs' projects from a UTC timestamp on, deduplicated by
/// message id over all of them (an account switch is not counted twice).
#[must_use]
pub fn usage_messages(cutoff_iso: &str, dirs: &[PathBuf]) -> Vec<Msg> {
    let mut msgs = MsgMap::new();
    let floor = parse_iso(cutoff_iso).unwrap_or(0) as f64 - 3600.0;
    let fresh = |f: &Path| {
        std::fs::metadata(f)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .is_some_and(|d| d.as_secs_f64() >= floor)
    };
    for cfg in dirs {
        let projects = super::util::glob_children(&cfg.join("projects"));
        let mut files: Vec<PathBuf> = Vec::new();
        for p in &projects {
            files.extend(jsonl_files(p));
        }
        for p in &projects {
            for sess in super::util::glob_children(p) {
                let sub = sess.join("subagents");
                let mut agents: Vec<PathBuf> = std::fs::read_dir(&sub)
                    .map(|rd| rd.filter_map(Result::ok).map(|e| e.path()).collect())
                    .unwrap_or_default();
                agents.sort();
                files.extend(agents.into_iter().filter(|a| {
                    let n = a
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    n.starts_with("agent-") && n.ends_with(".jsonl")
                }));
            }
        }
        for f in files {
            if fresh(&f) {
                collect(&f, &mut msgs, None);
            }
        }
    }
    msgs.values()
        .filter(|m| !m.ts.is_empty() && m.ts.as_str() >= cutoff_iso)
        .cloned()
        .collect()
}

/// Billable tokens across every project of the given config dirs since a UTC timestamp.
#[must_use]
pub fn usage_since(cutoff_iso: &str, dirs: &[PathBuf]) -> i64 {
    usage_messages(cutoff_iso, dirs)
        .iter()
        .map(|m| m.usage.bill())
        .sum()
}

/// (billable tokens per hour, messages) over the last `minutes` before `now`.
#[must_use]
pub fn usage_rate(msgs: &[Msg], now: i64, minutes: i64) -> (i64, i64) {
    let since = fmt_epoch(now - minutes * 60);
    let used: Vec<i64> = msgs
        .iter()
        .filter(|m| !m.ts.is_empty() && m.ts.as_str() >= since.as_str())
        .map(|m| m.usage.bill())
        .collect();
    (
        round_i(used.iter().sum::<i64>() as f64 * 60.0 / minutes as f64),
        used.len() as i64,
    )
}

/// role to model family: the defaults, overridden by the project's `orchestrate.yaml`.
#[must_use]
pub fn roles_models(path: &Path) -> OMap<String> {
    let mut out: OMap<String> = OMap::new();
    for (r, m) in [
        ("thinker", "opus"),
        ("builder", "sonnet"),
        ("scout", "haiku"),
    ] {
        out.set(r, m.into());
    }
    let Ok(text) = super::util::read_text(path) else {
        return out;
    };
    let lines: Vec<&str> = text.split('\n').collect();
    let mut offset = 0;
    for ln in &lines {
        let chars: Vec<char> = ln.chars().collect();
        // ^\s{2}(\w+):\s*\{
        if chars.len() > 2
            && chars[0].is_whitespace()
            && chars[1].is_whitespace()
            && super::util::is_word(chars[2])
        {
            let mut i = 2;
            while i < chars.len() && super::util::is_word(chars[i]) {
                i += 1;
            }
            let role: String = chars[2..i].iter().collect();
            if chars.get(i) == Some(&':') {
                let tail: String = chars[i + 1..].iter().collect();
                let t = tail.trim_start();
                if let Some(rest) = t.strip_prefix('{') {
                    // the rest of the file up to the first `}`, the last `model:` word in it
                    let all: String = text[offset..].to_string();
                    let start = all.find(rest).unwrap_or(0);
                    let body = &all[start..];
                    let end = body.find('}').unwrap_or(body.len());
                    let inside = &body[..end];
                    let mut found: Option<String> = None;
                    for (pos, _) in inside.match_indices("model:") {
                        let w: String = inside[pos + 6..]
                            .trim_start()
                            .chars()
                            .take_while(|c| super::util::is_word(*c))
                            .collect();
                        if !w.is_empty() {
                            found = Some(w);
                        }
                    }
                    if let Some(m) = found {
                        out.set(&role, family(&m));
                    }
                }
            }
        }
        offset += ln.len() + 1;
    }
    out
}

/// One card of the wave.
#[derive(Debug, Clone)]
pub struct WaveCard {
    /// The card id or the given projection.
    pub label: String,
    /// Model family.
    pub model: String,
    /// Executor role.
    pub role: String,
    /// Projected billable tokens.
    pub projected: i64,
    /// Upper bound.
    pub p75: Option<i64>,
    /// Where the projection came from.
    pub how: String,
    /// high, medium or low.
    pub conf: Option<String>,
}

/// The wave's cards: `--projected [model:]400k,...` first, then `--cards ID,ID`.
#[must_use]
pub fn wave_cards(
    projected: &str,
    cards: &str,
    feats: &OMap<Feature>,
    history: &[J],
    roles: &OMap<String>,
) -> Vec<WaveCard> {
    let mut out = Vec::new();
    for item in projected.split(',') {
        let item = item.trim();
        if item.is_empty() {
            continue;
        }
        let (fam, amt) = item.rsplit_once(':').unwrap_or(("", item));
        out.push(WaveCard {
            label: item.to_string(),
            model: if fam.is_empty() {
                "sonnet".into()
            } else {
                family(fam)
            },
            role: "builder".into(),
            projected: parse_tok(amt).unwrap_or(0),
            p75: None,
            how: "given".into(),
            conf: None,
        });
    }
    for cid in cards.split(',').map(str::trim).filter(|c| !c.is_empty()) {
        let Some(feat) = feats.values().find(|f| f.cards.contains_key(cid)) else {
            out.push(WaveCard {
                label: cid.into(),
                model: "sonnet".into(),
                role: "builder".into(),
                projected: 0,
                p75: None,
                how: "card not found".into(),
                conf: None,
            });
            continue;
        };
        let Some(c) = feat.cards.get(cid) else {
            continue;
        };
        let role = c.executor.clone().unwrap_or_else(|| "builder".into());
        let fam = roles.get(&role).cloned().unwrap_or_else(|| "sonnet".into());
        let g = Group {
            feature: Some(feat.name.clone()),
            card: Some(cid.into()),
            tasks: c.tasks.clone(),
            confidence: "exact",
            rows: vec![],
            billable: 0,
            cache_read: 0,
            calls: 0,
            end: String::new(),
            start: String::new(),
            files: vec![],
            lines: 0,
            model: String::new(),
            model_id: None,
            desc: String::new(),
            events: vec![],
            event_cost: 0,
        };
        let layer = layer_of(&g, feats);
        let effort = c.effort.clone().unwrap_or_else(|| "medium".into());
        let tags = (!c.tags.is_empty()).then(|| c.tags.clone());
        let est = estimate_card(
            history,
            &fam,
            Some(&effort),
            Some(&layer),
            Some(c.tasks.len()),
            tags.as_ref(),
            Some(&role),
        );
        let conf = est.confidence_label.map(String::from);
        if let Some(p) = c.projected {
            out.push(WaveCard {
                label: cid.into(),
                model: fam,
                role,
                projected: p,
                p75: Some(round_i(est.p75)),
                how: "cards.md".into(),
                conf,
            });
        } else {
            out.push(WaveCard {
                label: cid.into(),
                model: fam,
                role,
                projected: round_i(est.median),
                p75: Some(round_i(est.p75)),
                how: format!("estimated, {}", est.basis),
                conf,
            });
        }
    }
    out
}

/// In dependency (given) order: batches of at most `max_agents` subagents, at most `max_opus` on Opus.
#[must_use]
pub fn pack_batches(cards: &[WaveCard], max_agents: usize, max_opus: usize) -> Vec<Vec<&WaveCard>> {
    let mut batches: Vec<Vec<&WaveCard>> = Vec::new();
    let mut cur: Vec<&WaveCard> = Vec::new();
    for c in cards {
        let opus = cur.iter().filter(|x| x.model == "opus").count();
        if !cur.is_empty() && (cur.len() >= max_agents || (c.model == "opus" && opus >= max_opus)) {
            batches.push(std::mem::take(&mut cur));
        }
        cur.push(c);
    }
    if !cur.is_empty() {
        batches.push(cur);
    }
    batches
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::transcripts::Usage;

    #[test]
    fn parse_budget_reads_flat_keys_with_suffixes_and_defaults() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("b.yaml");
        std::fs::write(
            &p,
            "# caps\nsession_billable: 12M  # per session\nweek_billable: 800k\nmax_opus: 2\n",
        )
        .unwrap();
        let cfg = parse_budget(&p);
        let g = |k: &str| cfg.get(k).copied();
        assert_eq!(
            (
                g("session_billable"),
                g("week_billable"),
                g("max_opus"),
                g("max_agents")
            ),
            (Some(12_000_000), Some(800_000), Some(2), Some(2))
        );
        assert_eq!(
            parse_budget(&d.path().join("none")).get("rate_window_minutes"),
            Some(&30)
        );
    }

    fn msgs(items: &[(i64, i64)]) -> Vec<Msg> {
        let now = parse_iso("2026-10-31T12:00:00Z").unwrap();
        items
            .iter()
            .map(|(m, out)| Msg {
                ts: fmt_epoch(now - m * 60),
                model: "m".into(),
                usage: Usage {
                    input: 0,
                    cache_w: 0,
                    cache_r: 1000,
                    output: *out,
                },
            })
            .collect()
    }

    #[test]
    fn usage_rate_counts_only_the_window_and_only_billable_tokens() {
        let now = parse_iso("2026-10-31T12:00:00Z").unwrap();
        let m = msgs(&[(1, 1000), (10, 1000), (20, 1000), (29, 1000), (45, 50_000)]);
        assert_eq!(usage_rate(&m, now, 30), (8000, 4));
        assert_eq!(usage_rate(&msgs(&[(90, 5000)]), now, 30), (0, 0));
        assert_eq!(usage_rate(&[], now, 30), (0, 0));
        let m = msgs(&[(1, 1000), (50, 1000), (55, 1000)]);
        assert_eq!(usage_rate(&m, now, 60), (3000, 3));
        assert_eq!(usage_rate(&m, now, 10), (6000, 1));
        assert_eq!(MIN_RATE_MESSAGES, 3);
    }

    #[test]
    fn batches_respect_the_opus_cap() {
        let c = |m: &str| WaveCard {
            label: m.into(),
            model: m.into(),
            role: "builder".into(),
            projected: 1,
            p75: None,
            how: String::new(),
            conf: None,
        };
        let cards = [c("opus"), c("opus"), c("sonnet")];
        assert_eq!(pack_batches(&cards, 3, 1).len(), 2);
        assert_eq!(pack_batches(&cards, 3, 2).len(), 1);
    }

    #[test]
    fn roles_read_from_orchestrate_yaml() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("o.yaml");
        std::fs::write(
            &p,
            "roles:\n  builder: { model: opus, effort: high }\n  scout: {model: haiku}\n",
        )
        .unwrap();
        let r = roles_models(&p);
        assert_eq!(r.get("builder").map(String::as_str), Some("opus"));
        assert_eq!(r.get("thinker").map(String::as_str), Some("opus"));
    }
}
