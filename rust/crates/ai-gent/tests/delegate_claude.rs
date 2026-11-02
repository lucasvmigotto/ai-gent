//! The Claude-profile adapter against a fake `claude` (`examples/fake_claude.rs`): no real Claude call
//! and no network. Covers stdin closed, the profile as `CLAUDE_CONFIG_DIR`, the role's tool lists, the
//! single end-of-run JSON read as message and usage, no first-event wait, and the PID-file stop.
#![allow(clippy::many_single_char_names)]
use ai_gent_core::delegate::adapter::{Adapter, RunRequest, track_result};
use ai_gent_core::delegate::claude::ClaudeAdapter;
use ai_gent_core::delegate::status::{self, Role, State};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

fn fake() -> PathBuf {
    let exe = std::env::current_exe().unwrap();
    let p = exe
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples")
        .join(format!("fake_claude{}", std::env::consts::EXE_SUFFIX));
    assert!(p.exists(), "{} (cargo test builds examples)", p.display());
    p
}

fn bin() -> PathBuf {
    assert_cmd::cargo::cargo_bin("ai-gent")
}

fn git(dir: &Path, args: &[&str]) {
    let s = Command::new("git")
        .args(args)
        .current_dir(dir)
        .status()
        .unwrap();
    assert!(s.success(), "{args:?}");
}

fn repo() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let d = tempfile::tempdir().unwrap();
    git(d.path(), &["init", "-q", "-b", "main"]);
    let tmp = d.path().join("scratch-tmp");
    std::fs::create_dir_all(&tmp).unwrap();
    let profile = d.path().join("profile");
    std::fs::create_dir_all(&profile).unwrap();
    (d, tmp, profile)
}

fn adapter(root: &Path, tmp: &Path, profile: &Path) -> ClaudeAdapter {
    ClaudeAdapter {
        program: fake(),
        root: root.to_path_buf(),
        watch_exe: bin(),
        tmpdir: tmp.to_path_buf(),
        config_dir: Some(profile.to_path_buf()),
        max_turns: 12,
    }
}

fn request(root: &Path, track: &str, role: Role, wait: u64) -> RunRequest {
    request_mode(root, track, role, wait, "ok")
}

fn request_mode(root: &Path, track: &str, role: Role, wait: u64, mode: &str) -> RunRequest {
    RunRequest {
        track: track.into(),
        worktree: root.to_path_buf(),
        brief: format!("Task: T010c do the thing.\nallowed_paths: fake-output.txt\nMODE:{mode}"),
        tasks: vec!["T010c".into()],
        role,
        stall_secs: 600,
        first_event_wait: Duration::from_secs(wait),
    }
}

