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
