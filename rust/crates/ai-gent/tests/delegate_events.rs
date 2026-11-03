//! Completion events (spec 041, T022): `delegate events`, `watch --once`, `ack` and `idle`, against
//! temp repositories and a fake agent (`sh -c`): no network, no real agent, no container engine.
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn bin() -> PathBuf {
    assert_cmd::cargo::cargo_bin("ai-gent")
}

fn cli(root: &Path, args: &[&str]) -> Output {
    Command::new(bin())
        .args(["delegate", "--root"])
        .arg(root)
        .args(args)
        .env("NO_COLOR", "1")
        .env("AI_GENT_WT_NO_ENGINE", "1")
        .output()
        .unwrap()
}

fn stdout(root: &Path, args: &[&str]) -> String {
    let o = cli(root, args);
    assert!(o.status.success(), "{args:?}: {o:?}");
    String::from_utf8(o.stdout).unwrap()
}

fn write_status(root: &Path, track: &str, state: &str, started: u64, updated: u64, tasks: &[&str]) {
    let dir = root.join(".ai-gent/delegate").join(track);
    std::fs::create_dir_all(&dir).unwrap();
    let s = json!({"track": track, "agent": "codex", "role": "builder", "tasks": tasks,
        "state": state, "started": started, "updated": updated, "events": 0,
        "worktree": ".worktrees/x", "verdict": {"result": "pending", "reason": null},
        "tokens": null});
    std::fs::write(dir.join("status.json"), s.to_string()).unwrap();
}