fn wait_finished(root: &Path, track: &str) -> ai_gent_core::delegate::adapter::RunResult {
    let t = Instant::now();
    loop {
        let r = track_result(root, track).unwrap();
        if r.finished {
            return r;
        }
        assert!(
            t.elapsed() < Duration::from_secs(60),
            "agent did not finish"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// The `fake` object of the run's single event.
fn fake_report(root: &Path, track: &str) -> Value {
    let text = std::fs::read_to_string(
        root.join(".ai-gent/delegate")
            .join(track)
            .join("events.jsonl"),
    )
    .unwrap();
    assert_eq!(text.lines().count(), 1, "one JSON object when the run ends");
    let v: Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
    v["fake"].clone()
}

#[test]
fn detect_ready_and_preflight() {
    let (d, tmp, profile) = repo();
    let a = adapter(d.path(), &tmp, &profile);
    assert_eq!(a.id(), "claude");
    let det = a.detect();
    assert!(det.found);
    assert_eq!(det.version.as_deref(), Some("2.0.0-fake (Claude Code)"));
    a.ready().unwrap();
    let p = a.preflight(d.path());
    assert!(p.ok, "{p:?}");
    assert!(
        p.checks
            .iter()
            .any(|c| c.name == "profile_dir_exists" && c.ok)
    );
    assert!(p.notes.iter().any(|n| n.contains("does not commit")));
    let mut no_profile = adapter(d.path(), &tmp, &d.path().join("absent"));
    assert!(!no_profile.preflight(d.path()).ok);
    no_profile.program = d.path().join("no-such-claude");
    assert!(!no_profile.detect().found);
    assert!(!a.preflight(&d.path().join("missing")).ok);
}

#[test]
fn not_logged_in_is_reported_not_worked_around() {
    let (d, _tmp, profile) = repo();
    let out = Command::new(bin())
        .args(["delegate", "--agent", "claude", "--root"])
        .arg(d.path())
        .arg("ready")
        .env("AI_GENT_CLAUDE_BIN", fake())
        .env("AI_GENT_CLAUDE_CONFIG_DIR", &profile)
        .env("FAKE_LOGGED_IN", "0")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("claude is not logged in"));
}

#[test]
fn a_builder_run_reads_message_and_usage_from_the_json_result() {
    let (d, tmp, profile) = repo();
    let a = adapter(d.path(), &tmp, &profile);
    // a first-event wait is requested, yet the single event only comes at the end: it must not stop the run
    let h = a
        .run(&request_mode(d.path(), "c-build", Role::Builder, 1, "slow"))
        .unwrap();
    assert_eq!(h.track, "c-build");
    let r = wait_finished(d.path(), "c-build");
    assert_eq!(r.state, State::Done);
    assert_eq!(r.exit_code, Some(0));
    let r = a.result("c-build").unwrap();
    assert!(r.message.unwrap().contains("Done: T010c"));
    let u = r.usage.unwrap();
    assert_eq!(
        (u.input_tokens, u.cached_input_tokens, u.output_tokens),
        (1000, 800, 50)
    );
    let f = fake_report(d.path(), "c-build");
    assert_eq!(f["stdin_bytes"], 0, "stdin is closed");
    assert_eq!(
        f["config_dir"],
        profile.to_string_lossy().replace('\\', "/")
    );
    assert_eq!(f["tmpdir"], tmp.to_string_lossy().replace('\\', "/"));
    let brief = request_mode(d.path(), "c-build", Role::Builder, 0, "slow").brief;
    assert_eq!(f["brief_len"].as_u64().unwrap(), brief.len() as u64);
    let args: Vec<String> = serde_json::from_value(f["args"].clone()).unwrap();
    assert_eq!(args[0], "-p");
    assert!(args.windows(2).any(|w| w == ["--output-format", "json"]));
    assert!(args.windows(2).any(|w| w == ["--max-turns", "12"]));
    let after = |flag: &str| {
        let i = args.iter().position(|x| x == flag).unwrap();
        args[i + 1].clone()
    };
    assert!(after("--allowedTools").contains("Edit"));
    assert!(after("--allowedTools").contains("Bash(cargo:*)"));
    assert!(after("--disallowedTools").contains("Bash(git commit:*)"));
    let st = status::read(d.path(), "c-build").unwrap();
    assert_eq!(st.agent, "claude");
    assert_eq!(st.state, State::Done);
    assert!(
        !d.path()
            .join(".ai-gent/delegate/c-build/agent.pid")
            .exists()
    );
}

#[test]
fn a_reviewer_run_is_read_only() {
    let (d, tmp, profile) = repo();
    let a = adapter(d.path(), &tmp, &profile);
    a.run(&request(d.path(), "c-review", Role::Reviewer, 0))
        .unwrap();
    wait_finished(d.path(), "c-review");
    let f = fake_report(d.path(), "c-review");
    let args: Vec<String> = serde_json::from_value(f["args"].clone()).unwrap();
    let i = args.iter().position(|x| x == "--allowedTools").unwrap();
    assert!(!args[i + 1].contains("Edit") && !args[i + 1].contains("Write"));
    let j = args.iter().position(|x| x == "--disallowedTools").unwrap();
    assert_eq!(args[j + 1], "Edit,Write");
}

#[test]
fn a_failing_run_is_failed_with_its_exit_code() {
    let (d, tmp, profile) = repo();
    let a = adapter(d.path(), &tmp, &profile);
    a.run(&request_mode(d.path(), "c-fail", Role::Builder, 0, "fail"))
        .unwrap();
    let r = wait_finished(d.path(), "c-fail");
    assert_eq!(r.state, State::Failed);
    assert_eq!(r.exit_code, Some(1));
}

#[test]
fn stop_uses_the_pid_file() {
    let (d, tmp, profile) = repo();
    let a = adapter(d.path(), &tmp, &profile);
    a.run(&request_mode(
        d.path(),
        "c-stall",
        Role::Builder,
        0,
        "stall",
    ))
    .unwrap();
    let pid_file = d.path().join(".ai-gent/delegate/c-stall/agent.pid");
    let t = Instant::now();
    while !pid_file.exists() {
        assert!(t.elapsed() < Duration::from_secs(20), "no PID file");
        std::thread::sleep(Duration::from_millis(25));
    }
    assert!(a.status("c-stall").unwrap().running);
    let s = a.stop("c-stall").unwrap();
    assert!(s.signalled && s.exited);
    let r = wait_finished(d.path(), "c-stall");
    assert_eq!(r.state, State::TakenBack);
}
