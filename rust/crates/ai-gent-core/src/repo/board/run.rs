//! The one place a board change is sent (spec 045 FR-020, FR-021). An adapter only plans requests;
//! this runner refuses without `--write` before any call, reads first (board, field, option, item),
//! sends nothing for `--dry-run` or a no-op, and sends the rest through `Net`, which is `api_call`
//! with the caller's `write` flag, so the GraphQL body check and the REST method decide again.
use super::map;
use super::{
    Change, Field, ItemRef, Opt, Req, ReqBody, adapter_for, check_item, parse_item, resolve_ref,
};
use crate::repo::{Ctx, Failed, R};
use crate::tokens::pyjson::{J, dumps};

/// What `set` and `add` take from the command line (or a batch step).
#[derive(Debug, Clone, Default)]
pub struct WriteOpts<'a> {
    /// `add` instead of `set`.
    pub add: bool,
    /// The BOARD argument; none uses the file's only or default board.
    pub board: Option<&'a str>,
    /// `#N` or an issue or pull request URL.
    pub item: &'a str,
    /// A canonical state.
    pub state: Option<&'a str>,
    /// An exact option name.
    pub value: Option<&'a str>,
    /// GitHub: another single-select field than `Status`.
    pub field: Option<&'a str>,
    /// `--entry`.
    pub entry: Option<&'a str>,
    /// `--max-pages`.
    pub max_pages: usize,
    /// `--write`: the person confirmed this change.
    pub write: bool,
    /// `--dry-run`: read, print the plan, send nothing.
    pub dry_run: bool,
    /// The platform the caller works on (a batch plan's); the resolved board must be on it.
    pub platform: Option<&'a str>,
}

fn option_names(field: &Field) -> String {
    let names: Vec<&str> = field.options.iter().map(|o| o.name.as_str()).collect();
    if names.is_empty() {
        "(none)".to_string()
    } else {
        names.join(", ")
    }
}

/// The option a `--state` or `--value` names (FR-011, FR-022).
fn target<'f>(
    field: &'f Field,
    state: Option<&str>,
    value: Option<&str>,
    stale: &[(String, String)],
) -> R<Option<&'f Opt>> {
    if let Some(v) = value.map(str::trim).filter(|v| !v.is_empty()) {
        return field.option(v).map(Some).ok_or_else(|| {
            Failed::err(format!(
                "no option named '{v}' on '{}'; options: {}",
                field.name,
                option_names(field)
            ))
        });
    }
    let Some(st) = state.filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    if let Some((_, name)) = stale.iter().find(|(s, _)| s == st) {
        return Err(Failed::err(format!(
            "stale map: {st} is mapped to '{name}', which the board no longer has; fix .ai-gent/board.yaml (board fields shows the live options)"
        )));
    }
    field
        .options
        .iter()
        .find(|o| o.state == Some(st_static(st)) && o.mappable)
        .map(Some)
        .ok_or_else(|| {
            Failed::err(format!(
                "no option on '{}' matches sync state '{st}'; options: {}; pass --value with the exact option name instead, or map it in .ai-gent/board.yaml",
                field.name,
                option_names(field)
            ))
        })
}

fn st_static(s: &str) -> &'static str {
    super::STATES
        .iter()
        .copied()
        .find(|x| *x == s)
        .unwrap_or("")
}

fn walk<'a>(j: &'a J, path: &[&str]) -> Option<&'a J> {
    path.iter().try_fold(j, |c, k| c.get(k))
}

/// Send one planned request with the caller's `write` flag; the answer's data.
fn send(adapter: &dyn super::BoardAdapter, ctx: &Ctx, req: &Req, prev: &J, write: bool) -> R<J> {
    let net = adapter.net();
    let data = match &req.body {
        ReqBody::Gql { doc, vars } => {
            let mut v = vars.clone();
            if let Some((name, path)) = req.from_prev {
                let id = walk(prev, path).and_then(J::as_str).ok_or_else(|| {
                    Failed::err(format!(
                        "{}: the previous answer has no {}",
                        doc.0,
                        path.join(".")
                    ))
                })?;
                if let Some(slot) = v.iter_mut().find(|(k, _)| *k == name) {
                    slot.1 = J::s(id);
                }
            }
            net.gql_write(ctx, *doc, &v, write)?.data
        }
        ReqBody::Rest { method, path, body } => net.rest_write(ctx, method, path, body, write)?,
    };
    if let Some(path) = req.errors_at {
        if let Some(J::Arr(errs)) = walk(&data, path) {
            if !errs.is_empty() {
                let msg: Vec<String> = errs
                    .iter()
                    .map(|e| {
                        e.as_str()
                            .map_or_else(|| dumps(e, false, None), str::to_string)
                    })
                    .take(3)
                    .collect();
                return Err(Failed::err(format!(
                    "the platform refused the change: {}",
                    msg.join("; ")
                )));
            }
        }
    }
    Ok(data)
}