fn lines(root: &Path, file: &str) -> Vec<Value> {
    std::fs::read_to_string(root.join(".ai-gent/delegate").join(file))
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

#[cfg(unix)]
fn fake_agent(root: &Path, track: &str, script: &str) {
    let dir = root.join(".ai-gent/delegate").join(track);
    let l = json!({"program": "sh", "args": ["-c", script], "cwd": root, "env": [],
        "stall_secs": 600});
    std::fs::write(dir.join("launch.json"), l.to_string()).unwrap();
}

#[cfg(unix)]
fn watcher(root: &Path, track: &str) {
    let dir = root.join(".ai-gent/delegate").join(track);
    let o = Command::new(bin())
        .args(["delegate", "_watch"])
        .arg(&dir)
        .output()
        .unwrap();
    assert!(o.status.success(), "{o:?}");
}

#[cfg(unix)]
#[test]
fn the_watcher_appends_one_event_per_transition_even_when_it_restarts() {
    let d = tempfile::tempdir().unwrap();
    write_status(d.path(), "ok", "running", 10, 10, &["T1", "T2"]);
    fake_agent(d.path(), "ok", r#"echo '{"type":"thread.started"}'"#);
    write_status(d.path(), "bad", "running", 10, 10, &["T3"]);
    fake_agent(d.path(), "bad", "exit 3");
    for _ in 0..2 {
        watcher(d.path(), "ok");
        watcher(d.path(), "bad");
    }
    let ev = lines(d.path(), "events.jsonl");
    assert_eq!(ev.len(), 2, "{ev:?}");
    let by = |t: &str| ev.iter().find(|e| e["track"] == t).unwrap().clone();
    assert_eq!(by("ok")["id"], "ok:10:done");
    assert_eq!(by("ok")["tasks"], json!(["T1", "T2"]));
    assert_eq!(by("bad")["id"], "bad:10:failed");
    assert_eq!(by("bad")["state"], "failed");
    for k in ["ts", "agent", "role", "result"] {
        assert!(by("ok").get(k).is_some(), "missing {k}");
    }
    // The event carries no brief, message or absolute path.
    let raw = std::fs::read_to_string(d.path().join(".ai-gent/delegate/events.jsonl")).unwrap();
    assert!(!raw.contains(d.path().to_str().unwrap()), "{raw}");
}

#[test]
fn an_event_of_an_earlier_run_is_told_apart_and_dupes_are_refused() {
    let d = tempfile::tempdir().unwrap();
    write_status(d.path(), "a", "done", 1, 2, &["T1"]);
    stdout(d.path(), &["watch", "--once"]);
    stdout(d.path(), &["watch", "--once"]);
    // The track runs again and finishes again: a new `started`, a new id.
    write_status(d.path(), "a", "done", 50, 60, &["T1"]);
    stdout(d.path(), &["watch", "--once"]);
    let ids: Vec<_> = lines(d.path(), "events.jsonl")
        .iter()
        .map(|e| e["id"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(ids, ["a:1:done", "a:50:done"]);
}

#[test]
fn a_silent_running_track_is_announced_stalled_once() {
    let d = tempfile::tempdir().unwrap();
    write_status(d.path(), "s", "running", 1, 1, &["T1"]);
    for _ in 0..2 {
        stdout(d.path(), &["watch", "--once"]);
    }
    let ev = lines(d.path(), "events.jsonl");
    assert_eq!(ev.len(), 1);
    assert_eq!(ev[0]["id"], "s:1:stalled");
}

#[test]
fn watch_once_prints_pending_events_and_ack_removes_them() {
    let d = tempfile::tempdir().unwrap();
    write_status(d.path(), "a", "done", 7, 8, &["T1", "T2"]);
    write_status(d.path(), "b", "failed", 7, 8, &["T3"]);
    write_status(d.path(), "c", "taken-back", 7, 8, &["T4"]);
    let out = stdout(d.path(), &["watch", "--once"]);
    let rows: Vec<&str> = out.lines().collect();
    assert_eq!(rows.len(), 3, "{out}");
    assert!(rows[0].starts_with("event id=a:7:done ts="), "{}", rows[0]);
    assert!(
        rows[0].ends_with("track=a agent=codex role=builder tasks=T1,T2 state=done result=-"),
        "{}",
        rows[0]
    );

    let ack = stdout(d.path(), &["ack", "a:7:done"]);
    assert!(ack.contains("a:7:done"), "{ack}");
    // A restarted coordinator recovers only what it has not acknowledged.
    let replay = stdout(d.path(), &["watch", "--once", "--since-ack"]);
    assert_eq!(replay.lines().count(), 2);
    assert!(!replay.contains("a:7:done"));
    let o = cli(d.path(), &["ack", "nope"]);
    assert!(!o.status.success());
    stdout(d.path(), &["ack", "--all"]);
    assert_eq!(stdout(d.path(), &["watch", "--once"]), "");
    assert_eq!(lines(d.path(), "acks.jsonl").len(), 3);
    // The log itself is append-only: acknowledging does not touch it.
    assert_eq!(lines(d.path(), "events.jsonl").len(), 3);
}

#[test]
fn watch_once_json_is_one_object_per_line() {
    let d = tempfile::tempdir().unwrap();
    write_status(d.path(), "a", "done", 7, 8, &["T1"]);
    let o = Command::new(bin())
        .args(["--json", "delegate", "--root"])
        .arg(d.path())
        .args(["watch", "--once"])
        .output()
        .unwrap();
    let text = String::from_utf8(o.stdout).unwrap();
    assert_eq!(text.lines().count(), 1);
    let v: Value = serde_json::from_str(text.trim()).unwrap();
    assert_eq!(v["id"], "a:7:done");
    assert_eq!(v["state"], "done");
}

fn idle(root: &Path, tasks: &Value) -> String {
    let f = root.join("tasks.json");
    std::fs::write(&f, tasks.to_string()).unwrap();
    stdout(root, &["idle", "--tasks-file", f.to_str().unwrap()])
        .trim()
        .to_string()
}

fn task(id: &str, path: &str, extra: &Value) -> Value {
    let mut t = json!({"id": id, "kind": "docs", "paths": [path]});
    for (k, v) in extra.as_object().unwrap() {
        t[k] = v.clone();
    }
    t
}

#[test]
fn idle_says_why_free_slots_are_not_used() {
    let d = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(d.path().join(".ai-gent")).unwrap();
    let none = json!({});
    // Ready work and a free slot: the coordinator should dispatch.
    let list = json!([task("T1", "docs/a.md", &none)]);
    assert_eq!(
        idle(d.path(), &list),
        "free_slots=3 ready_tasks=1 reason=dispatch"
    );
    // T1 is taken by a running track: nothing else is ready.
    write_status(d.path(), "t1", "running", 1, u64::MAX / 2, &["T1"]);
    assert_eq!(
        idle(d.path(), &list),
        "free_slots=2 ready_tasks=0 reason=none-ready"
    );
    // T2 waits for T1; T3 waits for an owner decision.
    let list = json!([
        task("T1", "docs/a.md", &none),
        task("T2", "docs/b.md", &json!({"depends": ["T1"]})),
    ]);
    assert_eq!(
        idle(d.path(), &list),
        "free_slots=2 ready_tasks=0 reason=dependency"
    );
    let list = json!([
        task("T1", "docs/a.md", &none),
        task(
            "T3",
            "docs/c.md",
            &json!({"needs": {"owner_decision": true}})
        ),
    ]);
    assert_eq!(
        idle(d.path(), &list),
        "free_slots=2 ready_tasks=0 reason=approval"
    );
    // Once T1 is done, T2 is ready.
    write_status(d.path(), "t1", "done", 1, 2, &["T1"]);
    let list = json!([
        task("T1", "docs/a.md", &none),
        task("T2", "docs/b.md", &json!({"depends": ["T1"]})),
    ]);
    assert_eq!(
        idle(d.path(), &list),
        "free_slots=3 ready_tasks=1 reason=dispatch"
    );
    // No free slot: the cap is the reason.
    std::fs::write(d.path().join(".ai-gent/parallel.yaml"), "max_parallel: 1\n").unwrap();
    write_status(d.path(), "other", "running", 1, u64::MAX / 2, &["T9"]);
    assert_eq!(
        idle(d.path(), &list),
        "free_slots=0 ready_tasks=1 reason=cap"
    );
    // It never dispatched anything.
    assert!(!d.path().join(".ai-gent/delegate/t2").exists());
}
