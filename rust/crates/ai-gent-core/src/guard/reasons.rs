//! Reason strings, held once in `reasons.json` (FR-013).
use super::event::{Decision, Level};
use serde::Deserialize;
use std::sync::OnceLock;

const RAW: &str = include_str!("reasons.json");

/// One reason entry.
#[derive(Debug, Deserialize)]
pub struct Reason {
    /// Stable id, `<guard>.<rule>`.
    pub id: String,
    /// Guard id.
    pub guard: String,
    /// Prefix of the printed reason.
    pub marker: String,
    /// `deny` or `ask`.
    pub decision: String,
    /// The text; `{branch}`, `{tool}` and `{sub}` are placeholders.
    pub text: String,
}

/// Every reason entry.
///
/// # Panics
/// Never: the file is checked by the unit tests.
pub fn all() -> &'static [Reason] {
    static CELL: OnceLock<Vec<Reason>> = OnceLock::new();
    CELL.get_or_init(|| serde_json::from_str(RAW).expect("reasons.json is valid"))
}

/// Build the decision for reason `id`, filling the placeholders from `args` (`name`, `value`).
///
/// # Panics
/// When `id` is not in `reasons.json` (a bug the unit tests catch).
#[must_use]
pub fn decide(id: &'static str, args: &[(&str, &str)]) -> Decision {
    let r = all().iter().find(|r| r.id == id).expect("known reason id");
    let mut text = r.text.clone();
    for (k, v) in args {
        text = text.replace(&format!("{{{k}}}"), v);
    }
    Decision {
        level: if r.decision == "deny" {
            Level::Deny
        } else {
            Level::Ask
        },
        reason: format!("{} {}", r.marker, text),
        rule: id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_prefixed_by_their_guard() {
        let mut seen = std::collections::HashSet::new();
        for r in all() {
            assert!(seen.insert(&r.id), "duplicate {}", r.id);
            assert!(r.id.starts_with(&format!("{}.", r.guard)), "{}", r.id);
            assert!(r.decision == "deny" || r.decision == "ask");
        }
    }

    #[test]
    fn every_marker_matches_its_guard() {
        for r in all() {
            let want = match r.guard.as_str() {
                "git" => "git:workflow guard:",
                "db" => "db guard:",
                "secrets" => "secrets guard:",
                "repo" => "repo guard:",
                "containers" => "containers guard:",
                g => panic!("unknown guard {g}"),
            };
            assert_eq!(r.marker, want, "{}", r.id);
        }
    }

    #[test]
    fn placeholders_are_filled() {
        let d = decide("git.on_main", &[("branch", "main")]);
        assert_eq!(
            d.reason,
            "git:workflow guard: this commits or merges on main; the workflow works on a branch (allow only if the user asked for it)"
        );
    }
}