/// `board set` and `board add`: resolve, read, plan, and send (or print) the change.
///
/// # Errors
/// A usage error for a bad combination of arguments; a refusal (exit 3) without `--write` or
/// `--dry-run`, before any call; and the adapter's failures (not on the board, unmapped or stale
/// state, platform errors).
pub fn run_change(ctx: &Ctx, o: &WriteOpts<'_>) -> R<Change> {
    let state = o.state.filter(|s| !s.is_empty());
    let value = o.value.filter(|s| !s.is_empty());
    if state.is_some() && value.is_some() {
        return Err(Failed::usage("pass --state or --value, not both"));
    }
    if !o.add && state.is_none() && value.is_none() {
        return Err(Failed::usage(
            "pass --state <todo|doing|review|done> (references/sync.md) or --value <exact option name>",
        ));
    }
    if o.write && o.dry_run {
        return Err(Failed::usage(
            "pass --write or --dry-run, not both: --dry-run sends nothing",
        ));
    }
    let item: ItemRef = parse_item(o.item)?;
    let map = map::load(ctx)?;
    let mut bref = resolve_ref(o.board, map.as_ref())?;
    if let Some(p) = o.platform.filter(|p| *p != bref.platform) {
        return Err(Failed::usage(format!(
            "{} is a {} board, the plan is for {p}",
            bref.key(),
            bref.platform
        )));
    }
    check_item(ctx, &bref, &item)?;
    if let Some(f) = o.field.filter(|f| !f.is_empty()) {
        if bref.platform != "github" {
            return Err(Failed::usage("--field is for GitHub boards"));
        }
        bref.field = f.to_string();
    }
    if !o.write && !o.dry_run {
        return Err(Failed::refused(format!(
            "board {} changes {}; after the user confirmed this change, re-run with --write (--dry-run shows the plan and sends nothing)",
            if o.add { "add" } else { "set" },
            bref.key()
        )));
    }
    let adapter = adapter_for(&bref.platform, o.entry, o.max_pages, bref.mode == "status")?;
    let (board, mut field) = adapter.fields(ctx, &bref)?;
    let stale = map::apply(&bref.states, &mut field.options);
    let to = target(&field, state, value, &stale)?;
    let loc = adapter.locate(ctx, &board, &bref, &field, &item)?;
    let reqs = if o.add {
        adapter.plan_add(&board, &field, &loc, to)?
    } else {
        let Some(t) = to else {
            return Err(Failed::usage("set needs --state or --value"));
        };
        adapter.plan_set(&board, &field, &loc, t)?
    };
    let mut sent: Vec<String> = Vec::new();
    let mut failed = None;
    if o.dry_run {
        sent.extend(reqs.iter().map(Req::summary));
    } else {
        let mut prev = J::Null;
        for r in &reqs {
            match send(adapter.as_ref(), ctx, r, &prev, o.write) {
                Ok(d) => prev = d,
                // Nothing sent yet: a plain failure. After a first request went through (the
                // BoardAdd of `add --state`), report what changed together with the failure.
                Err(f) if sent.is_empty() => return Err(f),
                Err(f) => {
                    failed = Some(crate::passstore::redact(&f.msg));
                    break;
                }
            }
            sent.push(r.summary());
        }
    }
    Ok(Change {
        board,
        field: field.name.clone(),
        from: loc.from.clone(),
        to: to.map(|t| t.name.clone()),
        state: to
            .and_then(|t| t.state)
            .map(str::to_string)
            .or_else(|| state.map(str::to_string)),
        changed: !reqs.is_empty(),
        added: o.add && !loc.on_board && !sent.is_empty(),
        failed,
        sent,
        dry_run: o.dry_run,
        item: loc,
    })
}

/// `board set` / `board add`: the lines to print and, when a later request failed after an earlier one
/// had been sent, that failure (the caller prints the lines, then fails).
///
/// # Errors
/// As `run_change`.
pub fn run_write(ctx: &Ctx, o: &WriteOpts<'_>, json: bool) -> R<(Vec<String>, Option<Failed>)> {
    let c = run_change(ctx, o)?;
    let lines = if json {
        vec![dumps(&c.to_json(), false, Some(2))]
    } else {
        c.to_text()
    };
    Ok((lines, c.failed.as_ref().map(Failed::err)))
}
