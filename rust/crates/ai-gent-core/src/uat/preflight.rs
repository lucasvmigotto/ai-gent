//! Access before and after publishing (`ai-gent uat preflight`, FR-044, FR-092, FR-093, FR-097).
//!
//! `checklist` turns the owner's list into lines the owner confirms; the agent never claims to have
//! seen anyone's sharing level. `verify` compares the published declaration with `data-model.md` and
//! the test reads at `view`, `interact` and `admin`, and says what that proves and what it does not.
use super::model::Roadmap;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// The sharing level that grants `interact` on the register.
pub const LEVEL_NEEDED: &str = "Contributor";

/// One line of the checklist.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Line {
    /// The validator's label.
    pub validator: String,
    /// Their assigned role.
    pub role: String,
    /// The level needed to write marks.
    pub level_needed: &'static str,
    /// `pass`, `fix` or `ask`.
    pub state: &'static str,
    /// What to change in the sharing menu, if anything.
    pub change: String,
    /// Caveats that apply.
    pub caveats: Vec<String>,
    /// The owner's confirmation; always `pending` when the checklist is written.
    pub confirmation: &'static str,
}

/// The checklist.
#[derive(Debug, Serialize)]
pub struct Checklist {
    /// One line per validator.
    pub lines: Vec<Line>,
    /// Warnings that apply to the whole session.
    pub warnings: Vec<String>,
    /// Some line needs the owner to change or say something.
    pub action_needed: bool,
}

/// Build the checklist from the validators and the settings.
#[must_use]
pub fn checklist(r: &Roadmap) -> Checklist {
    let mut lines = Vec::new();
    let mut editors = false;
    for v in &r.validators {
        let role = r
            .roles
            .iter()
            .find(|x| x.key == v.assigned_role)
            .map_or(v.assigned_role.clone(), |x| x.label.clone());
        let level = v.planned_sharing.trim().to_ascii_lowercase();
        let (state, change) = match level.as_str() {
            "contributor" => ("pass", String::new()),
            "editor" => {
                editors = true;
                ("pass", String::new())
            }
            "viewer" | "commenter" => (
                "fix",
                format!(
                    "In the sharing menu, change {} from {} to {LEVEL_NEEDED}.",
                    v.label, level
                ),
            ),
            _ => (
                "ask",
                format!(
                    "Say which level {} will get; they need {LEVEL_NEEDED} to save marks.",
                    v.label
                ),
            ),
        };
        let mut caveats = Vec::new();
        if v.outside_org {
            caveats.push(
                "Outside your organization: switching the page to link sharing can remove their write access."
                    .to_string(),
            );
            caveats.push(
                "They may not be able to upload attachments; they keep text and links.".to_string(),
            );
        }
        if level == "editor" {
            caveats.push("Editors can also republish the page and change its rules.".to_string());
        }
        lines.push(Line {
            validator: v.label.clone(),
            role,
            level_needed: LEVEL_NEEDED,
            state,
            change,
            caveats,
            confirmation: "pending",
        });
    }
    let mut warnings = Vec::new();
    if editors {
        warnings.push(
            "Some validators are shared as Editors: they can republish the page and change its rules. The agent compares the published page before each republish and stops on a difference (FR-097)."
                .to_string(),
        );
    }
    let action_needed = lines.iter().any(|l| l.state != "pass");
    Checklist {
        lines,
        warnings,
        action_needed,
    }
}

/// The checklist as the owner reads it and as `preflight.md` starts.
#[must_use]
pub fn checklist_text(c: &Checklist) -> String {
    let mut o = String::from("Preflight checklist\n");
    for l in &c.lines {
        let _ = writeln!(
            o,
            "- {} ({}): needs {} to save marks — {} — confirmation: {}",
            l.validator,
            l.role,
            l.level_needed,
            match l.state {
                "pass" => "level as you said".to_string(),
                _ => l.change.clone(),
            },
            l.confirmation
        );
        for cv in &l.caveats {
            let _ = writeln!(o, "  - {cv}");
        }
    }
    for w in &c.warnings {
        let _ = writeln!(o, "- {w}");
    }
    o.push_str("I cannot see anyone's sharing level; each line is your word until the person's own write check appears.\n");
    o
}

/// A rule in a capabilities declaration.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Rule {
    /// The path.
    pub path: String,
    /// Minimum level to read, if set.
    pub read: Option<String>,
    /// Minimum level to write, if set.
    pub write: Option<String>,
}

/// The `db` part of a declaration.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Db {
    /// The rules.
    pub rules: Vec<Rule>,
}

/// The `user` part.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UserCap {
    /// The scopes.
    pub scopes: Vec<String>,
}

/// What was published: the `capabilities` of the first publish (or the last that restated them).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Declared {
    /// `db`.
    pub db: Option<Db>,
    /// `user`.
    pub user: Option<UserCap>,
    /// `assets`, present only when attachments are on.
    pub assets: Option<serde_json::Value>,
}

/// One test read: what `ArtifactData` `list` returned for each collection at a level.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Read {
    /// `view`, `interact` or `admin`.
    pub level: String,
    /// Collection name to what the read found.
    pub collections: BTreeMap<String, Seen>,
}

