//! `project-status`: an alias of `board set github:OWNER/NUMBER URL` (spec 045 FR-031). The board
//! adapter reads the field and sets the item over GraphQL; this file keeps the alias table
//! (`ALIASES`, `norm`, `alias_index`, shared with the board map) and the old flags and output.
use super::board::run::{WriteOpts, run_change};
use super::remote::detect_repo;
use super::{Ctx, Failed, R, note};
use crate::tokens::pyjson::{J, obj};

/// Sync state to the live option names a board might use for it, in preference order.
pub const ALIASES: [(&str, &[&str]); 4] = [
    (
        "todo",
        &["to do", "todo", "backlog", "not started", "new", "open"],
    ),
    (
        "doing",
        &[
            "in progress",
            "doing",
            "in-progress",
            "started",
            "active",
            "working",
        ],
    ),
    (
        "review",
        &[
            "in review",
            "review",
            "in-review",
            "ready for review",
            "testing",
            "qa",
        ],
    ),
    (
        "done",
        &["done", "closed", "complete", "completed", "finished"],
    ),
];

/// Lowercase, collapse everything that is not `a-z0-9` to single spaces, trim.
#[must_use]
pub fn norm(s: &str) -> String {
    let mut out = String::new();
    let mut gap = false;
    for c in s.to_lowercase().chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            if gap && !out.is_empty() {
                out.push(' ');
            }
            gap = false;
            out.push(c);
        } else {
            gap = true;
        }
    }
    out
}

/// The index of the option name that matches a sync state: the last exact alias hit in alias order,
/// else the first substring hit (the matching `resolve_option` does, shared with the board map).
#[must_use]
pub fn alias_index(names: &[String], state: &str) -> Option<usize> {
    let aliases = ALIASES.iter().find(|(s, _)| *s == state).map(|(_, a)| *a)?;
    for alias in aliases {
        let a = norm(alias);
        if let Some(i) = names.iter().rposition(|n| norm(n) == a) {
            return Some(i);
        }
    }
    let normed: Vec<String> = aliases.iter().map(|a| norm(a)).collect();
    names.iter().position(|n| {
        let n = norm(n);
        normed
            .iter()
            .any(|a| a.contains(&n) || n.contains(a.as_str()))
    })
}

/// One project-status request.
#[derive(Debug, Clone)]
pub struct Status<'a> {
    /// The project number.
    pub number: i64,
    /// The issue or pull request URL.
    pub url: &'a str,
    /// The project owner login.
    pub owner: Option<&'a str>,
    /// The single-select field name.
    pub field: &'a str,
    /// A sync state to map.
    pub state: Option<&'a str>,
    /// The exact option name.
    pub value: Option<&'a str>,
    /// Another entry under the prefix.
    pub entry: Option<&'a str>,
    /// `--write` was given.
    pub write: bool,
}

/// An alias of `board set github:OWNER/NUMBER URL` (spec 045 FR-031): the same board path, the old
/// flags and the old output keys.
///
/// # Errors
/// Usage errors (no state or value, no owner), the refusal without `--write`, and the board's failures.
pub fn project_status_call(ctx: &Ctx, s: &Status<'_>) -> R<J> {
    let state = s.state.filter(|x| !x.is_empty());
    let value = s.value.filter(|x| !x.is_empty());
    if state.is_none() && value.is_none() {
        return Err(Failed::usage(
            "pass --state <todo|doing|review|done> (references/sync.md) or --value <exact option name>",
        ));
    }
    let owner = match s.owner.filter(|o| !o.is_empty()) {
        Some(o) => o.to_string(),
        None => detect_repo(ctx)
            .get("owner")
            .map(str::to_string)
            .unwrap_or_default(),
    };
    if owner.is_empty() {
        return Err(Failed::usage(
            "no project owner: pass --owner <login> \u{2014} a Projects v2 board belongs to a user or an organization, never to a repository",
        ));
    }
    let board = format!("github:{owner}/{}", s.number);
    let c = run_change(
        ctx,
        &WriteOpts {
            add: false,
            board: Some(&board),
            item: s.url,
            state: if value.is_some() { None } else { state },
            value,
            field: Some(s.field),
            entry: s.entry,
            max_pages: 10,
            write: s.write,
            dry_run: false,
            platform: None,
        },
    )?;
    if let Some(e) = &c.failed {
        return Err(Failed::err(e.clone()));
    }
    let chosen = c.to.clone().unwrap_or_default();
    let suffix = match (state, value) {
        (Some(st), None) => format!(" for state {st}"),
        _ => String::new(),
    };
    note(&format!(
        "project {} ({owner}) \u{b7} {} -> '{chosen}'{suffix}",
        s.number, c.field
    ));
    Ok(obj(vec![
        ("owner", J::s(&owner)),
        ("number", J::Int(s.number)),
        ("url", J::s(s.url)),
        ("field", J::s(&c.field)),
        ("state", J::opt_s(s.state)),
        ("value", J::s(&chosen)),
    ]))
}
