//! The project-wide roll-up (`report`), its summary, and stage markers (snapshots).
use super::pyjson::{J, dumps, obj, parse};
use super::transcripts::{Msg, MsgMap, Row, Usage, idle_rebuilds};
use super::util::{OMap, ensure_parent, fmt_epoch, head, k, parse_iso, read_text, to_utc};
use std::path::Path;

/// A stage marker: when it landed and what it was.
pub type Snap = J;

fn snap_at(s: &J) -> &str {
    s.get("at").and_then(J::as_str).unwrap_or("")
}

fn snap_label(s: &J) -> &str {
    s.get("label").and_then(J::as_str).unwrap_or("")
}

/// The markers in a snapshots file (empty when it does not exist).
pub fn load_snapshots(path: &Path) -> Result<Vec<Snap>, String> {
    if !path.exists() {
        return Ok(vec![]);
    }
    let text = read_text(path).map_err(|e| format!("{}: {e}", path.display()))?;
    match parse(&text) {
        Some(J::Arr(a)) => Ok(a),
        _ => Err(format!("{}: not a JSON array of markers", path.display())),
    }
}

/// Write markers unique by (at, label) and sorted by `at`; returns what was written.
pub fn save_snapshots(path: &Path, snaps: Vec<Snap>) -> Result<Vec<Snap>, String> {
    let mut by: OMap<Snap> = OMap::new();
    for s in snaps {
        let key = format!("{}\u{0}{}", snap_at(&s), snap_label(&s));
        by.set(&key, s);
    }
    let mut v: Vec<Snap> = by.values().cloned().collect();
    v.sort_by(|a, b| snap_at(a).cmp(snap_at(b)));
    ensure_parent(path).map_err(|e| e.to_string())?;
    std::fs::write(path, dumps(&J::Arr(v.clone()), false, Some(1)))
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(v)
}

/// `Merge branch 'x' into ...` to `x`, else the subject.
#[must_use]
pub fn label_from_subject(subject: &str) -> String {
    if let Some(r) = subject.strip_prefix("Merge branch '") {
        if let Some(end) = r.find('\'') {
            if end > 0 && r[end..].starts_with("' into") {
                return r[..end].to_string();
            }
        }
    }
    subject.to_string()
}

/// Markers from `git log --first-parent` lines (`<iso>|<subject>`).
#[must_use]
pub fn markers_from_log(out: &str) -> Vec<Snap> {
    out.lines()
        .filter_map(|l| l.split_once('|'))
        .filter_map(|(t, s)| {
            to_utc(t).map(|at| {
                obj(vec![
                    ("at", J::s(at)),
                    ("label", J::s(label_from_subject(s))),
                    ("source", J::s("git")),
                ])
            })
        })
        .collect()
}

/// Stage markers from the current repo's first-parent landings, in memory.
#[must_use]
pub fn git_markers(reference: Option<&str>) -> Vec<Snap> {
    let refs: Vec<&str> =
        reference.map_or_else(|| vec!["origin/dev", "dev", "main", "HEAD"], |r| vec![r]);
    for r in refs {
        let out = std::process::Command::new("git")
            .args(["log", "--first-parent", "--reverse", "--format=%cI|%s", r])
            .output();
        if let Ok(o) = out {
            if o.status.success() {
                return markers_from_log(&String::from_utf8_lossy(&o.stdout));
            }
        }
    }
    vec![]
}

/// `YYYY-MM-DD HH:MM`, the form of the "Generated" line.
#[must_use]
pub fn stamp_minutes(epoch: i64) -> String {
    fmt_epoch(epoch)[..16].replace('T', " ")
}

fn short(t: &str) -> String {
    head(t, 16).replace('T', " ")
}

