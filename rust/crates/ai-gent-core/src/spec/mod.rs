//! `ai-gent spec`: what a feature's specs say its tracker items should be (a port of
//! `plugins/repo/scripts/specsync.py`, spec 037 Group B).
//!
//! Reads `specs/NNN-<feature>/` (stories in `spec.md`, checkboxes in `tasks.md`), `specs/README.md` and
//! the feature's `tracker.md`; writes only `specs/NNN-*/tracker.md`. Parity with the Python tool is the
//! contract until the 3.0.0 cutover (D7).
// A line-for-line port of a Python tool; short names and by-hand pattern matching mirror it.
#![allow(
    clippy::many_single_char_names,
    clippy::similar_names,
    clippy::format_push_string,
    clippy::items_after_statements,
    clippy::collapsible_if,
    clippy::manual_strip,
    clippy::cast_possible_truncation,
    clippy::too_many_lines,
    clippy::assigning_clones,
    clippy::missing_panics_doc,
    clippy::format_collect
)]

pub mod digest;
pub mod html;
pub mod parse;
pub mod plan;
pub mod readme;
pub mod tracker;

use crate::tokens::pyjson::{J, dumps, obj, parse as json_parse};
use crate::tokens::util::read_text;
use plan::{Plan, build_plan};
use std::path::{Component, Path, PathBuf};
use tracker::{read_tracker, render_tracker};

/// A failure the tool reports as `error: ...` with exit 1.
#[derive(Debug)]
pub struct Failed(pub String);

fn py_repr(s: &str) -> String {
    if s.contains('\'') && !s.contains('"') {
        format!("\"{s}\"")
    } else {
        format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'"))
    }
}

/// The posix form of a path as `pathlib` shows it (no `.` components, no trailing slash).
fn posix(p: &Path) -> String {
    let parts: Vec<String> = p
        .components()
        .filter(|c| !matches!(c, Component::CurDir))
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect();
    parts.join("/").replace("//", "/")
}

/// Resolve `NNN` or a directory to the feature directory (relative to the current directory).
///
/// # Errors
/// When nothing (or more than one match) fits.
pub fn feature_dir(arg: &str) -> Result<PathBuf, Failed> {
    let p = Path::new(arg);
    if p.is_dir() {
        return Ok(p.to_path_buf());
    }
    if arg.len() == 3 && arg.chars().all(|c| c.is_ascii_digit()) {
        let mut hits: Vec<PathBuf> = std::fs::read_dir("specs")
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with(&format!("{arg}-")))
            })
            .collect();
        hits.sort();
        if hits.len() == 1 {
            return Ok(hits.remove(0));
        }
    }
    Err(Failed(format!(
        "no feature {} under specs/ (pass NNN or the directory)",
        py_repr(arg)
    )))
}

/// Plan a feature directory.
#[must_use]
pub fn plan_for(fdir: &Path) -> Plan {
    build_plan(fdir, &posix(fdir), Path::new("specs"))
}

