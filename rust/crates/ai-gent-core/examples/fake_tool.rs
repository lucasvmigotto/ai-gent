//! A portable stand-in for `pass`, `gopass`, `gh`, `az`, `glab` and `jira` in tests (spec 037 T001).
//! The behaviour is chosen by the file name it was copied to (`pass`, `pass.exe`, `gh`, ...).
//!
//! - `pass show E` / `gopass show -f E`: print `$PASSWORD_STORE_DIR/E.gpg` (or `.age`), else exit 1.
//! - `pass insert --multiline E` / `gopass insert -m E`: store stdin as `E.gpg`; `FAKE_TOOL_FAIL_INSERT` fails it.
//! - `FAKE_TOOL_SLEEP=<seconds>`: sleep first (timeouts).
//! - anything else (`gh`, `az`, ...): `FAKE_TOOL_RULES` names a JSON file `{"rules": [{"args": "pr list",
//!   "stdout": "x {args} {env:GH_TOKEN}", "stderr": "", "exit": 0, "capture": "path"}], "default": {...}}`;
//!   the first rule whose `args` is a prefix of the joined arguments wins. `{args}` and `{env:NAME}` are
//!   expanded. `capture` writes the joined arguments to that file.
//! - `FAKE_TOOL_LOG`: appends one JSON line per run with the tool, the arguments and the NAMES of the
//!   environment variables that are set (never their values).
use std::io::{Read, Write};
use std::path::PathBuf;

/// Entry point (also called from the `ai-gent` crate example).
pub fn main() {
    let program = std::env::args().next().unwrap_or_default();
    let stem = std::path::Path::new(&program)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Ok(log) = std::env::var("FAKE_TOOL_LOG") {
        let names: Vec<String> = std::env::vars()
            .map(|(k, _)| k)
            .filter(|k| {
                k.ends_with("TOKEN")
                    || k.ends_with("_PAT")
                    || k.starts_with("GITLAB")
                    || k.starts_with("GH_")
            })
            .collect();
        let line = serde_json::json!({"tool": stem, "args": args, "env": names});
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log)
        {
            let _ = writeln!(f, "{line}");
        }
    }
    if let Ok(s) = std::env::var("FAKE_TOOL_SLEEP") {
        std::thread::sleep(std::time::Duration::from_secs(s.parse().unwrap_or(0)));
    }
    let code = match stem.as_str() {
        "pass" | "gopass" => pass_like(&stem, &args),
        _ => rules(&args),
    };
    std::process::exit(code);
}

fn store() -> PathBuf {
    PathBuf::from(std::env::var("PASSWORD_STORE_DIR").unwrap_or_default())
}

fn pass_like(stem: &str, args: &[String]) -> i32 {
    let show = args.first().map(String::as_str) == Some("show");
    let insert = args.first().map(String::as_str) == Some("insert");
    let entry = args.last().cloned().unwrap_or_default();
    if show {
        let exts: &[&str] = if stem == "pass" {
            &["gpg"]
        } else {
            &["gpg", "age"]
        };
        for ext in exts {
            if let Ok(text) = std::fs::read(store().join(format!("{entry}.{ext}"))) {
                let _ = std::io::stdout().write_all(&text);
                return 0;
            }
        }
        eprintln!("Error: {entry} is not in the password store.");
        return 1;
    }
    if insert {
        if std::env::var("FAKE_TOOL_FAIL_INSERT").is_ok() {
            eprintln!("insert failed");
            return 1;
        }
        let mut buf = String::new();
        let _ = std::io::stdin().read_to_string(&mut buf);
        let path = store().join(format!("{entry}.gpg"));
        if let Some(p) = path.parent() {
            let _ = std::fs::create_dir_all(p);
        }
        return i32::from(std::fs::write(path, buf).is_err());
    }
    2
}

fn expand(t: &str, joined: &str) -> String {
    let mut out = t.replace("{args}", joined);
    while let Some(i) = out.find("{env:") {
        let Some(j) = out[i..].find('}') else { break };
        let name = out[i + 5..i + j].to_string();
        let val = std::env::var(&name).unwrap_or_default();
        out.replace_range(i..=i + j, &val);
    }
    out
}

fn rules(args: &[String]) -> i32 {
    let joined = args.join(" ");
    let spec: serde_json::Value = std::env::var("FAKE_TOOL_RULES")
        .ok()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or(serde_json::json!({}));
    let hit = spec["rules"].as_array().and_then(|rs| {
        rs.iter().find(|r| {
            r["args"]
                .as_str()
                .is_some_and(|a| joined == a || joined.starts_with(&format!("{a} ")))
        })
    });
    let default = serde_json::json!({"stdout": "args: {args}\ntoken: {env:GH_TOKEN}\n"});
    let rule = hit.unwrap_or_else(|| {
        if spec["default"].is_object() {
            &spec["default"]
        } else {
            &default
        }
    });
    if let Some(cap) = rule["capture"].as_str() {
        let _ = std::fs::write(cap, format!("{joined}\n"));
    }
    print!("{}", expand(rule["stdout"].as_str().unwrap_or(""), &joined));
    eprint!("{}", expand(rule["stderr"].as_str().unwrap_or(""), &joined));
    i32::try_from(rule["exit"].as_i64().unwrap_or(0)).unwrap_or(1)
}
