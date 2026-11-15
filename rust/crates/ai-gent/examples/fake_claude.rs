//! A stand-in for `claude` in the `delegate` tests. It speaks just enough of the CLI: `--version` and
//! `-p <brief> --output-format json ...`, which first reads stdin to its end (the real CLI waits when
//! stdin is open), then prints one JSON result object, as the real one does when it ends.
//!
//! The mode is a `MODE:<name>` line in the brief (an environment variable would race between parallel
//! tests): `ok` (default), `fail` (exit 1 with `is_error`), `maxturns` (exit 0 with `is_error` and `error_max_turns`), `edit` (also writes
//! `fake-output.txt` in the working directory), `slow` (sleeps 3 s before answering, to prove no first
//! event is awaited), `stall` (never answers). `FAKE_LOGGED_IN=0` makes the probe fail like a missing
//! login.
#![allow(missing_docs)]
use std::io::Read;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--version") {
        println!("2.0.0-fake (Claude Code)");
        return;
    }
    if args.first().map(String::as_str) != Some("-p") || args.len() < 2 {
        eprintln!("unsupported: {args:?}");
        std::process::exit(2);
    }
    let brief = args[1].clone();
    let mut sink = Vec::new();
    let _ = std::io::stdin().read_to_end(&mut sink);
    let mode = ["fail", "maxturns", "edit", "slow", "stall"]
        .into_iter()
        .find(|m| brief.contains(&format!("MODE:{m}")))
        .unwrap_or("ok");
    if brief.starts_with("Reply with exactly:") {
        if std::env::var("FAKE_LOGGED_IN").is_ok_and(|v| v == "0") {
            println!(r#"{{"type":"result","is_error":true,"result":"Not logged in"}}"#);
            std::process::exit(1);
        }
        println!(r#"{{"type":"result","is_error":false,"result":"READY","num_turns":1}}"#);
        return;
    }
    if mode == "stall" {
        std::thread::sleep(std::time::Duration::from_secs(120));
        return;
    }
    if mode == "slow" {
        std::thread::sleep(std::time::Duration::from_secs(3));
    }
    let cwd = std::env::current_dir().unwrap_or_default();
    if mode == "edit" {
        let _ = std::fs::write(cwd.join("fake-output.txt"), "delegate output\n");
    }
    let failed = mode == "fail";
    let max_turns = mode == "maxturns";
    let v = serde_json::json!({
        "type": "result",
        "subtype": if failed || max_turns { "error_max_turns" } else { "success" },
        "is_error": failed || max_turns,
        "num_turns": 3,
        "result": "Done: T010c. Rules I found hard to follow: none.",
        "usage": {
            "input_tokens": 200,
            "cache_creation_input_tokens": 5,
            "cache_read_input_tokens": 800,
            "output_tokens": 50,
        },
        "fake": {
            "stdin_bytes": sink.len(),
            "config_dir": std::env::var("CLAUDE_CONFIG_DIR").unwrap_or_default(),
            "tmpdir": std::env::var("TMPDIR").unwrap_or_default(),
            "cwd": cwd.to_string_lossy(),
            "brief_len": brief.len(),
            "args": args,
        },
    });
    println!("{v}");
    if failed {
        std::process::exit(1);
    }
}
