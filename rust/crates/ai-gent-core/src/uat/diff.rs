//! Id stability and revisions between two versions of a roadmap (`ai-gent uat diff`, FR-005, FR-009, FR-022).
//!
//! A step id is a promise to the register: marks are keyed by it. So a removed, renumbered or reused id
//! is refused, a changed step must carry a bumped revision (that is what makes old marks stale), and
//! the diff lists what the independent review has to look at again.
use super::model::{Roadmap, Step};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// A reason to refuse the new version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Refusal {
    /// A stable kebab-case code.
    pub code: &'static str,
    /// The step or script.
    pub at: String,
    /// What is wrong and what to do.
    pub message: String,
}

/// What changed, and whether the new version may be published.
#[derive(Debug, Default, Serialize)]
pub struct Diff {
    /// Why the new version is refused; empty when it may be published.
    pub refused: Vec<Refusal>,
    /// Steps added in the new version.
    pub appended: Vec<String>,
    /// Steps newly retired.
    pub retired: Vec<String>,
    /// Steps whose text changed (their revision is bumped, or must be).
    pub changed: Vec<String>,
    /// Changed steps whose claims changed: the independent review looks at these again (FR-009).
    pub review: Vec<String>,
    /// The changelog entry to start from.
    pub changelog: String,
}

/// The fields whose change makes older marks stale.
fn body(
    s: &Step,
) -> (
    &str,
    &str,
    &str,
    &str,
    &str,
    &str,
    &str,
    &Option<super::model::Wait>,
) {
    (
        &s.who,
        &s.validator,
        &s.action,
        &s.expected,
        &s.pre,
        &s.exclusive_note,
        &s.evidence_expected,
        &s.wait,
    )
}

fn index(r: &Roadmap) -> BTreeMap<&str, &Step> {
    r.steps().map(|(_, st)| (st.id.as_str(), st)).collect()
}

/// Compare two versions.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn diff(old: &Roadmap, new: &Roadmap) -> Diff {
    let mut d = Diff::default();
    let (before, after) = (index(old), index(new));
    for s in &old.scripts {
        if !new.scripts.iter().any(|x| x.id == s.id) {
            d.refused.push(Refusal {
                code: "script-removed",
                at: s.id.clone(),
                message: "a script is never removed: retire its steps and keep the script".into(),
            });
        }
    }
    for (id, was) in &before {
        let Some(now) = after.get(id) else {
            d.refused.push(Refusal {
                code: "step-removed",
                at: (*id).to_string(),
                message:
                    "a removed step is retired, not deleted: its marks keep pointing at it (FR-005)"
                        .into(),
            });
            continue;
        };
        if was.retired() && !now.retired() {
            d.refused.push(Refusal {
                code: "id-reused",
                at: (*id).to_string(),
                message: "a retired id is never reused; add a new step with the next free number (FR-005)".into(),
            });
            continue;
        }
        if !was.retired() && now.retired() {
            d.retired.push((*id).to_string());
        }
        if now.retired() {
            continue;
        }
        if body(was) != body(now) {
            d.changed.push((*id).to_string());
            if now.rev <= was.rev {
                d.refused.push(Refusal {
                    code: "revision-not-bumped",
                    at: (*id).to_string(),
                    message: format!(
                        "the text changed, so the revision goes from {} to {}; older marks become stale",
                        was.rev,
                        was.rev + 1
                    ),
                });
            }
            if was.claims != now.claims {
                d.review.push((*id).to_string());
            }
        } else if now.rev != was.rev {
            d.refused.push(Refusal {
                code: "revision-without-change",
                at: (*id).to_string(),
                message: "the revision moved but the text did not; that would make marks stale for nothing".into(),
            });
        } else if was.claims != now.claims {
            d.review.push((*id).to_string());
        }
    }
    for (id, now) in &after {
        if before.contains_key(id) {
            continue;
        }
        d.appended.push((*id).to_string());
        d.review.push((*id).to_string());
        if let Some((old_id, _)) = before.iter().find(|(oid, was)| {
            !after.contains_key(**oid) && was.action == now.action && was.expected == now.expected
        }) {
            d.refused.push(Refusal {
                code: "step-renumbered",
                at: (*id).to_string(),
                message: format!("same text as removed step {old_id}; keep the old id (FR-005)"),
            });
        }
    }
    let changed_anything = !(d.appended.is_empty() && d.retired.is_empty() && d.changed.is_empty());
    if changed_anything && new.version != old.version + 1 {
        d.refused.push(Refusal {
            code: "version-not-bumped",
            at: "version".into(),
            message: format!(
                "the text changed: the version goes from {} to {}",
                old.version,
                old.version + 1
            ),
        });
    }
    if !changed_anything && new.version != old.version {
        d.refused.push(Refusal {
            code: "version-without-change",
            at: "version".into(),
            message: "the version moved but no step changed".into(),
        });
    }
    d.review.sort();
    d.review.dedup();
    d.changelog = changelog(new.version, &d);
    d
}

fn changelog(version: u32, d: &Diff) -> String {
    let mut parts = Vec::new();
    if !d.appended.is_empty() {
        parts.push(format!("added {}", d.appended.join(", ")));
    }
    if !d.changed.is_empty() {
        parts.push(format!("rewrote {}", d.changed.join(", ")));
    }
    if !d.retired.is_empty() {
        parts.push(format!("retired {}", d.retired.join(", ")));
    }
    if parts.is_empty() {
        return String::new();
    }
    format!("v{version}: {}", parts.join("; "))
}

/// The diff as text.
#[must_use]
pub fn text(d: &Diff) -> String {
    let mut out = String::new();
    for r in &d.refused {
        let _ = writeln!(out, "refused {:<24} {:<10} {}", r.code, r.at, r.message);
    }
    for (name, list) in [
        ("appended", &d.appended),
        ("changed", &d.changed),
        ("retired", &d.retired),
        ("review again", &d.review),
    ] {
        if !list.is_empty() {
            let _ = writeln!(out, "{name}: {}", list.join(", "));
        }
    }
    if !d.changelog.is_empty() {
        let _ = writeln!(out, "changelog draft: {}", d.changelog);
    }
    if d.refused.is_empty() && d.changelog.is_empty() {
        out.push_str("no change\n");
    }
    out
}
