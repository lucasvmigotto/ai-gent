//! `ai-gent delegate plan` and `delegate verify-sources` from fixtures: no agent, no network, no
//! container engine (the disk numbers come from the `wt` test hook).
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::Command;

fn bin() -> PathBuf {
    assert_cmd::cargo::cargo_bin("ai-gent")
}

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/delegate")
}

/// Run `ai-gent --json delegate --root ROOT ARGS`; exit code and stdout as JSON.
fn run(root: &Path, usage_gb: Option<(u64, u64)>, args: &[&str]) -> (i32, Value) {
    let mut c = Command::new(bin());
    c.args(["--json", "delegate", "--root"])
        .arg(root)
        .args(args)
        .env("NO_COLOR", "1");
    match usage_gb {
        Some((total, free)) => {
            c.env("CONTAINER_ENGINE", "fake-engine").env(
                "AI_GENT_WT_FAKE_USAGE",
                format!("{},{},/", total * 1_000_000_000, free * 1_000_000_000),
            );
        }
        None => {
            c.env("AI_GENT_WT_NO_ENGINE", "1");
        }
    }
    let out = c.output().unwrap();
    let v = serde_json::from_slice(&out.stdout).unwrap_or(Value::Null);
    (out.status.code().unwrap(), v)
}

fn track_status(root: &Path, track: &str, state: &str) {
    let dir = root.join(".ai-gent/delegate").join(track);
    std::fs::create_dir_all(&dir).unwrap();
    let s = json!({"track": track, "agent": "codex", "role": "builder", "tasks": ["T1"],
        "state": state, "started": 1, "updated": 1, "events": 0, "worktree": ".worktrees/x",
        "verdict": {"result": "pending", "reason": null}, "tokens": null});
    std::fs::write(dir.join("status.json"), s.to_string()).unwrap();
}

fn mixed() -> String {
    fixtures()
        .join("schedule/mixed.json")
        .to_string_lossy()
        .into_owned()
}

fn ids(v: &Value) -> Vec<&str> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|t| t["id"].as_str().or(t["track"].as_str()).unwrap())
        .collect()
}

#[test]
fn plan_assigns_tracks_and_admits_up_to_the_default_cap() {
    let d = tempfile::tempdir().unwrap();
    let (code, v) = run(
        d.path(),
        Some((1000, 900)),
        &["plan", "--tasks-file", &mixed()],
    );
    assert_eq!(code, 0, "{v}");
    let data = &v["data"];
    assert_eq!(data["pool"]["cap"], 3);
    assert_eq!(data["pool"]["live"], 0);
    assert_eq!(data["slots"], 3);
    assert_eq!(ids(&data["start"]), ["a1", "b1", "c1"]);
    assert_eq!(data["tracks"].as_array().unwrap().len(), 7);
    let t = &data["tracks"];
    assert_eq!(t[0]["worker"], "delegate");
    assert_eq!(t[0]["agent"], "codex");
    assert_eq!(t[0]["tasks"], json!(["A1", "A2"]));
    assert_eq!(t[2]["worker"], "claude");
    assert!(t[2]["reason"].as_str().unwrap().contains("credentials"));
    let deferred = data["deferred"].as_array().unwrap();
    assert_eq!(deferred.len(), 4);
    assert!(deferred.iter().all(|x| x["because"] == "cap"));
}

#[test]
fn live_tracks_and_the_cap_come_from_the_project_files() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join(".ai-gent")).unwrap();
    std::fs::write(d.path().join(".ai-gent/parallel.yaml"), "max_parallel: 4\n").unwrap();
    track_status(d.path(), "feat-a", "running");
    track_status(d.path(), "feat-b", "stalled");
    track_status(d.path(), "feat-c", "done");
    std::fs::create_dir_all(d.path().join(".ai-gent/delegate/broken")).unwrap();
    std::fs::write(d.path().join(".ai-gent/delegate/broken/status.json"), "{").unwrap();
    let (code, v) = run(
        d.path(),
        Some((1000, 900)),
        &["plan", "--tasks-file", &mixed()],
    );
    assert_eq!(code, 0, "{v}");
    let pool = &v["data"]["pool"];
    assert_eq!(pool["cap"], 4);
    assert_eq!(pool["live"], 2);
    assert_eq!(pool["unreadable_status_files"], 1);
    assert_eq!(v["data"]["slots"], 2);
    assert_eq!(ids(&v["data"]["start"]), ["a1", "b1"]);
}

