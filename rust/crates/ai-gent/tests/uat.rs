//! `ai-gent uat lint` and `uat diff` through the real binary: exit codes and the `--json` envelope.
use assert_cmd::Command;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

const CLEAN: &str = include_str!("../../../fixtures/uat/clean.json");

fn bin() -> Command {
    Command::cargo_bin("ai-gent").expect("binary")
}

fn write(dir: &Path, name: &str, edit: impl FnOnce(&mut Value)) -> PathBuf {
    let mut v: Value = serde_json::from_str(CLEAN).unwrap();
    edit(&mut v);
    let p = dir.join(name);
    std::fs::write(&p, serde_json::to_string_pretty(&v).unwrap()).unwrap();
    p
}

fn envelope(out: &[u8]) -> Value {
    serde_json::from_slice(out).expect("one JSON object on stdout")
}

#[test]
fn lint_clean_exits_0() {
    let d = tempfile::tempdir().unwrap();
    let p = write(d.path(), "r.json", |_| {});
    let out = bin()
        .args(["--json", "uat", "lint"])
        .arg(&p)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    let v = envelope(&out.stdout);
    assert_eq!(v["ok"], true);
    assert_eq!(v["data"]["clean"], true);
    assert_eq!(v["data"]["findings"], json!([]));
}

#[test]
fn lint_text_summarizes() {
    let d = tempfile::tempdir().unwrap();
    let p = write(d.path(), "r.json", |_| {});
    let out = bin().args(["uat", "lint"]).arg(&p).output().unwrap();
    assert_eq!(
        String::from_utf8_lossy(&out.stdout),
        "0 error(s), 0 warning(s)\n"
    );
}

#[test]
fn lint_errors_exit_1_and_name_the_defect() {
    let d = tempfile::tempdir().unwrap();
    let p = write(d.path(), "r.json", |v| {
        v["scripts"][1]["steps"][0]["claims"][0]["label"] = json!("");
    });
    let out = bin()
        .args(["--json", "uat", "lint"])
        .arg(&p)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let v = envelope(&out.stdout);
    assert_eq!(v["data"]["errors"], true);
    assert_eq!(v["data"]["findings"][0]["code"], "claim-without-label");
}

#[test]
fn lint_warnings_alone_exit_0() {
    let d = tempfile::tempdir().unwrap();
    let p = write(d.path(), "r.json", |_| {});
    let out = bin()
        .args(["--json", "uat", "lint", "--budget", "1000"])
        .arg(&p)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        envelope(&out.stdout)["data"]["findings"][0]["code"],
        "size-budget"
    );
}

#[test]
fn lint_reads_mode_b_features_from_root() {
    let d = tempfile::tempdir().unwrap();
    let f = d.path().join("specs/010-orders");
    std::fs::create_dir_all(&f).unwrap();
    std::fs::write(
        f.join("spec.md"),
        "# F\n\n### User Story 1 - A (Priority: P1)\n\n**Acceptance Scenarios**:\n\n1. **Given** a, **When** b, **Then** c.\n",
    )
    .unwrap();
    let p = write(d.path(), "r.json", |v| {
        v["mode"] = json!("B");
        v["features"] = json!(["010"]);
    });
    let out = bin()
        .args(["--json", "uat", "lint", "--root"])
        .arg(d.path())
        .arg(&p)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        envelope(&out.stdout)["data"]["findings"][0]["code"],
        "scenario-without-step"
    );
}

#[test]
fn unreadable_or_invalid_roadmap_is_a_runtime_error() {
    let d = tempfile::tempdir().unwrap();
    let out = bin()
        .args(["--json", "uat", "lint"])
        .arg(d.path().join("missing.json"))
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(envelope(&out.stdout)["error"]["code"], "runtime");
    let bad = d.path().join("bad.json");
    std::fs::write(&bad, "{").unwrap();
    let out = bin().args(["uat", "lint"]).arg(&bad).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("not a roadmap"));
}

#[test]
fn missing_argument_is_usage() {
    let out = bin().args(["uat", "lint"]).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn diff_accepts_a_proper_change() {
    let d = tempfile::tempdir().unwrap();
    let old = write(d.path(), "old.json", |_| {});
    let new = write(d.path(), "new.json", |v| {
        v["version"] = json!(2);
        v["scripts"][2]["steps"][0]["status"] = json!("retired");
    });
    let out = bin()
        .args(["--json", "uat", "diff"])
        .arg(&old)
        .arg(&new)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    let v = envelope(&out.stdout);
    assert_eq!(v["data"]["retired"], json!(["r2-1"]));
    assert_eq!(v["data"]["changelog"], "v2: retired r2-1");
}

#[test]
fn diff_refuses_a_removed_step() {
    let d = tempfile::tempdir().unwrap();
    let old = write(d.path(), "old.json", |_| {});
    let new = write(d.path(), "new.json", |v| {
        v["version"] = json!(2);
        v["scripts"][2]["steps"] = json!([]);
    });
    let out = bin()
        .args(["uat", "diff"])
        .arg(&old)
        .arg(&new)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stdout).starts_with("refused step-removed"));
}

#[test]
fn diff_of_the_same_file_is_no_change() {
    let d = tempfile::tempdir().unwrap();
    let p = write(d.path(), "r.json", |_| {});
    let out = bin()
        .args(["uat", "diff"])
        .arg(&p)
        .arg(&p)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(String::from_utf8_lossy(&out.stdout), "no change\n");
}
