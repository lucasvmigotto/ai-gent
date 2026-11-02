//! `batch`: several calls from a JSON plan, one confirmation. Steps run in order and stop at the first
//! failure; later steps reference earlier results with `{{step.field}}`.
use super::api::{Call, Data, api_call, api_is_write};
use super::cli::{cli_is_write, run_cli_capture};
use super::platforms::{PLATFORMS, platform};
use super::project_status::{Status, project_status_call};
use super::py::py_str;
use super::{Ctx, Failed, R};
use crate::passstore::redact;
use crate::tokens::pyjson::{J, dumps, obj, parse};

/// Results by step name.
pub type Results = Vec<(String, J)>;

fn is_ref_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-')
}

/// `\{\{\s*([A-Za-z0-9_.-]+)\s*\}\}` at the start of `s`: (reference, length).
fn ref_at(s: &str) -> Option<(&str, usize)> {
    let r = s.strip_prefix("{{")?;
    let t = r.trim_start();
    let lead = r.len() - t.len();
    let n: usize = t
        .chars()
        .take_while(|c| is_ref_char(*c))
        .map(char::len_utf8)
        .sum();
    if n == 0 {
        return None;
    }
    let after = t[n..].trim_start();
    let end = after.strip_prefix("}}")?;
    let used = 2 + lead + n + (t[n..].len() - after.len()) + 2;
    let _ = end;
    Some((&t[..n], used))
}

/// Follow a dotted reference through the results.
///
/// # Errors
/// When no earlier step has that name or the path is not in its result.
pub fn lookup(results: &Results, dotted: &str) -> R<J> {
    let (name, rest) = dotted.split_once('.').map_or((dotted, ""), |(a, b)| (a, b));
    let Some((_, first)) = results.iter().rev().find(|(k, _)| k == name) else {
        return Err(Failed::err(format!(
            "{{{{{dotted}}}}}: no earlier step named '{name}'"
        )));
    };
    let mut value = first.clone();
    if !rest.is_empty() {
        for part in rest.split('.') {
            let next = match &value {
                J::Arr(a) if !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()) => {
                    part.parse::<usize>().ok().and_then(|i| a.get(i).cloned())
                }
                J::Obj(_) => value.get(part).cloned(),
                _ => None,
            };
            match next {
                Some(v) => value = v,
                None => {
                    return Err(Failed::err(format!(
                        "{{{{{dotted}}}}}: '{part}' isn't in that step's result"
                    )));
                }
            }
        }
    }
    Ok(value)
}

/// Fill `{{step.field}}` from earlier results; a string that is only a reference keeps its type. Only
/// the batch's own step names (and `self`) are references: other `{{...}}` text stays as written.
///
/// # Errors
/// When a reference to a step cannot be followed.
pub fn resolve(value: &J, results: &Results, names: &[String]) -> R<J> {
    let is_ref = |r: &str| names.iter().any(|n| n == r.split('.').next().unwrap_or(""));
    match value {
        J::Str(s) => {
            let trimmed = s.trim();
            if let Some((r, used)) = ref_at(trimmed) {
                if used == trimmed.len() && is_ref(r) {
                    return lookup(results, r);
                }
            }
            let mut out = String::new();
            let mut rest = s.as_str();
            while !rest.is_empty() {
                if rest.starts_with("{{") {
                    if let Some((r, used)) = ref_at(rest) {
                        if is_ref(r) {
                            out.push_str(&py_str(&lookup(results, r)?));
                        } else {
                            out.push_str(&rest[..used]);
                        }
                        rest = &rest[used..];
                        continue;
                    }
                }
                let c = rest.chars().next().unwrap_or(' ');
                out.push(c);
                rest = &rest[c.len_utf8()..];
            }
            Ok(J::Str(out))
        }
        J::Arr(a) => Ok(J::Arr(
            a.iter()
                .map(|v| resolve(v, results, names))
                .collect::<R<Vec<_>>>()?,
        )),
        J::Obj(m) => Ok(J::Obj(
            m.iter()
                .map(|(k, v)| Ok((k.clone(), resolve(v, results, names)?)))
                .collect::<R<Vec<_>>>()?,
        )),
        other => Ok(other.clone()),
    }
}

fn step_words(v: &J) -> Vec<String> {
    match v {
        J::Arr(a) => a.iter().map(py_str).collect(),
        _ => Vec::new(),
    }
}

fn step_is_write(plat: &str, step: &J) -> R<bool> {
    if let Some(a) = step.get("api") {
        let body = a.get("data").map(|d| dumps(d, true, None).into_bytes());
        let method = a
            .get("method")
            .and_then(J::as_str)
            .unwrap_or("GET")
            .to_uppercase();
        let path = a.get("path").and_then(J::as_str).unwrap_or("");
        return Ok(api_is_write(plat, &method, path, body.as_deref()));
    }
    if let Some(c) = step.get("cli") {
        return Ok(cli_is_write(
            platform(plat).and_then(|p| p.cli).unwrap_or(""),
            &step_words(c),
        ));
    }
    if step.get("project_status").is_some() {
        return Ok(true); // always ends in an item-edit mutation
    }
    Err(Failed::err(format!(
        "step {} has neither api, cli nor project_status",
        step.get("name")
            .map_or_else(|| "None".to_string(), |n| format!("'{}'", py_str(n)))
    )))
}