/// What a read found.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Seen {
    /// The read succeeded.
    pub readable: bool,
    /// Documents returned.
    pub docs: u32,
}

/// The collections every level must read.
pub const COLLECTIONS: [&str; 5] = ["marks", "roster", "proposals", "meta", "triage"];
/// The levels the test read covers.
pub const LEVELS: [&str; 3] = ["view", "interact", "admin"];

/// The rules `data-model.md` declares: path, read, write.
fn expected_rules() -> Vec<Rule> {
    let r = |p: &str, rd: Option<&str>, w: &str| Rule {
        path: p.to_string(),
        read: rd.map(str::to_string),
        write: Some(w.to_string()),
    };
    vec![
        r("marks", Some("view"), "owner"),
        r("marks/{self}", None, "interact"),
        r("roster", Some("view"), "owner"),
        r("proposals", Some("view"), "owner"),
        r("proposals/{self}", None, "interact"),
        r("meta", Some("view"), "owner"),
        r("triage", Some("view"), "owner"),
    ]
}

/// One comparison.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Check {
    /// `declaration` or `read`.
    pub kind: &'static str,
    /// What was compared.
    pub what: String,
    /// What the contract expects.
    pub expected: String,
    /// What was seen.
    pub seen: String,
    /// They match.
    pub pass: bool,
}

/// The result of `preflight verify`.
#[derive(Debug, Serialize)]
pub struct Verification {
    /// Every comparison.
    pub checks: Vec<Check>,
    /// Every check passed.
    pub ok: bool,
}

fn show(r: &Rule) -> String {
    format!(
        "read {} write {}",
        r.read.as_deref().unwrap_or("-"),
        r.write.as_deref().unwrap_or("-")
    )
}

/// Compare the declaration and the test reads with the contract.
#[must_use]
pub fn verify(r: &Roadmap, declared: &Declared, reads: &[Read]) -> Verification {
    let mut checks = Vec::new();
    let mut push = |kind, what: String, expected: String, seen: String| {
        let pass = expected == seen;
        checks.push(Check {
            kind,
            what,
            expected,
            seen,
            pass,
        });
    };
    let have: &[Rule] = declared.db.as_ref().map_or(&[], |d| d.rules.as_slice());
    for want in expected_rules() {
        let seen = have
            .iter()
            .find(|x| x.path == want.path)
            .map_or_else(|| "missing".to_string(), show);
        push(
            "declaration",
            format!("rule {}", want.path),
            show(&want),
            seen,
        );
    }
    for extra in have
        .iter()
        .filter(|x| !expected_rules().iter().any(|w| w.path == x.path))
    {
        push(
            "declaration",
            format!("rule {}", extra.path),
            "no such rule".into(),
            show(extra),
        );
    }
    let scopes = declared
        .user
        .as_ref()
        .map(|u| u.scopes.clone())
        .unwrap_or_default();
    push(
        "declaration",
        "user scope profile".into(),
        "profile".into(),
        if scopes.iter().any(|x| x == "profile") {
            "profile".into()
        } else {
            "missing".into()
        },
    );
    push(
        "declaration",
        "assets".into(),
        if r.settings.attachments.on {
            "declared"
        } else {
            "not declared"
        }
        .into(),
        if declared.assets.is_some() {
            "declared"
        } else {
            "not declared"
        }
        .into(),
    );
    for level in LEVELS {
        let read = reads.iter().find(|x| x.level == level);
        for c in COLLECTIONS {
            let seen = match read {
                None => "no read".to_string(),
                Some(x) => match x.collections.get(c) {
                    None => "missing".to_string(),
                    Some(s) if s.readable => "readable".to_string(),
                    Some(_) => "hidden".to_string(),
                },
            };
            push("read", format!("{c} at {level}"), "readable".into(), seen);
        }
    }
    let ok = checks.iter().all(|c| c.pass);
    Verification { checks, ok }
}

/// What a passing test read proves.
pub const PROVES: &str = "The register's rules, as enforced for reading: each collection exists under the declared rules and is readable at the lowest level a validator holds, and the declared write rules match the contract.";
/// What it does not.
pub const DOES_NOT_PROVE: &str = "The write side as enforced (reads cannot exercise it) and each person's sharing level (the agent cannot read it). Each validator's own write access is proven only when their R0 write-check mark appears in a snapshot.";

/// The verification as text, ending with what it proves and does not prove.
#[must_use]
pub fn verify_text(v: &Verification) -> String {
    let mut o = String::new();
    for c in &v.checks {
        let _ = writeln!(
            o,
            "{} {:<11} {:<26} expected {:<22} seen {}",
            if c.pass { "ok      " } else { "MISMATCH" },
            c.kind,
            c.what,
            c.expected,
            c.seen
        );
    }
    let bad = v.checks.iter().filter(|c| !c.pass).count();
    if bad == 0 {
        let _ = writeln!(o, "\nPreflight passed.");
    } else {
        let _ = writeln!(o, "\n{bad} mismatch(es): do not send the invitation.");
    }
    let _ = writeln!(o, "\nWhat this proves: {PROVES}");
    let _ = writeln!(o, "What this does not prove: {DOES_NOT_PROVE}");
    o
}
