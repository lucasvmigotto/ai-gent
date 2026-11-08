//! Test support: the contract cases every [`Connector`] must pass, read from `rust/fixtures/db-exec/`.
//!
//! One JSON file per case. The runner is generic over the connector, so the SQLite connector and every
//! later one share the same cases; only the seeding of the database is the caller's (a closure that
//! returns the connection facts for a fresh database). The functions panic with the case name on a
//! mismatch, so they are meant for tests.
//!
//! ```json
//! {
//!   "name": "short-kebab-name",
//!   "derived_from": "where the case comes from",
//!   "kind": "exec | policy | pending",
//!   "engines": ["sqlite"],
//!   "setup": {"sqlite": ["CREATE TABLE ..."]},
//!   "steps": [{"request": {"mode": "read", "statements": ["..."]},
//!              "expected": {"ok": true},
//!              "masked": {"reveal": false, "rows": [["A*** L***"]]}}]
//! }
//! ```
//!
//! A step may also carry `absent` (result keys that must not be there; `absent_port` for the port only), `conn_patch` (connection
//! fields to override, `$dir` being the database's directory), `only: "port"`, `expected_port` and
//! `difference` (an id of `specs/043-dbrun-port/differences.md`); see [`Profile`].
//!
//! `exec` cases run their `steps` in order against one fresh database; `expected` is a subset match on
//! the result line (`"$number"` and `"$string"` match by type, `error_contains` is a substring of the
//! error). `policy` cases check the classifier and cost rules and need no connector. `pending` cases
//! name a behavior of the Python runner that the crate does not own yet; they are listed, not run.
use super::{ConnFacts, Connector};
use crate::core::{classify_statement, cost_rules, mask};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The version of the request/result contract; every fixture and `contract.schema.json` carry it. A
/// change that alters what a connector must answer raises it and re-records the fixtures.
pub const CONTRACT_VERSION: u32 = 1;

/// Whose behavior a run checks.
///
/// - [`Profile::Original`]: what `dbrun.py _exec` does. Run through the process connector against the
///   real script, it proves the fixtures describe the original.
/// - [`Profile::Port`]: the contract of the Rust runner: the same, except for the listed intended
///   differences (a step with `expected_port`, or `only: "port"`, each naming a `difference` id of
///   `specs/043-dbrun-port/differences.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    /// The Python runner.
    Original,
    /// The Rust runner.
    Port,
}

/// The directory of the contract fixtures.
#[must_use]
pub fn fixtures_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/db-exec")
}

/// One fixture file.
#[derive(Debug)]
pub struct Case {
    /// File stem.
    pub file: String,
    /// The whole JSON document.
    pub doc: Value,
}

impl Case {
    /// The case's `kind`.
    #[must_use]
    pub fn kind(&self) -> &str {
        self.doc["kind"].as_str().unwrap_or("")
    }

    /// The case's `name`.
    #[must_use]
    pub fn name(&self) -> &str {
        self.doc["name"].as_str().unwrap_or(&self.file)
    }