/// Options of a batch run.
#[derive(Debug, Clone)]
pub struct Options<'a> {
    /// The plan file.
    pub file: &'a str,
    /// The user confirmed this batch.
    pub write: bool,
    /// List the steps, call nothing.
    pub dry_run: bool,
    /// Another entry under the prefix.
    pub entry: Option<&'a str>,
    /// Also write the result JSON here.
    pub out: Option<&'a str>,
}

/// What a run produced: lines for stdout and stderr, and the exit status.
#[derive(Debug, Default)]
pub struct Outcome {
    /// Stdout lines.
    pub stdout: Vec<String>,
    /// Stderr line (the summary).
    pub stderr: Option<String>,
    /// Exit status.
    pub code: i32,
}

fn trunc(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// Run a batch.
///
/// # Errors
/// Plan errors (exit 2 or 1) and refusals; step failures are reported in the result instead.
pub fn run(ctx: &Ctx, o: &Options<'_>) -> R<Outcome> {
    let text = crate::tokens::util::read_text(std::path::Path::new(o.file))
        .map_err(|e| Failed::err(format!("{}: {}", o.file, e.kind())))?;
    let plan = parse(&text).ok_or_else(|| Failed::err(format!("{}: not valid JSON", o.file)))?;
    let plat = plan
        .get("platform")
        .and_then(J::as_str)
        .unwrap_or("")
        .to_string();
    if platform(&plat).is_none() {
        let all: Vec<&str> = PLATFORMS.iter().map(|p| p.name).collect();
        return Err(Failed::usage(format!(
            "the batch's platform '{plat}' isn't one of {}",
            all.join(", ")
        )));
    }
    let steps: Vec<J> = match plan.get("steps") {
        Some(J::Arr(a)) => a.clone(),
        _ => Vec::new(),
    };
    let names: Vec<String> = steps
        .iter()
        .map(|s| s.get("name").map_or_else(String::new, py_str))
        .collect();
    let name_ok = |s: &J| s.get("name").is_some_and(J::truthy);
    let mut seen = std::collections::BTreeSet::new();
    if steps.iter().any(|s| !name_ok(s)) || !names.iter().all(|n| seen.insert(n.clone())) {
        return Err(Failed::usage("every step needs a unique name"));
    }
    if plat != "github" && steps.iter().any(|s| s.get("project_status").is_some()) {
        return Err(Failed::usage(
            "project_status steps need \"platform\": \"github\" \u{2014} Projects v2 is GitHub-only",
        ));
    }
    let mut writes: Vec<String> = Vec::new();
    for (s, n) in steps.iter().zip(&names) {
        if step_is_write(&plat, s)? {
            writes.push(n.clone());
        }
    }
    if !writes.is_empty() && !o.write && !o.dry_run {
        return Err(Failed::refused(format!(
            "{} of {} steps change something on {plat} ({}{}); after the user confirmed this batch, re-run with --write",
            writes.len(),
            steps.len(),
            writes
                .iter()
                .take(8)
                .cloned()
                .collect::<Vec<_>>()
                .join(", "),
            if writes.len() > 8 { "\u{2026}" } else { "" }
        )));
    }
    let mut res = Outcome::default();
    if o.dry_run {
        for (s, n) in steps.iter().zip(&names) {
            let desc = if let Some(a) = s.get("api") {
                format!(
                    "{} {}",
                    a.get("method").map_or_else(|| "GET".to_string(), py_str),
                    a.get("path").map_or_else(|| "None".to_string(), py_str)
                )
            } else if let Some(p) = s.get("project_status") {
                format!(
                    "project item-edit #{} -> {}",
                    p.get("number").map_or_else(|| "None".to_string(), py_str),
                    p.get("state")
                        .filter(|v| v.truthy())
                        .or_else(|| p.get("value"))
                        .map_or_else(|| "None".to_string(), py_str)
                )
            } else {
                step_words(s.get("cli").unwrap_or(&J::Null))
                    .iter()
                    .map(|w| {
                        let w = w.replace('\n', " ");
                        if w.chars().count() > 48 {
                            format!("{}\u{2026}", trunc(&w, 48))
                        } else {
                            w
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            res.stdout.push(format!(
                "{n:<16} {:<5} {desc}",
                if writes.contains(n) { "write" } else { "read " }
            ));
        }
        return Ok(res);
    }

    let mut results: Results = Vec::new();
    let mut refs = names.clone();
    refs.push("self".to_string());
    let mut report_steps: Vec<J> = Vec::new();
    let mut records: Vec<J> = Vec::new();
    let mut ok = true;
    let mut last_name = String::new();
    for (s, name) in steps.iter().zip(&names) {
        let outcome: R<J> = (|| {
            if let Some(a) = s.get("api") {
                let a = resolve(a, &results, &names)?;
                let path = a
                    .get("path")
                    .and_then(J::as_str)
                    .ok_or_else(|| Failed::err("'path'"))?
                    .to_string();
                let method = a.get("method").map_or_else(|| "GET".to_string(), py_str);
                let body = a.get("data").map(|d| dumps(d, true, None).into_bytes());
                let query: Vec<(String, String)> = match a.get("query") {
                    Some(J::Obj(m)) => m.iter().map(|(k, v)| (k.clone(), py_str(v))).collect(),
                    _ => Vec::new(),
                };
                let data = api_call(
                    ctx,
                    &Call {
                        platform: &plat,
                        method: &method,
                        path: &path,
                        body: body.as_deref(),
                        query,
                        entry: o.entry,
                        write: o.write,
                        paginate: a.get("paginate").is_some_and(J::truthy),
                        max_pages: a
                            .get("max_pages")
                            .and_then(J::as_i64)
                            .map_or(10, |n| usize::try_from(n).unwrap_or(10)),
                    },
                )?;
                Ok(match data {
                    Data::Nothing => J::Null,
                    Data::Json(j) => j,
                    Data::Text(t) => J::Str(t),
                })
            } else if let Some(p) = s.get("project_status") {
                let p = resolve(p, &results, &names)?;
                let number = p
                    .get("number")
                    .map(py_str)
                    .and_then(|n| n.parse::<i64>().ok())
                    .ok_or_else(|| Failed::err("'number'"))?;
                let url = p
                    .get("url")
                    .and_then(J::as_str)
                    .ok_or_else(|| Failed::err("'url'"))?
                    .to_string();
                project_status_call(
                    ctx,
                    &Status {
                        number,
                        url: &url,
                        owner: p.get("owner").and_then(J::as_str),
                        field: p.get("field").and_then(J::as_str).unwrap_or("Status"),
                        state: p.get("state").and_then(J::as_str),
                        value: p.get("value").and_then(J::as_str),
                        entry: o.entry,
                        write: o.write,
                    },
                )
            } else {
                let words = resolve(s.get("cli").unwrap_or(&J::Null), &results, &names)?;
                let text = run_cli_capture(ctx, &plat, &step_words(&words), o.entry, o.write)?;
                // `gh api`, `glab api`, `az ... -o json` print JSON: keep it structured.
                Ok(if text.is_empty() {
                    obj(vec![])
                } else {
                    parse(&text).unwrap_or_else(|| obj(vec![("stdout", J::s(&text))]))
                })
            }
        })();
        match outcome {
            Ok(data) => {
                let data = if matches!(data, J::Null) {
                    obj(vec![])
                } else {
                    data
                };
                results.push((name.clone(), data.clone()));
                results.push(("self".to_string(), data));
                if let Some(rec) = s.get("record").filter(|r| r.truthy()) {
                    match resolve(rec, &results, &refs) {
                        Ok(r) => records.push(r),
                        Err(e) => {
                            ok = false;
                            report_steps.push(obj(vec![
                                ("name", J::s(name)),
                                ("ok", J::Bool(false)),
                                ("error", J::s(trunc(&redact(&e.msg), 2000))),
                            ]));
                            last_name.clone_from(name);
                            results.retain(|(k, _)| k != "self");
                            break;
                        }
                    }
                }
                results.retain(|(k, _)| k != "self");
                report_steps.push(obj(vec![("name", J::s(name)), ("ok", J::Bool(true))]));
            }
            Err(e) => {
                ok = false;
                report_steps.push(obj(vec![
                    ("name", J::s(name)),
                    ("ok", J::Bool(false)),
                    ("error", J::s(trunc(&redact(&e.msg), 2000))),
                ]));
                last_name.clone_from(name);
                break;
            }
        }
    }
    let done = report_steps
        .iter()
        .filter(|x| x.get("ok").is_some_and(J::truthy))
        .count();
    let skipped: Vec<String> = names.iter().skip(report_steps.len()).cloned().collect();
    let report = obj(vec![
        ("platform", J::s(&plat)),
        ("ok", J::Bool(ok)),
        ("steps", J::Arr(report_steps)),
        ("records", J::Arr(records)),
        ("skipped", J::strs(&skipped)),
    ]);
    let out_text = dumps(&report, false, Some(2));
    if let Some(path) = o.out {
        std::fs::write(path, format!("{out_text}\n"))
            .map_err(|e| Failed::err(format!("{path}: {}", e.kind())))?;
    }
    res.stdout.push(out_text);
    res.stderr = Some(format!(
        "{done}/{} steps done{}",
        steps.len(),
        if ok {
            String::new()
        } else {
            format!("; stopped at {last_name}")
        }
    ));
    res.code = i32::from(!ok);
    Ok(res)
}
