//! The consolidated report, `.ai-gent/uat/<slug>/report.md` (`ai-gent uat report`, FR-060, FR-079, FR-095,
//! FR-099; Decision D2: local, never committed). A file, not a chat message, so tables are fine.
use super::digest::{Snapshot, View, stamp};
use super::model::Roadmap;
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// The session records the report quotes as they are.
#[derive(Debug, Default)]
pub struct Records<'a> {
    /// `triage.md`.
    pub triage: Option<&'a str>,
    /// `decisions.md`.
    pub decisions: Option<&'a str>,
    /// `preflight.md`.
    pub preflight: Option<&'a str>,
}

fn cell(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace('|', "\\|")
}

/// Build the report.
#[must_use]
#[allow(clippy::too_many_lines, clippy::many_single_char_names)]
pub fn report(r: &Roadmap, s: &Snapshot, rec: &Records<'_>) -> String {
    let v = View::new(r, s);
    let mut o = String::new();
    let _ = writeln!(o, "# {} — consolidated report\n", r.title);
    let _ = writeln!(o, "- Roadmap version: {}", r.version);
    let _ = writeln!(
        o,
        "- Environment: {} ({})",
        r.environment.name, r.environment.class
    );
    let _ = writeln!(o, "- Last real check: {}", stamp(&s.at));
    let _ = writeln!(o, "- Validators by assigned role:");
    for role in &r.roles {
        let names: Vec<&str> = r
            .validators
            .iter()
            .filter(|x| x.assigned_role == role.key)
            .map(|x| x.label.as_str())
            .collect();
        let _ = writeln!(
            o,
            "  - {}: {}",
            role.label,
            if names.is_empty() {
                "nobody".to_string()
            } else {
                names.join(", ")
            }
        );
    }

    let _ = writeln!(o, "\n## Results per script\n");
    let _ = writeln!(
        o,
        "| Script | Steps | OK | NOK | Unmarked | Stale | Marked by |\n|---|---|---|---|---|---|---|"
    );
    for p in v.progress() {
        let _ = writeln!(
            o,
            "| {} {} | {} | {} | {} | {} | {} | {} |",
            p.id.to_uppercase(),
            cell(&p.title),
            p.steps,
            p.ok,
            p.nok,
            p.unmarked,
            p.stale,
            p.markers
        );
    }

    let _ = writeln!(o, "\n## Coverage by role\n");
    let _ = writeln!(
        o,
        "| Role | Steps covered by the role | Steps targeting it |\n|---|---|---|"
    );
    for (label, done, total) in v.coverage() {
        let _ = writeln!(o, "| {} | {done} | {total} |", cell(&label));
    }
    let gap = v.gap_role();
    let _ = writeln!(
        o,
        "\nSteps not covered by their role: {}",
        if gap.is_empty() {
            "none".into()
        } else {
            gap.join(", ")
        }
    );
    let left = v.gap_left();
    let _ = writeln!(
        o,
        "Steps marked only by people who left: {}",
        if left.is_empty() {
            "none".into()
        } else {
            left.join(", ")
        }
    );
    let un = v.unassigned();
    let _ = writeln!(
        o,
        "Accounts with no assigned role: {}",
        if un.is_empty() {
            "none".into()
        } else {
            un.join(", ")
        }
    );

    let noks = v.noks();
    let _ = writeln!(o, "\n## NOKs by class\n");
    if noks.is_empty() {
        let _ = writeln!(o, "None.");
    } else {
        let mut by: BTreeMap<String, Vec<&super::digest::Nok>> = BTreeMap::new();
        for (n, _) in &noks {
            let k = if n.class.is_empty() {
                "not triaged".to_string()
            } else {
                n.class.clone()
            };
            by.entry(k).or_default().push(n);
        }
        for (class, list) in by {
            let _ = writeln!(o, "### {class}\n");
            for n in list {
                let st = if n.status.is_empty() {
                    "open"
                } else {
                    &n.status
                };
                let _ = writeln!(
                    o,
                    "- {} by {} ({}): \"{}\" — status: {st}",
                    n.step,
                    n.who,
                    stamp(&n.at),
                    cell(&n.obs)
                );
            }
            o.push('\n');
        }
    }

    let _ = writeln!(o, "## Improvements\n");
    let ideas = v.ideas();
    if ideas.is_empty() {
        let _ = writeln!(o, "None.");
    }
    for (i, _, _) in ideas {
        let _ = writeln!(o, "- {} by {}: \"{}\"", i.step, i.who, cell(&i.txt));
    }

    let _ = writeln!(o, "\n## Role proposals and decisions\n");
    if s.proposals.is_empty() {
        let _ = writeln!(o, "None.");
    }
    for (uid, p) in &s.proposals {
        let _ = writeln!(
            o,
            "- {}: proposed {} — {} (\"{}\")",
            v.label(uid),
            p.role,
            p.decision,
            cell(&p.note)
        );
    }

    let _ = writeln!(o, "\n## Stale marks and retired steps\n");
    let stale = v.stale_steps();
    let _ = writeln!(
        o,
        "Stale marks on: {}",
        if stale.is_empty() {
            "none".into()
        } else {
            stale.join(", ")
        }
    );
    let retired: Vec<&str> = r
        .steps()
        .filter(|(_, st)| st.retired())
        .map(|(_, st)| st.id.as_str())
        .collect();
    let _ = writeln!(
        o,
        "Retired steps: {}",
        if retired.is_empty() {
            "none".into()
        } else {
            retired.join(", ")
        }
    );

    let _ = writeln!(o, "\n## Write checks\n");
    let missing = v.write_checks_missing();
    let _ = writeln!(
        o,
        "Validators with no write check: {}",
        if missing.is_empty() {
            "none".into()
        } else {
            missing.join(", ")
        }
    );

    if let Some(t) = rec.triage {
        let _ = writeln!(o, "\n## Triage log\n\n{}", t.trim_end());
    }
    if let Some(d) = rec.decisions {
        let _ = writeln!(o, "\n## Decision list\n\n{}", d.trim_end());
    }
    if let Some(p) = rec.preflight {
        let _ = writeln!(o, "\n## Preflight record\n\n{}", p.trim_end());
    }

    let _ = writeln!(o, "\n## Review record\n");
    if r.review.is_empty() {
        let _ = writeln!(o, "No independent review recorded.");
    }
    for x in &r.review {
        let _ = writeln!(
            o,
            "- version {} by {} at {}: {} ({} findings)",
            x.version,
            x.by,
            stamp(&x.at),
            x.result,
            x.findings.len()
        );
    }
    let _ = writeln!(o, "\n## Changelog\n");
    for c in &r.changelog {
        let _ = writeln!(
            o,
            "- version {} ({}): {}",
            c.version,
            stamp(&c.at),
            cell(&c.summary)
        );
    }

    if r.mode == "B" {
        let _ = writeln!(o, "\n## Scenario traceability\n");
        let _ = writeln!(o, "| Scenario | Steps |\n|---|---|");
        let mut map: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for (_, st) in r.active_steps() {
            for t in &st.traces {
                map.entry(t.as_str()).or_default().push(st.id.as_str());
            }
        }
        for (t, ids) in map {
            let _ = writeln!(o, "| {} | {} |", cell(t), ids.join(", "));
        }
        for n in &r.not_testable {
            let _ = writeln!(
                o,
                "| {} | not testable by hand: {} |",
                cell(&n.trace),
                cell(&n.reason)
            );
        }
    }
    o
}