/// The roll-up as lines, and the grand totals (billable, total).
#[allow(clippy::too_many_arguments)]
#[must_use]
pub fn render(
    project: &str,
    sessions: &[String],
    n_roots: usize,
    coords: &[Row],
    rows: &[Row],
    tagged: &[(String, String, String, Usage)],
    snaps: &[Snap],
    top: Option<usize>,
    generated: &str,
) -> (Vec<String>, (i64, i64)) {
    let everyone: Vec<&Row> = rows.iter().chain(coords.iter()).collect();
    let grand = (
        everyone.iter().map(|r| r.s.billable).sum::<i64>(),
        everyone.iter().map(|r| r.s.total).sum::<i64>(),
    );
    let sess = if sessions.is_empty() {
        "none".to_string()
    } else {
        sessions
            .iter()
            .map(|s| format!("`{}`", head(s, 8)))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut l: Vec<String> = vec![
        format!("# Token usage report — {project}"),
        String::new(),
        format!(
            "Generated {generated} from session(s) {sess} ({n_roots} config dirs, {} subagents). API-reported usage, deduplicated by message id; local `<synthetic>` placeholders excluded.",
            rows.len()
        ),
        String::new(),
        "**Columns:** *In* uncached input · *Cache w* cache writes · *Cache r* cache reads · *Out* output · *Billable* = In + Cache w + Out (cache reads cost a small fraction of input, so this is the better cost proxy) · *Total* = all four. Times are UTC.".into(),
        String::new(),
    ];
    if !snaps.is_empty() {
        l.extend([
            "## By stage".to_string(),
            String::new(),
            "Each stage covers the usage after the previous marker up to its own. Markers come from git landings or manual snapshots.".into(),
            String::new(),
            "| # | Stage (ends with) | Window (UTC) | Billable | Total | Main contributors (billable) |".into(),
            "|---|---|---|---|---|---|".into(),
        ]);
        let mut bounds: Vec<(String, String, String)> = snaps
            .iter()
            .enumerate()
            .map(|(i, s)| {
                (
                    if i > 0 {
                        snap_at(&snaps[i - 1]).to_string()
                    } else {
                        String::new()
                    },
                    snap_at(s).to_string(),
                    snap_label(s).to_string(),
                )
            })
            .collect();
        bounds.push((
            snap_at(&snaps[snaps.len() - 1]).to_string(),
            "9999".into(),
            "in progress (since last marker)".into(),
        ));
        let mut n = 0;
        for (lo, hi, label) in bounds {
            let sel: Vec<(&String, &Usage)> = tagged
                .iter()
                .filter(|(_, t, _, _)| {
                    !t.is_empty() && lo.as_str() < t.as_str() && t.as_str() <= hi.as_str()
                })
                .map(|(w, _, _, u)| (w, u))
                .collect();
            if sel.is_empty() {
                continue;
            }
            n += 1;
            let mut per_who: OMap<i64> = OMap::new();
            let (mut tot, mut bill) = (0, 0);
            for (who, u) in sel {
                let b = u.bill();
                tot += b + u.cache_r;
                bill += b;
                *per_who.entry(who, || 0) += b;
            }
            let mut ranked: Vec<(&String, &i64)> = per_who.iter().collect();
            ranked.sort_by_key(|(_, v)| std::cmp::Reverse(**v));
            let contributors = ranked
                .iter()
                .take(3)
                .map(|(w, v)| format!("{w} ({})", k(**v)))
                .collect::<Vec<_>>()
                .join("; ");
            let lo_s = short(&lo);
            let win = format!(
                "{} → {}",
                if lo_s.is_empty() {
                    "start".to_string()
                } else {
                    lo_s
                },
                if hi == "9999" {
                    "now".to_string()
                } else {
                    short(&hi)
                }
            );
            l.push(format!(
                "| {n} | {label} | {win} | {} | {} | {contributors} |",
                k(bill),
                k(tot)
            ));
        }
        l.push(String::new());
    }

    l.extend([
        "## By label".to_string(),
        String::new(),
        "| Label | Agents | Model(s) | Billable | Total |".into(),
        "|---|---|---|---|---|".into(),
    ]);
    struct Lab {
        billable: i64,
        total: i64,
        n: i64,
        models: std::collections::BTreeSet<String>,
    }
    let mut by_label: OMap<Lab> = OMap::new();
    for r in &everyone {
        let p = by_label.entry(&r.label, || Lab {
            billable: 0,
            total: 0,
            n: 0,
            models: std::collections::BTreeSet::new(),
        });
        p.billable += r.s.billable;
        p.total += r.s.total;
        p.n += 1;
        p.models
            .extend(r.s.models.split(',').map(|x| x.trim().to_string()));
    }
    let mut labels: Vec<(&String, &Lab)> = by_label.iter().collect();
    labels.sort_by_key(|(_, p)| std::cmp::Reverse(p.billable));
    let cut = top.unwrap_or(labels.len()).min(labels.len());
    for (name, p) in &labels[..cut] {
        l.push(format!(
            "| {name} | {} | {} | {} | {} |",
            p.n,
            p.models.iter().cloned().collect::<Vec<_>>().join(", "),
            k(p.billable),
            k(p.total)
        ));
    }
    if let Some(t) = top {
        if labels.len() > t {
            l.push(format!(
                "| … {} more not shown (--top {t}) | | | | |",
                labels.len() - t
            ));
        }
    }
    l.push(format!(
        "| **All** | {} | | **{}** | {} |",
        everyone.len(),
        k(grand.0),
        k(grand.1)
    ));
    l.push(String::new());

    l.extend([
        "## By agent".to_string(),
        String::new(),
        "| Started | Who | Label | Task | Model | Calls | Billable | Total |".into(),
        "|---|---|---|---|---|---|---|---|".into(),
    ]);
    let mut by_agent: Vec<&Row> = coords.iter().chain(rows.iter()).collect();
    if top.is_some() {
        by_agent.sort_by_key(|r| std::cmp::Reverse(r.s.billable));
    }
    let cut = top.unwrap_or(by_agent.len()).min(by_agent.len());
    for r in &by_agent[..cut] {
        l.push(format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} |",
            short(&r.s.start),
            r.who,
            r.label,
            r.desc,
            r.s.models,
            r.s.calls,
            k(r.s.billable),
            k(r.s.total)
        ));
    }
    if let Some(t) = top {
        if by_agent.len() > t {
            l.push(format!(
                "| … {} more not shown (--top {t}) | | | | | | | |",
                by_agent.len() - t
            ));
        }
    }
    l.extend([
        String::new(),
        "## Notes".into(),
        String::new(),
        "- A resumed subagent keeps its id, so its row covers every resume after a usage limit.".into(),
        "- Each coordinator row spans one session's main thread across account switches (several config dirs).".into(),
        "- Labels come from each card's own description (its leading task id or short code) — nothing is guessed from project-specific vocabulary.".into(),
        String::new(),
    ]);
    (l, grand)
}

