//! The files `tokens tasks` keeps up to date: the `Tokens:` tag on `tasks.md` lines, the Tokens
//! block of `tracker.md` and the Tokens column of the orchestration ledger. All idempotent.
use super::specs::Feature;
use super::tags::{Tags, format_tags};
use super::util::{OMap, fmt_tok, pct_error, read_text, write_atomic};
use std::path::Path;

/// One card (or task set) with its projection and cost.
#[derive(Debug, Clone)]
pub struct Unit {
    /// Backticked card id, a task key, or none for feature-level work.
    pub label: Option<String>,
    /// Task ids.
    pub tasks: Vec<String>,
    /// Projected tokens.
    pub projected: Option<i64>,
    /// Billable tokens used.
    pub used: Option<i64>,
    /// Cache reads.
    pub cache_read: i64,
    /// The planned tags.
    pub tags: Tags,
}

/// Feature-level work not tied to a card.
#[derive(Debug, Clone)]
pub struct ExtraRow {
    /// Row label.
    pub label: String,
    /// Billable tokens.
    pub used: i64,
    /// Cache reads.
    pub cache_read: i64,
}

/// Remove a trailing `\s*·\s*{tag}[^\n]*$` from a line.
#[must_use]
pub fn strip_tag(line: &str, tag: &str) -> String {
    for (i, _) in line.match_indices('·') {
        let after = line[i + '·'.len_utf8()..].trim_start();
        if after.starts_with(tag) {
            return line[..i].trim_end().to_string();
        }
    }
    line.to_string()
}

fn tag_for(projected: Option<i64>, used: Option<i64>, with_task: Option<&str>) -> String {
    if let Some(t) = with_task {
        return format!(" · Tokens: with {t}");
    }
    let p = projected.map_or_else(|| "—".to_string(), |p| format!("~{}", fmt_tok(Some(p))));
    format!(" · Tokens: projected {p} | used {}", fmt_tok(used))
}

/// Put each unit's tag on its first task line (file order) and `with Txxx` on the others. The tag is
/// replaced, never added twice. Returns how many lines changed.
pub fn write_task_lines(feature: &Feature, units: &[Unit], dry: bool) -> std::io::Result<usize> {
    let path = feature.dir.join("tasks.md");
    if !path.exists() {
        return Ok(0);
    }
    let text = read_text(&path)?;
    let mut lines: Vec<String> = text.split('\n').map(String::from).collect();
    let mut changed = 0;
    for u in units {
        let mut mine: Vec<&String> = u
            .tasks
            .iter()
            .filter(|t| feature.tasks.contains_key(t))
            .collect();
        mine.sort_by_key(|t| feature.tasks.get(t).map_or(0, |x| x.line));
        if mine.is_empty() || (u.projected.is_none() && u.used.is_none()) {
            continue;
        }
        for (i, t) in mine.iter().enumerate() {
            let n = feature.tasks.get(t).map_or(0, |x| x.line);
            let Some(cur) = lines.get(n) else { continue };
            let mut line = strip_tag(cur, "Tokens:").trim_end().to_string();
            if i == 0 && !u.tags.is_empty() {
                line = format!(
                    "{} · Tags: {}",
                    strip_tag(&line, "Tags:").trim_end(),
                    format_tags(&u.tags)
                );
            } else if i > 0 && !u.tags.is_empty() {
                line = strip_tag(&line, "Tags:").trim_end().to_string();
            }
            let new = line
                + &if i == 0 {
                    tag_for(u.projected, u.used, None)
                } else {
                    tag_for(None, None, Some(mine[0]))
                };
            if new != lines[n] {
                lines[n] = new;
                changed += 1;
            }
        }
    }
    if changed > 0 && !dry {
        write_atomic(&path, &lines.join("\n"))?;
    }
    Ok(changed)
}

const TRACKER_START: &str =
    "<!-- tokens:start (token_report.py tasks - regenerated on every run, do not edit) -->";
const TRACKER_END: &str = "<!-- tokens:end -->";

