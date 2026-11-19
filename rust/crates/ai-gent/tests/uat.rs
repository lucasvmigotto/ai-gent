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

// render, digest, report and preflight (T012 to T014)

fn write_json(dir: &Path, name: &str, v: &Value) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, serde_json::to_string_pretty(v).unwrap()).unwrap();
    p
}

fn snapshot(at: &str) -> Value {
    json!({
        "at": at,
        "roster": {"u1": {"validator": "v1", "role": "admin"}},
        "marks": {"u1": {"steps": {"r0-1": {"r": "OK", "at": "2026-11-20T09:00:00Z", "rev": 1}}}}
    })
}

#[test]
fn render_writes_the_page_and_refuses_a_broken_roadmap() {
    let d = tempfile::tempdir().unwrap();
    let ok = write(d.path(), "r.json", |_| {});
    let out = d.path().join("page.html");
    let run = bin()
        .args(["--json", "uat", "render"])
        .arg(&ok)
        .arg("--out")
        .arg(&out)
        .output()
        .unwrap();
    assert_eq!(run.status.code(), Some(0));
    assert_eq!(
        envelope(&run.stdout)["data"]["out"],
        out.display().to_string()
    );
    let html = std::fs::read_to_string(&out).unwrap();
    assert!(html.contains("data-step=\"r1-2\"") && !html.contains("ana@example.test"));
    let bad = write(d.path(), "bad.json", |v| v["how_to"] = json!(""));
    let run = bin().args(["uat", "render"]).arg(&bad).output().unwrap();
    assert_eq!(run.status.code(), Some(1));
    assert!(run.stdout.is_empty());
    assert!(String::from_utf8_lossy(&run.stderr).contains("missing-section"));
}

#[test]
fn render_prints_to_stdout_without_out_but_not_with_json() {
    let d = tempfile::tempdir().unwrap();
    let ok = write(d.path(), "r.json", |_| {});
    let run = bin().args(["uat", "render"]).arg(&ok).output().unwrap();
    assert_eq!(run.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&run.stdout).contains("<tr data-step=\"r0-1\""));
    let run = bin()
        .args(["--json", "uat", "render"])
        .arg(&ok)
        .output()
        .unwrap();
    assert_eq!(run.status.code(), Some(1));
}

#[test]
fn digest_reports_and_says_no_news() {
    let d = tempfile::tempdir().unwrap();
    let r = write(d.path(), "r.json", |_| {});
    let a = write_json(d.path(), "a.json", &snapshot("2026-11-20T10:00:00Z"));
    let b = write_json(d.path(), "b.json", &snapshot("2026-11-20T10:30:00Z"));
    let run = bin()
        .args(["uat", "digest"])
        .arg(&r)
        .arg(&a)
        .output()
        .unwrap();
    assert_eq!(run.status.code(), Some(0));
    let text = String::from_utf8_lossy(&run.stdout);
    assert!(
        text.starts_with("Last real check: 2026-11-20 10:00\n"),
        "{text}"
    );
    let run = bin()
        .args(["uat", "digest"])
        .arg(&r)
        .arg(&b)
        .arg("--since")
        .arg(&a)
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&run.stdout),
        "No news since 10:00 (last real check 10:30). Next check on request.\n"
    );
    let run = bin()
        .args(["--json", "uat", "digest"])
        .arg(&r)
        .arg(&a)
        .output()
        .unwrap();
    assert_eq!(envelope(&run.stdout)["data"]["scripts"][0]["ok"], 1);
}

#[test]
fn digest_with_a_bad_snapshot_is_a_runtime_error() {
    let d = tempfile::tempdir().unwrap();
    let r = write(d.path(), "r.json", |_| {});
    let bad = d.path().join("s.json");
    std::fs::write(&bad, "[").unwrap();
    let run = bin()
        .args(["uat", "digest"])
        .arg(&r)
        .arg(&bad)
        .output()
        .unwrap();
    assert_eq!(run.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&run.stderr).contains("not a snapshot"));
}