/// The text output of `spec plan`.
#[must_use]
pub fn plan_text(plan: &Plan) -> String {
    let mut out = String::new();
    if plan.retired {
        out += &format!(
            "{} \u{b7} retired in specs/README.md \u{2014} nothing to sync\n",
            plan.feature
        );
    }
    let statuses = if plan.statuses.is_empty() {
        String::new()
    } else {
        format!(
            " \u{b7} {}",
            plan.statuses
                .iter()
                .map(|(k, v)| format!("{k} {v}"))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    out += &format!(
        "{} \u{b7} tracker: {}{statuses}\n",
        plan.feature,
        if plan.platform.is_empty() {
            "not synced yet"
        } else {
            &plan.platform
        }
    );
    for i in &plan.items {
        let (item_ref, was) = i
            .existing
            .as_ref()
            .map_or(("", ""), |e| (e.item_ref.as_str(), e.state.as_str()));
        let state = if !was.is_empty() && was != i.state {
            format!("{was} \u{2192} {}", i.state)
        } else {
            i.state.clone()
        };
        out += &format!(
            "  {:<8} {:<13} {:<16} {:<10} {}\n",
            i.key, i.action, state, item_ref, i.title
        );
    }
    for k in &plan.orphans {
        out += &format!(
            "  {k:<8} orphan        in tracker.md, gone from the specs \u{2014} report it; never delete the item\n"
        );
    }
    let pending = plan.items.iter().filter(|i| i.action != "none").count();
    if pending > 0 {
        out += &format!("\n{pending} item(s) to create or change\n");
    } else {
        out += "\nin sync\n";
    }
    out
}

fn py_str(v: &J) -> String {
    match v {
        J::Str(s) => s.clone(),
        J::Int(i) => i.to_string(),
        J::Bool(b) => (if *b { "True" } else { "False" }).to_string(),
        J::Float(f) => crate::tokens::pyjson::float_repr(*f),
        other => dumps(other, false, None),
    }
}

/// Merge a `repo batch` output's records into the feature's `tracker.md`; the line to print.
///
/// # Errors
/// When the file cannot be read or parsed, or `tracker.md` cannot be written.
pub fn record(fdir: &Path, platform: &str, wher: &str, from: &Path) -> Result<String, Failed> {
    let text = read_text(from).map_err(|e| Failed(format!("{}: {e}", from.display())))?;
    let data =
        json_parse(&text).ok_or_else(|| Failed(format!("{}: not valid JSON", from.display())))?;
    let records: Vec<J> = match &data {
        J::Obj(_) => match data.get("records") {
            Some(J::Arr(a)) => a.clone(),
            Some(_) => return Err(Failed("records is not a list".into())),
            None => return Err(Failed("the file has no records".into())),
        },
        J::Arr(a) => a.clone(),
        _ => return Err(Failed("the file is not a batch output".into())),
    };
    let tpath = fdir.join("tracker.md");
    let mut tracker = read_tracker(read_text(&tpath).ok().as_deref());
    for r in &records {
        let Some(key) = r.get("key").filter(|k| k.truthy()).map(py_str) else {
            continue;
        };
        let cur = tracker.entry(&key);
        for (field, slot) in [
            ("ref", &mut cur.item_ref),
            ("id", &mut cur.id),
            ("state", &mut cur.state),
            ("hash", &mut cur.hash),
            ("url", &mut cur.url),
        ] {
            if let Some(v) = r.get(field) {
                if !matches!(v, J::Null) && !matches!(v, J::Str(s) if s.is_empty()) {
                    *slot = py_str(v);
                }
            }
        }
    }
    let name = fdir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let body = render_tracker(&name, platform, wher, &tracker.items, &tracker::today());
    std::fs::write(&tpath, body).map_err(|e| Failed(format!("{}: {e}", tpath.display())))?;
    Ok(format!(
        "{}/tracker.md: {} item(s)",
        posix(fdir),
        tracker.items.len()
    ))
}

/// One row of `spec features`: feature, synced, pending, state.
pub type FeatureRow = (String, bool, usize, String);

/// Every feature's state and pending changes.
#[must_use]
pub fn features() -> Vec<FeatureRow> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir("specs")
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.is_dir()
                && p.file_name().is_some_and(|n| {
                    let b = n.to_string_lossy();
                    let b = b.as_bytes();
                    b.len() >= 4 && b[..3].iter().all(u8::is_ascii_digit) && b[3] == b'-'
                })
        })
        .collect();
    dirs.sort();
    dirs.iter()
        .map(|d| {
            let plan = plan_for(d);
            let pending = plan.items.iter().filter(|i| i.action != "none").count();
            let state = if plan.retired {
                "retired".to_string()
            } else {
                plan.items[0].state.clone()
            };
            (
                plan.feature.clone(),
                !plan.platform.is_empty(),
                pending,
                state,
            )
        })
        .collect()
}

/// The `--json` array of `spec features` (`json.dumps(rows, indent=2)`, ASCII-escaped).
#[must_use]
pub fn features_json(rows: &[FeatureRow]) -> String {
    let arr = rows
        .iter()
        .map(|(f, synced, pending, state)| {
            obj(vec![
                ("feature", J::s(f)),
                ("synced", J::Bool(*synced)),
                ("pending", J::Int(i64::try_from(*pending).unwrap_or(0))),
                ("state", J::s(state)),
            ])
        })
        .collect();
    dumps(&J::Arr(arr), true, Some(2))
}