/// The feature's token block in `tracker.md` (created if missing), replaced in place on re-runs.
pub fn write_tracker(
    feature: &Feature,
    units: &[Unit],
    extras: &[ExtraRow],
    coord: f64,
    dry: bool,
) -> std::io::Result<bool> {
    let mut rows = vec![
        "| Card / tasks | Projected | Used | Error | Cache reads (reference) |".to_string(),
        "|---|---|---|---|---|".to_string(),
    ];
    let (mut p_tot, mut u_tot, mut c_tot) = (0i64, 0i64, 0i64);
    for u in units {
        let Some(used) = u.used else { continue };
        rows.push(format!(
            "| {} | {} | {} | {} | {} |",
            u.label.as_deref().unwrap_or(""),
            u.projected
                .map_or_else(|| "—".to_string(), |p| format!("~{}", fmt_tok(Some(p)))),
            fmt_tok(Some(used)),
            pct_error(u.projected, Some(used)),
            fmt_tok(Some(u.cache_read))
        ));
        u_tot += used;
        c_tot += u.cache_read;
        if let Some(p) = u.projected {
            p_tot += p;
        }
    }
    for e in extras {
        rows.push(format!(
            "| {} | — | {} | — | {} |",
            e.label,
            fmt_tok(Some(e.used)),
            fmt_tok(Some(e.cache_read))
        ));
        u_tot += e.used;
        c_tot += e.cache_read;
    }
    let coord_i = super::util::round_i(coord);
    rows.push(format!(
        "| **Subagents** | ~{} | **{}** | {} | {} |",
        fmt_tok(Some(p_tot)),
        fmt_tok(Some(u_tot)),
        pct_error(Some(p_tot), Some(u_tot)),
        fmt_tok(Some(c_tot))
    ));
    rows.push(format!(
        "| Coordinator share | | {} | | |",
        fmt_tok(Some(coord_i))
    ));
    rows.push(format!(
        "| **Feature total** | | **{}** | | |",
        fmt_tok(Some(u_tot + coord_i))
    ));
    let mut parts: Vec<String> = vec![
        TRACKER_START.into(),
        String::new(),
        "## Tokens".into(),
        String::new(),
        "Billable tokens (uncached input + cache writes + output); cache reads are reference only. Projected is the card's estimate before the work, used what its subagents then cost. The coordinator share is the main thread's usage in the windows this feature landed, split by subagent share (`plugins/agents/references/token-tracking.md`).".into(),
        String::new(),
    ];
    parts.extend(rows);
    parts.push(String::new());
    parts.push(TRACKER_END.into());
    let block = parts.join("\n");
    let path = feature.dir.join("tracker.md");
    let text = if path.exists() {
        read_text(&path)?
    } else {
        format!("# Tracker — {}\n", feature.name)
    };
    let new = replace_block(&text, &block)
        .unwrap_or_else(|| format!("{}\n\n{block}\n", text.trim_end_matches('\n')));
    if new != text && !dry {
        write_atomic(&path, &new)?;
    }
    Ok(new != text)
}

/// Replace every `START.*?END` span with `block`; `None` when there is no span.
fn replace_block(text: &str, block: &str) -> Option<String> {
    let mut out = String::new();
    let mut rest = text;
    let mut found = false;
    while let Some(s) = rest.find(TRACKER_START) {
        let after = s + TRACKER_START.len();
        let Some(e) = rest[after..].find(TRACKER_END) else {
            break;
        };
        out.push_str(&rest[..s]);
        out.push_str(block);
        rest = &rest[after + e + TRACKER_END.len()..];
        found = true;
    }
    out.push_str(rest);
    found.then_some(out)
}

/// `~400k / 391k`, `— / 391k`, or none when the unit has neither number.
#[must_use]
pub fn ledger_cell(u: Option<&Unit>) -> Option<String> {
    let u = u?;
    if u.projected.is_none() && u.used.is_none() {
        return None;
    }
    let p = u
        .projected
        .map_or_else(|| "—".to_string(), |p| format!("~{}", fmt_tok(Some(p))));
    Some(format!("{p} / {}", fmt_tok(u.used)))
}

