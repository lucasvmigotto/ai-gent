//! GitHub Projects v2 status: look the field and option up live through `gh project`, then set the item.
//!
//! GitHub's REST API has no Projects v2 support (GraphQL only); `gh project` wraps the GraphQL and
//! resolves names to node ids itself. No REST fallback: without `gh` there is no path here.
use super::cli::run_cli_capture;
use super::remote::detect_repo;
use super::{Ctx, Failed, R, note};
use crate::tokens::pyjson::{J, obj, parse};

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

fn name_of(o: &J) -> String {
    o.get("name").and_then(J::as_str).unwrap_or("").to_string()
}

/// The single-select field named `field_name` (case-insensitive) from a `field-list` response.
///
/// # Errors
/// When no field has that name, or it has no options.
pub fn resolve_field<'a>(fields: &'a [J], field_name: &str) -> R<&'a J> {
    let want = field_name.to_lowercase();
    for f in fields {
        if matches!(f, J::Obj(_)) && name_of(f).to_lowercase() == want {
            if !matches!(f.get("options"), Some(J::Arr(_))) {
                return Err(Failed::err(format!(
                    "'{}' isn't a single-select field (no options to map a state to)",
                    name_of(f)
                )));
            }
            return Ok(f);
        }
    }
    let names: Vec<String> = fields
        .iter()
        .filter(|f| matches!(f, J::Obj(_)))
        .map(|f| f.get("name").and_then(J::as_str).unwrap_or("?").to_string())
        .collect();
    let list = if names.is_empty() {
        "(none)".to_string()
    } else {
        names.join(", ")
    };
    Err(Failed::err(format!(
        "no field named '{field_name}' on this project; fields: {list}"
    )))
}

/// The field's live option that matches a sync state, by alias then substring.
///
/// # Errors
/// When no option matches (the message lists the options).
pub fn resolve_option<'a>(field: &'a J, state: &str) -> R<&'a J> {
    let options: Vec<&J> = match field.get("options") {
        Some(J::Arr(a)) => a
            .iter()
            .filter(|o| matches!(o, J::Obj(_)) && o.get("name").is_some_and(J::truthy))
            .collect(),
        _ => Vec::new(),
    };
    let aliases = ALIASES
        .iter()
        .find(|(s, _)| *s == state)
        .map(|(_, a)| *a)
        .ok_or_else(|| {
            Failed::err(format!(
                "unknown sync state '{state}' (todo, doing, review, done)"
            ))
        })?;
    for alias in aliases {
        let a = norm(alias);
        if let Some(hit) = options.iter().rev().find(|o| norm(&name_of(o)) == a) {
            return Ok(hit);
        }
    }
    let normed: Vec<String> = aliases.iter().map(|a| norm(a)).collect();
    for o in &options {
        let n = norm(&name_of(o));
        if normed
            .iter()
            .any(|a| a.contains(&n) || n.contains(a.as_str()))
        {
            return Ok(o);
        }
    }
    let names: Vec<String> = options.iter().map(|o| name_of(o)).collect();
    let list = if names.is_empty() {
        "(none)".to_string()
    } else {
        names.join(", ")
    };
    Err(Failed::err(format!(
        "no option on '{}' matches sync state '{state}'; options: {list}; pass --value with the exact option name instead",
        name_of(field)
    )))
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

/// Read the board's field, map the state to its live option (unless `value` gives it), set the item.
///
/// # Errors
/// Usage errors (no state or value, no owner), failures of `gh`, and refusals without `--write`.
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
    let num = s.number.to_string();
    let words = |v: &[&str]| v.iter().map(|x| (*x).to_string()).collect::<Vec<_>>();
    let text = run_cli_capture(
        ctx,
        "github",
        &words(&[
            "project",
            "field-list",
            &num,
            "--owner",
            &owner,
            "--format",
            "json",
        ]),
        s.entry,
        false,
    )?;
    let data = if text.is_empty() {
        obj(vec![])
    } else {
        parse(&text).ok_or_else(|| {
            let cut: String = text.chars().take(500).collect();
            Failed::err(format!("gh project field-list didn't return JSON: {cut}"))
        })?
    };
    let fields: Vec<J> = match data.get("fields") {
        Some(J::Arr(a)) => a.clone(),
        _ => Vec::new(),
    };
    let field = resolve_field(&fields, s.field)?;
    let fname = name_of(field);
    let chosen = match value {
        Some(v) => v.to_string(),
        None => name_of(resolve_option(field, state.unwrap_or(""))?),
    };
    let suffix = match (state, value) {
        (Some(st), None) => format!(" for state {st}"),
        _ => String::new(),
    };
    note(&format!(
        "project {} ({owner}) \u{b7} {fname} -> '{chosen}'{suffix}",
        s.number
    ));
    run_cli_capture(
        ctx,
        "github",
        &words(&[
            "project",
            "item-edit",
            &num,
            "--owner",
            &owner,
            "--url",
            s.url,
            "--field",
            &fname,
            "--value",
            &chosen,
        ]),
        s.entry,
        s.write,
    )?;
    Ok(obj(vec![
        ("owner", J::s(&owner)),
        ("number", J::Int(s.number)),
        ("url", J::s(s.url)),
        ("field", J::s(&fname)),
        ("state", J::opt_s(s.state)),
        ("value", J::s(&chosen)),
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields() -> Vec<J> {
        let j = parse(r#"[{"id":"PVTF_1","name":"Status","type":"ProjectV2SingleSelectField","options":[{"id":"OPT_TODO","name":"To do"},{"id":"OPT_DOING","name":"In-Progress"},{"id":"OPT_DONE","name":"Done ✅"}]},{"id":"PVTF_2","name":"Title","type":"ProjectV2Field"}]"#).unwrap();
        match j {
            J::Arr(a) => a,
            _ => unreachable!(),
        }
    }

    #[test]
    fn field_lookup() {
        let f = fields();
        assert_eq!(
            resolve_field(&f, "status")
                .unwrap()
                .get("name")
                .and_then(J::as_str),
            Some("Status")
        );
        assert!(resolve_field(&f, "Priority").is_err());
        assert!(resolve_field(&f, "Title").is_err());
    }

    #[test]
    fn option_matching() {
        let f = fields();
        let field = resolve_field(&f, "Status").unwrap();
        let id = |s| {
            resolve_option(field, s)
                .unwrap()
                .get("id")
                .and_then(J::as_str)
                .unwrap()
                .to_string()
        };
        assert_eq!(id("todo"), "OPT_TODO");
        assert_eq!(id("done"), "OPT_DONE");
        assert_eq!(id("doing"), "OPT_DOING");
        let other = parse(r#"{"name":"Status","options":[{"id":"o1","name":"Backlog"},{"id":"o2","name":"Shipped"}]}"#).unwrap();
        let e = resolve_option(&other, "review").unwrap_err().msg;
        assert!(e.contains("Backlog") && e.contains("--value"));
    }
}