/// A few lines: totals, the top labels, and idle cache rebuilds.
#[must_use]
pub fn summary_lines(
    project: &str,
    sessions: &[String],
    coords: &[Row],
    rows: &[Row],
    main_msgs: &[(String, MsgMap)],
    top: usize,
) -> Vec<String> {
    let everyone: Vec<&Row> = rows.iter().chain(coords.iter()).collect();
    let bill: i64 = everyone.iter().map(|r| r.s.billable).sum();
    let tot: i64 = everyone.iter().map(|r| r.s.total).sum();
    let cw: i64 = everyone.iter().map(|r| r.s.cache_w).sum();
    let cr: i64 = everyone.iter().map(|r| r.s.cache_r).sum();
    let mut l = vec![format!(
        "{project}: {} session(s), {} subagent(s); billable {}, total {} (cache reads {}, cache writes {})",
        sessions.len(),
        rows.len(),
        k(bill),
        k(tot),
        k(cr),
        k(cw)
    )];
    let mut by: OMap<i64> = OMap::new();
    for r in &everyone {
        *by.entry(&r.label, || 0) += r.s.billable;
    }
    let mut ranked: Vec<(&String, &i64)> = by.iter().collect();
    ranked.sort_by_key(|(_, v)| std::cmp::Reverse(**v));
    for (name, v) in ranked.into_iter().take(top) {
        l.push(format!("  {:>9}  {}", k(*v), head(name, 60)));
    }
    let (mut n, mut w) = (0, 0);
    for (_, msgs) in main_msgs {
        let list: Vec<&Msg> = msgs.values().collect();
        let (a, b) = idle_rebuilds(&list, 300, 20_000);
        n += a;
        w += b;
    }
    if n > 0 {
        l.push(format!(
            "coordinator idle rebuilds: {n} gap(s) over 5 min re-wrote {} of cache (wait with a background watcher or checkpoint instead of idling)",
            k(w)
        ));
    }
    l
}

#[doc(hidden)]
#[must_use]
pub fn _epoch(ts: &str) -> Option<i64> {
    parse_iso(ts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_and_markers() {
        assert_eq!(
            label_from_subject("Merge branch 'feat/x' into dev"),
            "feat/x"
        );
        assert_eq!(
            label_from_subject("chore: initial commit"),
            "chore: initial commit"
        );
        let m = markers_from_log(
            "2026-10-19T00:00:00+00:00|chore: initial commit\nbad line\n2026-10-20T00:00:00+00:00|manual one\n",
        );
        assert_eq!(m.len(), 2);
        assert_eq!(snap_at(&m[0]), "2026-10-19T00:00:00");
    }

    #[test]
    fn snapshots_merge_without_duplicates() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("snaps.json");
        let one = obj(vec![
            ("at", J::s("2026-10-20T00:00:00")),
            ("label", J::s("manual one")),
            ("source", J::s("manual")),
        ]);
        save_snapshots(&p, vec![one]).unwrap();
        let mut all = load_snapshots(&p).unwrap();
        all.extend(markers_from_log("2026-10-19T00:00:00+00:00|chore: initial commit\n2026-10-20T00:00:00+00:00|manual one\n"));
        assert_eq!(save_snapshots(&p, all).unwrap().len(), 2);
        assert_eq!(load_snapshots(&p).unwrap().len(), 2);
    }
}