#[test]
fn the_disk_floor_binds_before_the_cap_and_is_named() {
    let d = tempfile::tempdir().unwrap();
    // 1000 GB store: floor max(20, 100) = 100 GB; 130 GB free at 10 GB per track leaves room for 3,
    // with 2 live (max_volumes 9 lifts the cap) only one more fits.
    std::fs::create_dir_all(d.path().join(".ai-gent")).unwrap();
    std::fs::write(d.path().join(".ai-gent/parallel.yaml"), "max_volumes: 9\n").unwrap();
    track_status(d.path(), "feat-a", "running");
    track_status(d.path(), "feat-b", "queued");
    let (code, v) = run(
        d.path(),
        Some((1000, 130)),
        &["plan", "--tasks-file", &mixed()],
    );
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["data"]["slots"], 1);
    assert_eq!(v["data"]["pool"]["floor_gb"], 100.0);
    assert_eq!(v["data"]["pool"]["free_gb"], 130.0);
    let deferred = v["data"]["deferred"].as_array().unwrap();
    assert_eq!(deferred.len(), 6);
    assert!(deferred.iter().all(|x| x["because"] == "disk-floor"));
    // the flag changes the floor for the call, never the cap
    let (_, v) = run(
        d.path(),
        Some((1000, 130)),
        &["plan", "--tasks-file", &mixed(), "--min-free-gb", "50"],
    );
    assert_eq!(v["data"]["slots"], 6); // (130 - 50) / 10 = 8 fit, 2 live; the cap of 9 would allow 7
}

#[test]
fn without_an_engine_only_the_cap_binds() {
    let d = tempfile::tempdir().unwrap();
    let (code, v) = run(d.path(), None, &["plan", "--tasks-file", &mixed()]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["data"]["pool"]["disk_checked"], false);
    assert_eq!(v["data"]["pool"]["free_gb"], Value::Null);
    assert_eq!(v["data"]["slots"], 3);
}

#[test]
fn no_allowed_agent_keeps_everything_with_claude_and_a_bad_list_is_refused() {
    let d = tempfile::tempdir().unwrap();
    let (_, v) = run(
        d.path(),
        None,
        &[
            "plan",
            "--tasks-file",
            &mixed(),
            "--allow-agent",
            "opencode",
        ],
    );
    assert_eq!(v["data"]["tracks"][0]["agent"], "opencode");
    let bad = d.path().join("bad.json");
    std::fs::write(&bad, r#"[{"id":"X","paths":[]}]"#).unwrap();
    let (code, v) = run(
        d.path(),
        None,
        &["plan", "--tasks-file", bad.to_str().unwrap()],
    );
    assert_eq!(code, 1);
    assert_eq!(v["ok"], false);
    let tainted = fixtures().join("schedule/tainted.json");
    let (code, v) = run(
        d.path(),
        None,
        &["plan", "--tasks-file", tainted.to_str().unwrap()],
    );
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["data"]["tracks"].as_array().unwrap().len(), 1);
}

#[test]
fn verify_sources_confirms_flags_and_reports_what_cannot_be_read() {
    let d = tempfile::tempdir().unwrap();
    let a = d.path().join("advisory.md");
    std::fs::write(
        &a,
        "RUSTSEC-2020-0010 affects tiberius 0.3 only. Last push 2026-09-25.",
    )
    .unwrap();
    let a = a.to_str().unwrap();
    let (code, v) = run(
        d.path(),
        None,
        &[
            "verify-sources",
            "--claim",
            "`tiberius` RUSTSEC-2020-0010 covers 0.3 only",
            "--source",
            a,
        ],
    );
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["data"]["outcome"], "confirmed");
    let (code, v) = run(
        d.path(),
        None,
        &[
            "verify-sources",
            "--claim",
            "`tiberius` repository archived in 2021",
            "--source",
            a,
        ],
    );
    assert_eq!(code, 4, "{v}");
    assert_eq!(v["data"]["outcome"], "partial");
    let (code, v) = run(
        d.path(),
        None,
        &[
            "verify-sources",
            "--claim",
            "v9.9",
            "--source",
            "/no/such/source",
        ],
    );
    assert_eq!(code, 4);
    assert_eq!(v["data"]["outcome"], "unfetched");
}

#[test]
fn verify_sources_fetches_a_url_through_curl() {
    use std::io::{Read, Write};
    if Command::new("curl").arg("--version").output().is_err() {
        return; // no curl here: the comparison core is covered by the unit tests
    }
    let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/doc", l.local_addr().unwrap());
    let h = std::thread::spawn(move || {
        let (mut s, _) = l.accept().unwrap();
        let mut buf = [0u8; 1024];
        let _ = s.read(&mut buf);
        let body = "crate foo-bar is at 1.4.2";
        write!(
            s,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        )
        .unwrap();
    });
    let d = tempfile::tempdir().unwrap();
    let (code, v) = run(
        d.path(),
        None,
        &[
            "verify-sources",
            "--claim",
            "`foo-bar` is at 1.4.2",
            "--source",
            &url,
        ],
    );
    h.join().unwrap();
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["data"]["outcome"], "confirmed");
}
