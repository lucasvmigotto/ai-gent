//! A stand-in for `codex` in the `delegate` tests. It speaks just enough of the CLI:
//! `--version`, `login status`, and `exec ... -o <last> --json <brief>`, which first reads stdin to
//! its end (as the real CLI does: it hangs when stdin is open), then streams JSON events.
//!
//! `FAKE_AGENT_MODE`: `ok` (default), `stall` (one event, then silence), `fail` (exit 3 after events),
//! `silent` (no event at all), `edit` (also writes `fake-output.txt` in the `--cd` directory).
//! `FAKE_LOGGED_IN=0` makes `login status` fail. `FAKE_ITEMS` is the number of items (default 3).
#![allow(missing_docs)]
use std::io::{Read, Write};

fn emit(line: &str) {
    let mut o = std::io::stdout().lock();
    let _ = writeln!(o, "{line}");
    let _ = o.flush();
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--version") {
        println!("codex-cli 0.0.0-fake");
        return;
    }
    if args.len() >= 2 && args[0] == "login" && args[1] == "status" {
        let ok = std::env::var("FAKE_LOGGED_IN").map_or(true, |v| v != "0");
        std::process::exit(i32::from(!ok));
    }
    if args.first().map(String::as_str) != Some("exec") {
        eprintln!("unsupported: {args:?}");
        std::process::exit(2);
    }
    let value = |flag: &str| {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    // The real CLI waits here when stdin is left open.
    let mut sink = Vec::new();
    let _ = std::io::stdin().read_to_end(&mut sink);
    let mode = std::env::var("FAKE_AGENT_MODE").unwrap_or_else(|_| "ok".into());
    if mode == "silent" {
        std::thread::sleep(std::time::Duration::from_secs(120));
        return;
    }
    emit(r#"{"type":"thread.started","thread_id":"t"}"#);
    emit(&format!(
        r#"{{"type":"fake.env","stdin_bytes":{},"tmpdir":{},"brief_len":{},"args":{}}}"#,
        sink.len(),
        serde_json::json!(std::env::var("TMPDIR").unwrap_or_default()),
        args.last().map_or(0, String::len),
        serde_json::json!(args),
    ));
    if mode == "stall" {
        emit(r#"{"type":"item.started","item":{"id":"1"}}"#);
        std::thread::sleep(std::time::Duration::from_secs(120));
        return;
    }
    let n: u32 = std::env::var("FAKE_ITEMS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3);
    for i in 0..n {
        emit(&format!(
            r#"{{"type":"item.started","item":{{"id":"{i}"}}}}"#
        ));
        if i == 1 {
            // one failed apply_patch mid-run, retried by the agent: not a stall
            emit(r#"{"type":"error","message":"apply_patch verification failed"}"#);
        }
        emit(&format!(
            r#"{{"type":"item.completed","item":{{"id":"{i}"}}}}"#
        ));
    }
    if mode == "edit"
        && let Some(dir) = value("--cd")
    {
        let _ = std::fs::write(
            std::path::Path::new(&dir).join("fake-output.txt"),
            "delegate output\n",
        );
    }
    if let Some(last) = value("-o") {
        let _ = std::fs::write(last, "Done: T010. Rules I found hard to follow: none.\n");
    }
    emit(
        r#"{"type":"turn.completed","usage":{"input_tokens":1000,"cached_input_tokens":800,"output_tokens":50,"reasoning_output_tokens":10}}"#,
    );
    if mode == "fail" {
        std::process::exit(3);
    }
}
