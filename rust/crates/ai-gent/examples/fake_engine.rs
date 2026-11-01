//! A fake container engine for the `wt` parity tests (spec 036): a port of the script in
//! `plugins/agents/scripts/test_wt.py`. State lives in the JSON file named by `FAKE_ENGINE_STATE`
//! (`containers`, `volumes`, `images`, `calls`, optional `stuck` and `graphroot`); every call is
//! recorded and any `prune` fails. Built only with the `test-support` feature.
#![allow(missing_docs)]
use serde_json::{Value, json};
use std::process::ExitCode;

fn opt(argv: &[String], name: &str) -> Vec<String> {
    argv.windows(2)
        .filter(|w| w[0] == name)
        .map(|w| w[1].clone())
        .collect()
}

fn matches(obj: &Value, filters: &[String]) -> bool {
    for f in filters {
        let (k, v) = f.split_once('=').unwrap_or((f, ""));
        if k == "dangling" {
            if obj
                .get("dangling")
                .is_some_and(|d| d.as_bool() == Some(true) || d.as_i64().is_some_and(|n| n != 0))
                != (v == "true")
            {
                return false;
            }
        } else if k == "label" {
            let (lk, eq, lv) = match v.split_once('=') {
                Some((a, b)) => (a, true, b),
                None => (v, false, ""),
            };
            let labels = obj.get("labels").and_then(Value::as_object);
            match labels.and_then(|l| l.get(lk)) {
                None => return false,
                Some(x) if eq && x.as_str() != Some(lv) => return false,
                Some(_) => {}
            }
        }
    }
    true
}

fn s<'a>(o: &'a Value, k: &str) -> &'a str {
    o.get(k).and_then(Value::as_str).unwrap_or("")
}

fn fmt(o: &Value, f: &str) -> String {
    let mut f = f.to_string();
    for (k, v) in [
        ("ID", "id"),
        ("Names", "name"),
        ("Status", "status"),
        ("Name", "name"),
    ] {
        f = f.replace(&format!("{{{{.{k}}}}}"), s(o, v));
    }
    f.replace("\\t", "\t")
}

fn is_external(c: &Value) -> bool {
    c.get("external").is_some_and(|v| v.as_bool() == Some(true))
}

fn list_mut<'a>(st: &'a mut Value, k: &str) -> &'a mut Vec<Value> {
    st[k].as_array_mut().expect("array")
}

fn main() -> ExitCode {
    let path = std::env::var("FAKE_ENGINE_STATE").expect("FAKE_ENGINE_STATE");
    let mut st: Value =
        serde_json::from_str(&std::fs::read_to_string(&path).expect("state")).expect("json");
    let argv: Vec<String> = std::env::args().skip(1).collect();
    list_mut(&mut st, "calls").push(json!(argv));
    let a: Vec<&str> = argv.iter().map(String::as_str).collect();
    let stuck: Vec<String> = st["stuck"]
        .as_array()
        .map(|v| {
            v.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let (mut out, mut code) = (String::new(), 0u8);
    let has_ext = a.contains(&"--external");
    if a.contains(&"prune") {
        code = 99;
    } else if a.first() == Some(&"info") {
        out = st
            .get("graphroot")
            .and_then(Value::as_str)
            .unwrap_or("/")
            .to_string();
    } else if a.starts_with(&["system", "df"]) {
        out = "TYPE TOTAL".into();
    } else if a.first() == Some(&"ps") {
        let filters = opt(&argv, "--filter");
        let rows: Vec<&Value> = st["containers"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|c| {
                (is_external(c) == has_ext || (has_ext && !is_external(c))) && matches(c, &filters)
            })
            .collect();
        let format = opt(&argv, "--format").first().cloned().unwrap_or_default();
        out = rows
            .iter()
            .map(|c| {
                if a.contains(&"-aq") {
                    s(c, "id").to_string()
                } else {
                    fmt(c, &format)
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
    } else if a.starts_with(&["volume", "ls"]) {
        let filters = opt(&argv, "--filter");
        out = st["volumes"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|v| matches(v, &filters))
            .map(|v| s(v, "name").to_string())
            .collect::<Vec<_>>()
            .join("\n");
    } else if a.starts_with(&["volume", "inspect"]) {
        let rows: Vec<Value> = st["volumes"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|v| a[2..].contains(&s(v, "name")))
            .map(|v| json!({"Name": s(v, "name"), "Labels": v.get("labels").cloned().unwrap_or(json!({}))}))
            .collect();
        out = Value::Array(rows).to_string();
    } else if a.first() == Some(&"images") {
        let filters = opt(&argv, "--filter");
        out = st["images"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|i| matches(i, &filters))
            .map(|i| s(i, "id").to_string())
            .collect::<Vec<_>>()
            .join("\n");
    } else if a.starts_with(&["volume", "rm"]) {
        let name = a.get(2).copied().unwrap_or("");
        if stuck.iter().any(|x| x == name) {
            code = 1;
        }
        list_mut(&mut st, "volumes").retain(|v| s(v, "name") != name || code != 0);
    } else if a.first() == Some(&"rm") {
        let last = a.last().copied().unwrap_or("");
        list_mut(&mut st, "containers")
            .retain(|c| s(c, "id") != last || stuck.iter().any(|x| x == s(c, "id")));
    } else if a.first() == Some(&"rmi") {
        let id = a.get(1).copied().unwrap_or("");
        if stuck.iter().any(|x| x == id) {
            code = 1;
        }
        list_mut(&mut st, "images").retain(|i| s(i, "id") != id || code != 0);
    } else {
        code = 2;
    }
    std::fs::write(&path, st.to_string()).expect("save");
    println!("{out}");
    ExitCode::from(code)
}
