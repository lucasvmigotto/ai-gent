//! A stand-in for `opencode` in the `delegate` tests. It speaks just enough of the CLI: `--version` and
//! `run --standalone --model M --format json --auto <brief>`, which first reads stdin to its end (the
//! real CLI could wait when stdin is open), then prints `step_start`, `tool_use`, `text` and
//! `step_finish` events as JSON lines, as the real one does.
//!
//! The mode is a `MODE:<name>` line in the brief (an environment variable would race between parallel
//! tests): `ok` (default; two steps, the last with reason `stop`), `exit1` (the same, then exit 1),
//! `nostop` (exit 0 but the last step ends with reason `tool-calls`), `edit` (also tries to write
//! `fake-output.txt` in the working directory), `stall` (never answers). `FAKE_LOGGED_IN=0` makes the
//! probe fail like a missing login.
//!
//! The brief may also hold `EDIT:<path>`, `READ:<path>` and `SHELL:<command>` lines: attempts the fake
//! makes the way OpenCode would. Each one is matched against the permission set it received in
//! `OPENCODE_CONFIG_CONTENT` (a rule applies when its action is equal and its resource, where `*` is
//! any run of characters, matches the whole target). A denied edit is not made; an allowed shell
//! command is not run. The `tool_use` event reports every attempt and its outcome.
#![allow(missing_docs, clippy::doc_markdown)]
use serde_json::{Value, json};
use std::io::Read;

fn step_finish(reason: &str, input: u64, output: u64, reasoning: u64, read: u64, write: u64) {
    println!(
        "{}",
        json!({"type":"step_finish","part":{"reason":reason,"cost":0,"tokens":{
            "input":input,"output":output,"reasoning":reasoning,"cache":{"read":read,"write":write}}}})
    );
}

/// Whether `text` matches `pat` as a whole, `*` standing for any run of characters (`/` included).
fn glob(pat: &str, text: &str) -> bool {
    let (p, t): (Vec<char>, Vec<char>) = (pat.chars().collect(), text.chars().collect());
    let (mut pi, mut ti) = (0, 0);
    let (mut star, mut mark) = (None, 0);
    while ti < t.len() {
        if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ti;
            pi += 1;
        } else if pi < p.len() && p[pi] == t[ti] {
            pi += 1;
            ti += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    p[pi..].iter().all(|c| *c == '*')
}

/// The rule that denies `action` on `target`, if any.
fn denied_by(rules: &[Value], action: &str, target: &str) -> Option<String> {
    rules
        .iter()
        .find(|r| {
            r["effect"] == "deny"
                && r["action"] == action
                && r["resource"].as_str().is_some_and(|p| glob(p, target))
        })
        .and_then(|r| r["resource"].as_str().map(str::to_string))
}

/// The variables the adapter must have removed that are still in the environment.
fn env_present() -> Vec<&'static str> {
    [
        "INIT_CWD",
        "OLDPWD",
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_COMMON_DIR",
        "GIT_INDEX_FILE",
        "GIT_PREFIX",
    ]
    .into_iter()
    .filter(|k| std::env::var_os(k).is_some())
    .collect()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--version") {
        println!("2.2.5-fake");
        return;
    }
    if args.first().map(String::as_str) != Some("run") || args.len() < 2 {
        eprintln!("unsupported: {args:?}");
        std::process::exit(2);
    }
    let brief = args.last().cloned().unwrap_or_default();
    let mut sink = Vec::new();
    let _ = std::io::stdin().read_to_end(&mut sink);
    let mode = ["exit1", "nostop", "edit", "stall"]
        .into_iter()
        .find(|m| brief.contains(&format!("MODE:{m}")))
        .unwrap_or("ok");
    if brief.starts_with("Reply with exactly:") {
        if std::env::var("FAKE_LOGGED_IN").is_ok_and(|v| v == "0") {
            println!(r#"{{"type":"error","message":"not logged in"}}"#);
            std::process::exit(1);
        }
        println!(r#"{{"type":"step_start","part":{{}}}}"#);
        println!(r#"{{"type":"text","part":{{"text":"READY"}}}}"#);
        step_finish("stop", 10, 1, 0, 0, 0);
        return;
    }
    if mode == "stall" {
        println!(r#"{{"type":"step_start","part":{{}}}}"#);
        std::thread::sleep(std::time::Duration::from_secs(120));
        return;
    }
    let cwd = std::env::current_dir().unwrap_or_default();
    let config = std::env::var("OPENCODE_CONFIG_CONTENT").unwrap_or_default();
    let rules: Vec<Value> = serde_json::from_str::<Value>(&config)
        .ok()
        .and_then(|c| c["permissions"].as_array().cloned())
        .unwrap_or_default();

    let mut attempts: Vec<(&str, String)> = Vec::new();
    if mode == "edit" {
        attempts.push((
            "edit",
            cwd.join("fake-output.txt").to_string_lossy().into_owned(),
        ));
    }
    for line in brief.lines() {
        for (prefix, action) in [("EDIT:", "edit"), ("READ:", "read"), ("SHELL:", "shell")] {
            if let Some(t) = line.strip_prefix(prefix) {
                attempts.push((action, t.trim().to_string()));
            }
        }
    }
    let mut report = Vec::new();
    for (action, target) in &attempts {
        let rule = denied_by(&rules, action, target);
        if rule.is_none() && *action == "edit" {
            let _ = std::fs::write(target, "delegate output\n");
        }
        report.push(
            json!({"action": action, "target": target, "denied": rule.is_some(), "rule": rule}),
        );
    }

    let env_present = env_present();
    println!(r#"{{"type":"step_start","part":{{}}}}"#);
    println!(
        "{}",
        json!({"type":"tool_use","fake":{
            "stdin_bytes": sink.len(),
            "config": config,
            "tmpdir": std::env::var("TMPDIR").unwrap_or_default(),
            "cwd": cwd.to_string_lossy(),
            "pwd": std::env::var("PWD").ok(),
            "env_present": env_present,
            "brief_len": brief.len(),
            "args": args,
            "attempts": report,
        }})
    );
    step_finish("tool-calls", 100, 10, 1, 50, 5);
    println!(r#"{{"type":"step_start","part":{{}}}}"#);
    println!(
        r#"{{"type":"text","part":{{"text":"Done: T010a. Rules I found hard to follow: none."}}}}"#
    );
    step_finish(
        if mode == "nostop" {
            "tool-calls"
        } else {
            "stop"
        },
        200,
        20,
        2,
        150,
        0,
    );
    if mode == "exit1" {
        std::process::exit(1);
    }
}