    /// The seeding statements for an engine (none when the case lists no setup for it).
    #[must_use]
    pub fn setup(&self, engine: &str) -> Vec<String> {
        self.doc["setup"][engine]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|s| s.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn serves(&self, engine: &str) -> bool {
        self.doc["engines"]
            .as_array()
            .is_some_and(|a| a.iter().any(|e| e == engine))
    }
}

/// Every case file except `redaction.json`, sorted by file name.
///
/// # Panics
/// When the fixtures directory or a file in it cannot be read or parsed.
#[must_use]
pub fn load_cases() -> Vec<Case> {
    let dir = fixtures_dir();
    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .filter(|p| {
            p.file_stem()
                .is_some_and(|s| s != "redaction" && s != "contract.schema")
        })
        .collect();
    files.sort();
    files
        .into_iter()
        .map(|p| {
            let text = std::fs::read_to_string(&p).unwrap();
            Case {
                file: p.file_stem().unwrap().to_string_lossy().into_owned(),
                doc: serde_json::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", p.display())),
            }
        })
        .collect()
}

/// The redaction fixture: connection facts and the expected redacted error.
///
/// # Panics
/// When the file cannot be read or parsed.
#[must_use]
pub fn load_redaction() -> Value {
    let p = fixtures_dir().join("redaction.json");
    serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap()
}

fn matches(expected: &Value, actual: &Value, path: &str) -> Result<(), String> {
    match (expected, actual) {
        (Value::String(s), Value::Number(_)) if s == "$number" => Ok(()),
        (Value::String(s), Value::String(_)) if s == "$string" => Ok(()),
        (Value::Object(e), Value::Object(a)) => {
            for (k, ev) in e {
                if k == "error_contains" {
                    let err = a.get("error").and_then(Value::as_str).unwrap_or("");
                    let want = ev.as_str().unwrap_or("");
                    if !err.contains(want) {
                        return Err(format!("{path}.error: {err:?} lacks {want:?}"));
                    }
                } else {
                    let av = a.get(k).ok_or_else(|| format!("{path}.{k}: missing"))?;
                    matches(ev, av, &format!("{path}.{k}"))?;
                }
            }
            Ok(())
        }
        (Value::Array(e), Value::Array(a)) => {
            if e.len() != a.len() {
                return Err(format!(
                    "{path}: expected {} items, got {}",
                    e.len(),
                    a.len()
                ));
            }
            e.iter()
                .zip(a)
                .enumerate()
                .try_for_each(|(i, (ev, av))| matches(ev, av, &format!("{path}[{i}]")))
        }
        _ if expected == actual => Ok(()),
        _ => Err(format!("{path}: expected {expected}, got {actual}")),
    }
}

/// Runs every `exec` case that lists `engine` against `connector`, through the JSON-line contract
/// ([`Connector::run_line`]). `provision` seeds a fresh database from the case's setup statements for
/// this engine and returns the connection facts. Returns how many cases ran.
///
/// # Panics
/// On the first mismatch, naming the case, the step and the path.
pub fn run_exec_cases<C: Connector + ?Sized>(
    connector: &C,
    engine: &str,
    profile: Profile,
    mut provision: impl FnMut(&Case, &[String]) -> ConnFacts,
) -> usize {
    let mut ran = 0;
    for case in load_cases()
        .iter()
        .filter(|c| c.kind() == "exec" && c.serves(engine))
    {
        let conn = provision(case, &case.setup(engine));
        for (i, step) in case.doc["steps"]
            .as_array()
            .expect("steps")
            .iter()
            .enumerate()
        {
            if step["only"] == "port" && profile == Profile::Original {
                continue;
            }
            let mut request = step["request"].clone();
            request["engine"] = json!(engine);
            request["conn"] = serde_json::to_value(&conn).unwrap();
            if let Some(patch) = step.get("conn_patch").and_then(Value::as_object) {
                let dir = Path::new(conn.path.as_deref().unwrap_or(".")).parent();
                for (k, v) in patch {
                    let text = v.as_str().unwrap_or("").replace(
                        "$dir",
                        &dir.map_or_else(String::new, |d| d.to_string_lossy().into_owned()),
                    );
                    request["conn"][k] = json!(text);
                }
            }
            if let Err(e) = check_schema("request", &schema()["request"], &request) {
                panic!(
                    "{} step {}: request breaks the schema: {e}",
                    case.name(),
                    i + 1
                );
            }
            let line = connector.run_line(&request.to_string());
            assert!(
                !line.contains('\n'),
                "{}: the result is one line",
                case.name()
            );
            let actual: Value = serde_json::from_str(&line).unwrap();
            let at = format!("{} step {}", case.name(), i + 1);
            if let Err(e) = check_schema("result", &schema()["result"], &actual) {
                panic!("{at}: result breaks the schema: {e}\nresult: {line}");
            }
            let expected = match step.get("expected_port") {
                Some(e) if profile == Profile::Port => e,
                _ => &step["expected"],
            };
            if let Err(e) = matches(expected, &actual, &at) {
                panic!("{e}\nresult: {line}");
            }
            for key in step
                .get("absent")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let key = key.as_str().unwrap_or("");
                assert!(
                    actual.get(key).is_none(),
                    "{at}: `{key}` must be absent: {line}"
                );
            }
            if profile == Profile::Port {
                for key in step
                    .get("absent_port")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    let key = key.as_str().unwrap_or("");
                    assert!(
                        actual.get(key).is_none(),
                        "{at}: `{key}` must be absent on the port: {line}"
                    );
                }
            }
            if let Some(m) = step.get("masked") {
                check_masked(&at, m, &actual);
            }
        }
        ran += 1;
    }
    ran
}

