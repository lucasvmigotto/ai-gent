//! Black-box tests of the binary: exit codes and the `--json` envelope.
use assert_cmd::Command;

fn bin() -> Command {
    let mut c = Command::cargo_bin("ai-gent").unwrap();
    c.env_remove("AI_GENT_HOME").env("NO_COLOR", "1");
    c
}

#[test]
fn version_json_shape() {
    let out = bin()
        .args(["version", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    assert!(
        text.ends_with('\n') && text.matches('\n').count() == 1,
        "exactly one line"
    );
    let v: serde_json::Value = serde_json::from_str(&text).unwrap();
    // The commit and target vary by machine: pin their type, then redact them in the text so the
    // snapshot also pins the key order.
    let target = v["data"]["target"].as_str().unwrap();
    let mut shown = text.replace(target, "<target>");
    if let Some(commit) = v["data"]["commit"].as_str() {
        shown = shown.replace(commit, "<commit>");
    }
    insta::assert_snapshot!(shown.trim_end(), @r#"{"schema":1,"ok":true,"data":{"crate":"0.1.0","release":"dev","target":"<target>","commit":"<commit>","profile":"debug"}}"#);
}

#[test]
fn json_flag_before_or_after_subcommand() {
    let a = bin()
        .args(["--json", "version"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let b = bin()
        .args(["version", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    assert_eq!(a, b);
}

#[test]
fn version_flag_line() {
    let out = bin()
        .arg("--version")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let text = String::from_utf8(out).unwrap();
    assert!(
        text.starts_with("ai-gent 0.1.0 (") && text.ends_with(")\n"),
        "{text}"
    );
}

#[test]
fn doctor_fails_without_install_root() {
    let d = tempfile::tempdir().unwrap();
    let out = bin()
        .env("AI_GENT_HOME", d.path())
        .args(["doctor", "--json"])
        .assert()
        .code(1)
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    let checks = v["data"]["checks"].as_array().unwrap();
    let status = |id: &str| checks.iter().find(|c| c["id"] == id).unwrap()["status"].clone();
    assert_eq!(status("install_root"), "fail");
    assert_eq!(status("version_match"), "skip");
}

#[test]
fn doctor_passes_in_an_install_root() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir(d.path().join("current")).unwrap();
    bin()
        .env("AI_GENT_HOME", d.path())
        .arg("doctor")
        .assert()
        .success();
}

#[test]
fn usage_error_is_exit_2_with_envelope() {
    let out = bin()
        .args(["--json", "nope"])
        .assert()
        .code(2)
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["ok"], false);
    assert_eq!(v["error"]["code"], "usage");
}

#[test]
fn no_subcommand_is_usage() {
    bin().assert().code(2);
}

fn guard(args: &[&str], input: &str) -> String {
    let out = bin()
        .arg("guard")
        .args(args)
        .write_stdin(input)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    String::from_utf8(out).unwrap()
}

#[test]
fn guard_asks_on_push_and_is_silent_on_ls() {
    let push = r#"{"tool_name":"Bash","cwd":"/tmp","tool_input":{"command":"git push origin x"}}"#;
    let v: serde_json::Value = serde_json::from_str(&guard(&["--guard", "git"], push)).unwrap();
    assert_eq!(v["hookSpecificOutput"]["permissionDecision"], "ask");
    assert!(
        v["hookSpecificOutput"]["permissionDecisionReason"]
            .as_str()
            .unwrap()
            .starts_with("git:workflow guard: ")
    );
    let ls = r#"{"tool_name":"Bash","cwd":"/tmp","tool_input":{"command":"ls -la"}}"#;
    assert_eq!(guard(&["--guard", "all"], ls), "");
}

#[test]
fn guard_failure_is_an_ask_line_and_exit_zero() {
    let v: serde_json::Value =
        serde_json::from_str(&guard(&["--guard", "secrets"], "not json")).unwrap();
    assert_eq!(v["hookSpecificOutput"]["permissionDecision"], "ask");
    assert!(
        v["hookSpecificOutput"]["permissionDecisionReason"]
            .as_str()
            .unwrap()
            .starts_with("secrets guard: guard failed internally")
    );
}

#[test]
fn guard_opencode_format() {
    let out = guard(
        &["--format", "opencode"],
        r#"{"action":"shell","resources":["podman system prune -a"],"cwd":"/tmp"}"#,
    );
    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    assert_eq!(v["decision"], "deny");
}

#[test]
fn guard_list_reasons_json() {
    let out = bin()
        .args(["guard", "list-reasons", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
    assert!(v["data"].as_array().unwrap().len() >= 20);
}