fn cells_of(line: &str) -> Vec<String> {
    line.trim()
        .trim_matches('|')
        .split('|')
        .map(|c| c.trim().to_string())
        .collect()
}

/// Add or refresh a Tokens column (projected / used) on the orchestration ledger's card table.
pub fn write_ledger(path: &Path, by_card: &OMap<Unit>, dry: bool) -> std::io::Result<bool> {
    if !path.exists() {
        return Ok(false);
    }
    let text = read_text(path)?;
    let lines: Vec<&str> = text.split('\n').collect();
    let mut out: Vec<String> = Vec::new();
    let (mut state, mut idx) = (0u8, 0usize); // 0 none, 1 separator next, 2 body
    for ln in &lines {
        if !ln.starts_with('|') {
            state = 0;
            out.push((*ln).to_string());
            continue;
        }
        let mut cells = cells_of(ln);
        match state {
            0 => {
                let mut line = (*ln).to_string();
                if cells.iter().any(|c| c.to_lowercase() == "card") {
                    if !cells.iter().any(|c| c == "Tokens") {
                        cells.push("Tokens".into());
                    }
                    idx = cells.iter().position(|c| c == "Tokens").unwrap_or(0);
                    state = 1;
                    line = format!("| {} |", cells.join(" | "));
                }
                out.push(line);
            }
            1 => {
                out.push(format!(
                    "|{}|",
                    vec!["---"; cells.len().max(idx + 1)].join("|")
                ));
                state = 2;
            }
            _ => {
                let cid = cells
                    .iter()
                    .find(|c| c.starts_with('`') && c.ends_with('`') && c.len() >= 2)
                    .map(|c| c.trim_matches('`').to_string());
                while cells.len() < idx + 1 {
                    cells.push(String::new());
                }
                let cell = cid
                    .as_deref()
                    .and_then(|c| by_card.get(c))
                    .and_then(|u| ledger_cell(Some(u)));
                cells[idx] = cell
                    .or_else(|| Some(cells[idx].clone()).filter(|c| !c.is_empty()))
                    .unwrap_or_else(|| "—".into());
                out.push(format!("| {} |", cells.join(" | ")));
            }
        }
    }
    let new = out.join("\n");
    let changed = new != text;
    if changed && !dry {
        write_atomic(path, &new)?;
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_tag_removes_the_trailing_tag_only() {
        assert_eq!(
            strip_tag(
                "- [ ] T001 x · Effort: high · Tokens: projected ~1k | used 2k",
                "Tokens:"
            ),
            "- [ ] T001 x · Effort: high"
        );
        assert_eq!(
            strip_tag("- [ ] T001 x · Tokens: with T001", "Tokens:"),
            "- [ ] T001 x"
        );
        assert_eq!(
            strip_tag("- [ ] T001 x · Effort: high", "Tokens:"),
            "- [ ] T001 x · Effort: high"
        );
        assert_eq!(strip_tag("a · b  · Tags: x · Tokens: 1", "Tags:"), "a · b");
    }

    #[test]
    fn ledger_cell_prints_what_the_toast_reads() {
        let u = Unit {
            label: None,
            tasks: vec![],
            projected: Some(420_000),
            used: Some(391_000),
            cache_read: 0,
            tags: vec![],
        };
        assert_eq!(ledger_cell(Some(&u)).as_deref(), Some("~420k / 391k"));
        assert_eq!(ledger_cell(None), None);
    }

    #[test]
    fn replace_block_swaps_the_span() {
        let t = format!("# x\n\n{TRACKER_START}\nold\n{TRACKER_END}\n");
        assert_eq!(replace_block(&t, "NEW").unwrap(), "# x\n\nNEW\n");
        assert_eq!(replace_block("nothing", "NEW"), None);
    }
}