fn check_masked(at: &str, m: &Value, actual: &Value) {
    let first = &actual["results"][0];
    let columns: Vec<String> = first["columns"]
        .as_array()
        .expect("columns")
        .iter()
        .map(|c| c.as_str().unwrap().to_string())
        .collect();
    let rows: Vec<Vec<Value>> = first["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .map(|r| r.as_array().unwrap().clone())
        .collect();
    let (masked, kinds) =
        mask::mask_result(&columns, &rows, m["reveal"].as_bool().unwrap_or(false));
    assert_eq!(json!(masked), m["rows"], "{at}: masked rows");
    if let Some(want) = m.get("kinds") {
        let got: BTreeMap<_, _> = kinds.into_iter().collect();
        assert_eq!(json!(got), *want, "{at}: masked kinds");
    }
}

/// Runs every `policy` case: each check names a statement and the layer that must refuse it on a remote
/// database (`classifier` for a write verdict, `cost` for a cost rule, `none` for allowed). Needs no
/// connector. Returns how many checks ran.
///
/// # Panics
/// On the first mismatch.
#[must_use]
pub fn run_policy_cases() -> usize {
    let mut n = 0;
    for case in load_cases().iter().filter(|c| c.kind() == "policy") {
        for check in case.doc["checks"].as_array().expect("checks") {
            let (engine, sql) = (
                check["engine"].as_str().unwrap(),
                check["sql"].as_str().unwrap(),
            );
            let (verdict, why) = classify_statement(sql, engine);
            let cost = cost_rules(sql, "remote", false, engine);
            let (layer, detail) = if verdict == "write" {
                ("classifier", why)
            } else if let Some(c) = cost {
                ("cost", c)
            } else {
                ("none", String::new())
            };
            let want = check["refused_by"].as_str().unwrap();
            assert_eq!(layer, want, "{}: {sql:?} ({detail})", case.name());
            if let Some(part) = check["detail_contains"].as_str() {
                assert!(
                    detail.contains(part),
                    "{}: {detail:?} lacks {part:?}",
                    case.name()
                );
            }
            n += 1;
        }
    }
    n
}

/// The `pending` cases: name and reason, for a test to assert the list is what it expects.
#[must_use]
pub fn pending_cases() -> Vec<(String, String)> {
    load_cases()
        .iter()
        .filter(|c| c.kind() == "pending")
        .map(|c| {
            (
                c.name().to_string(),
                c.doc["reason"].as_str().unwrap_or("").to_string(),
            )
        })
        .collect()
}

/// `contract.schema.json`: JSON Schema (a small subset) for the request and for the result line.
///
/// # Panics
/// When the file cannot be read or parsed.
#[must_use]
pub fn schema() -> Value {
    let p = fixtures_dir().join("contract.schema.json");
    serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap()
}

/// Validates `value` against the subset of JSON Schema the contract schema uses: `type` (one name or a
/// list), `enum`, `required`, `properties`, `additionalProperties: false`, `items`, `minimum`. An
/// unknown schema keyword is an error, so the schema cannot silently stop checking.
///
/// # Errors
/// The path and the reason of the first violation.
pub fn check_schema(path: &str, schema: &Value, value: &Value) -> Result<(), String> {
    const KNOWN: &[&str] = &[
        "type",
        "enum",
        "required",
        "properties",
        "additionalProperties",
        "items",
        "minimum",
        "description",
        "$comment",
        "default",
    ];
    let obj = schema
        .as_object()
        .ok_or_else(|| format!("{path}: schema is not an object"))?;
    if let Some(k) = obj.keys().find(|k| !KNOWN.contains(&k.as_str())) {
        return Err(format!("{path}: unsupported schema keyword {k}"));
    }
    if let Some(t) = obj.get("type") {
        let names: Vec<&str> = match t {
            Value::String(s) => vec![s.as_str()],
            Value::Array(a) => a.iter().filter_map(Value::as_str).collect(),
            _ => return Err(format!("{path}: bad type in schema")),
        };
        let kind = match value {
            Value::Null => "null",
            Value::Bool(_) => "boolean",
            Value::Number(n) if n.is_i64() || n.is_u64() => "integer",
            Value::Number(_) => "number",
            Value::String(_) => "string",
            Value::Array(_) => "array",
            Value::Object(_) => "object",
        };
        let fits = names
            .iter()
            .any(|n| *n == kind || (*n == "number" && kind == "integer"));
        if !fits {
            return Err(format!("{path}: expected {names:?}, got {kind}"));
        }
    }
    if let Some(Value::Array(options)) = obj.get("enum")
        && !options.contains(value)
    {
        return Err(format!("{path}: not one of the allowed values"));
    }
    if let (Some(min), Some(n)) = (obj.get("minimum").and_then(Value::as_f64), value.as_f64())
        && n < min
    {
        return Err(format!("{path}: below the minimum"));
    }
    if let Some(map) = value.as_object() {
        for key in obj
            .get("required")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let key = key.as_str().unwrap_or("");
            if !map.contains_key(key) {
                return Err(format!("{path}: missing {key}"));
            }
        }
        let props = obj.get("properties").and_then(Value::as_object);
        for (k, v) in map {
            match props.and_then(|p| p.get(k)) {
                Some(sub) => check_schema(&format!("{path}.{k}"), sub, v)?,
                None if obj.get("additionalProperties") == Some(&Value::Bool(false)) => {
                    return Err(format!("{path}: unexpected field {k}"));
                }
                None => {}
            }
        }
    }
    if let (Some(items), Some(list)) = (obj.get("items"), value.as_array()) {
        for (i, v) in list.iter().enumerate() {
            check_schema(&format!("{path}[{i}]"), items, v)?;
        }
    }
    Ok(())
}
