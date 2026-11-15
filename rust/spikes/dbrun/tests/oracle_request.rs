#![cfg(feature = "oracle")]
//! The adapter binary must answer a hostile `timeout` or an unreachable server with one line of failure
//! JSON, never a panic, and must not wait out the thin client's default one second retry delay.
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Instant;

fn adapter(request: &str) -> (bool, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_adapter"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(request.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(
        out.stderr.is_empty(),
        "panic output: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    (out.status.success(), String::from_utf8(out.stdout).unwrap())
}

fn request(timeout: &str) -> String {
    format!(
        r#"{{"engine":"oracle","conn":{{"host":"127.0.0.1","port":"1","user":"u","password":"p","service":"S"}},"mode":"read","statements":["SELECT 1 FROM DUAL"],"timeout":{timeout}}}"#
    )
}

#[test]
fn negative_nan_like_and_huge_timeouts_return_failure_json() {
    for timeout in ["-6", "-1e-9", "\"NaN\"", "1e300", "1e999", "NaN"] {
        let (ok, stdout) = adapter(&request(timeout));
        assert!(ok, "{timeout}");
        assert_eq!(stdout.lines().count(), 1, "{timeout}");
        let v: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
        assert_eq!(v["ok"], false, "{timeout}");
        assert_eq!(v["stage"], "connect", "{timeout}");
    }
}

#[test]
fn a_refused_port_fails_fast_with_the_password_nowhere_in_the_answer() {
    let started = Instant::now();
    let (ok, stdout) = adapter(&request("5"));
    assert!(ok);
    let v: serde_json::Value = serde_json::from_str(stdout.trim()).unwrap();
    assert_eq!(v["ok"], false);
    assert_eq!(v["stage"], "connect");
    assert!(v["error"].as_str().is_some_and(|e| !e.contains("\"p\"")));
    assert!(
        started.elapsed().as_millis() < 900,
        "the thin client's one second retry delay was not switched off"
    );
}