#[test]
fn report_writes_markdown_with_the_records() {
    let d = tempfile::tempdir().unwrap();
    let r = write(d.path(), "r.json", |_| {});
    let a = write_json(d.path(), "a.json", &snapshot("2026-11-20T10:00:00Z"));
    let pre = d.path().join("preflight.md");
    std::fs::write(&pre, "Preflight passed.\n").unwrap();
    let out = d.path().join("report.md");
    let run = bin()
        .args(["uat", "report"])
        .arg(&r)
        .arg(&a)
        .arg("--preflight")
        .arg(&pre)
        .arg("--out")
        .arg(&out)
        .output()
        .unwrap();
    assert_eq!(run.status.code(), Some(0));
    let md = std::fs::read_to_string(&out).unwrap();
    assert!(md.starts_with("# Order approval acceptance test — consolidated report"));
    assert!(md.contains("## Preflight record\n\nPreflight passed."));
}

#[test]
fn preflight_checklist_exit_code_follows_the_lines() {
    let d = tempfile::tempdir().unwrap();
    let needs = write(d.path(), "a.json", |_| {});
    let run = bin()
        .args(["uat", "preflight", "checklist"])
        .arg(&needs)
        .output()
        .unwrap();
    assert_eq!(run.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&run.stdout).contains("Say which level"));
    let fine = write(d.path(), "b.json", |v| {
        v["validators"][0]["planned_sharing"] = json!("contributor");
        v["validators"][1]["planned_sharing"] = json!("contributor");
    });
    let run = bin()
        .args(["--json", "uat", "preflight", "checklist"])
        .arg(&fine)
        .output()
        .unwrap();
    assert_eq!(run.status.code(), Some(0));
    assert_eq!(envelope(&run.stdout)["data"]["action_needed"], false);
}

fn read_snapshot(hide: &[&str]) -> Value {
    let mut cols = serde_json::Map::new();
    for c in ["marks", "roster", "proposals", "meta", "triage"] {
        if !hide.contains(&c) {
            cols.insert(c.into(), json!({"readable": true, "docs": 0}));
        }
    }
    json!({"collections": cols})
}

#[test]
fn preflight_verify_exit_codes() {
    let d = tempfile::tempdir().unwrap();
    let r = write(d.path(), "r.json", |_| {});
    let declared = write_json(
        d.path(),
        "declared.json",
        &json!({
            "db": {"rules": [
                {"path": "marks", "read": "view", "write": "owner"},
                {"path": "marks/{self}", "write": "interact"},
                {"path": "roster", "read": "view", "write": "owner"},
                {"path": "proposals", "read": "view", "write": "owner"},
                {"path": "proposals/{self}", "write": "interact"},
                {"path": "meta", "read": "view", "write": "owner"},
                {"path": "triage", "read": "view", "write": "owner"}
            ]},
            "user": {"scopes": ["profile"]}
        }),
    );
    let all = write_json(d.path(), "all.json", &read_snapshot(&[]));
    let gap = write_json(d.path(), "gap.json", &read_snapshot(&["triage"]));
    let run = |interact: &Path| {
        bin()
            .args(["--json", "uat", "preflight", "verify"])
            .arg(&r)
            .arg("--declared")
            .arg(&declared)
            .arg("--read")
            .arg(format!("view={}", all.display()))
            .arg("--read")
            .arg(format!("interact={}", interact.display()))
            .arg("--read")
            .arg(format!("admin={}", all.display()))
            .output()
            .unwrap()
    };
    let pass = run(&all);
    assert_eq!(pass.status.code(), Some(0));
    assert_eq!(envelope(&pass.stdout)["data"]["ok"], true);
    let miss = run(&gap);
    assert_eq!(miss.status.code(), Some(1));
    assert_eq!(envelope(&miss.stdout)["data"]["ok"], false);
    let usage = bin()
        .args(["uat", "preflight", "verify"])
        .arg(&r)
        .arg("--declared")
        .arg(&declared)
        .arg("--read")
        .arg("nonsense")
        .output()
        .unwrap();
    assert_eq!(usage.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&usage.stderr).contains("<level>=<file>"));
}
