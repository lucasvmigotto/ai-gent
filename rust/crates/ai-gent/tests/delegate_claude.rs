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
        home: None,
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
    assert!(after("--allowedTools").contains("Bash(cargo test:*)"));
    assert!(!after("--allowedTools").contains("Bash(cargo:*)"));
    assert!(after("--disallowedTools").contains("Bash(git commit:*)"));
    assert!(after("--disallowedTools").contains("Read(//"));
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
    assert_eq!(args[i + 1], "Read,Grep,Glob");
    let j = args.iter().position(|x| x == "--disallowedTools").unwrap();
    for t in ["Edit", "Write", "NotebookEdit", "Bash"] {
        assert!(args[j + 1].split(',').any(|d| d == t), "{t}");
    }
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
fn status_tokens_last_txt_and_the_log_line_come_from_the_result_object() {
    let (d, tmp, profile) = repo();
    let a = adapter(d.path(), &tmp, &profile);
    a.run(&request(d.path(), "c-tally", Role::Builder, 0))
        .unwrap();
    wait_finished(d.path(), "c-tally");
    let st = status::read(d.path(), "c-tally").unwrap();
    assert_eq!(st.events, 1);
    let u = st.tokens.unwrap();
    assert_eq!(
        (u.input_tokens, u.cached_input_tokens, u.output_tokens),
        (1000, 800, 50)
    );
    let last =
        std::fs::read_to_string(d.path().join(".ai-gent/delegate/c-tally/last.txt")).unwrap();
    assert!(last.contains("Done: T010c"));
    let log = std::fs::read_to_string(d.path().join(".ai-gent/delegation.log")).unwrap();
    let line: Value = serde_json::from_str(log.lines().last().unwrap()).unwrap();
    assert_eq!(line["track"], "c-tally");
    assert_eq!(line["tokens"]["input_tokens"], 1000);
    assert_eq!(line["tokens"]["cached_input_tokens"], 800);
    assert_eq!(line["tokens"]["output_tokens"], 50);
}

#[test]
fn a_claude_run_gets_at_least_an_hour_before_it_can_stall() {
    let (d, tmp, profile) = repo();
    let a = adapter(d.path(), &tmp, &profile);
    a.run(&request_mode(d.path(), "c-long", Role::Builder, 0, "stall"))
        .unwrap();
    let launch: Value = serde_json::from_str(
        &std::fs::read_to_string(d.path().join(".ai-gent/delegate/c-long/launch.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(launch["stall_secs"], 3600);
    a.stop("c-long").unwrap();
}

#[test]
fn exit_zero_with_is_error_is_failed_with_a_reason() {
    let (d, tmp, profile) = repo();
    let a = adapter(d.path(), &tmp, &profile);
    a.run(&request_mode(
        d.path(),
        "c-turns",
        Role::Builder,
        0,
        "maxturns",
    ))
    .unwrap();
    let r = wait_finished(d.path(), "c-turns");
    assert_eq!(r.exit_code, Some(0));
    assert_eq!(r.state, State::Failed);
    assert!(r.is_error);
    assert_eq!(r.reason.as_deref(), Some("max-turns"));
    assert_eq!(
        status::read(d.path(), "c-turns").unwrap().state,
        State::Failed
    );
    // a clean run is not an error
    a.run(&request(d.path(), "c-clean", Role::Builder, 0))
        .unwrap();
    let ok = wait_finished(d.path(), "c-clean");
    assert!(!ok.is_error && ok.reason.is_none());
}

#[test]
fn run_without_a_profile_is_refused_but_detect_and_ready_work() {
    let (d, _tmp, profile) = repo();
    let brief = d.path().join("brief.txt");
    std::fs::write(&brief, "Task: T\nMODE:ok").unwrap();
    let cli = |args: &[&str], with_profile: bool| {
        let mut c = Command::new(bin());
        c.args(["delegate", "--agent", "claude", "--root"])
            .arg(d.path())
            .args(args)
            .env("AI_GENT_CLAUDE_BIN", fake())
            .env_remove("AI_GENT_CLAUDE_CONFIG_DIR");
        if with_profile {
            c.env("AI_GENT_CLAUDE_CONFIG_DIR", &profile);
        }
        c.output().unwrap()
    };
    let run_args = [
        "run",
        "--track",
        "c-noprofile",
        "--worktree",
        ".",
        "--brief-file",
        brief.to_str().unwrap(),
    ];
    let out = cli(&run_args, false);
    assert!(!out.status.success());
    assert!(
        String::from_utf8_lossy(&out.stderr)
            .contains("a Claude delegate needs its own profile: set AI_GENT_CLAUDE_CONFIG_DIR"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(!d.path().join(".ai-gent/delegate/c-noprofile").exists());
    assert!(cli(&["detect"], true).status.success());
    assert!(cli(&["ready"], true).status.success());
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
    assert!(
        s.signalled && s.exited,
        "stop outcome: {s:?}; track files: {:?}",
        track_files(d.path(), "c-stall")
    );
    let r = wait_finished(d.path(), "c-stall");
    assert_eq!(r.state, State::TakenBack);
}

/// The track directory's files with their sizes, for a failing assertion's message.
fn track_files(root: &Path, track: &str) -> Vec<String> {
    std::fs::read_dir(root.join(".ai-gent/delegate").join(track))
        .map(|d| {
            d.flatten()
                .map(|e| {
                    let n = e.file_name().to_string_lossy().into_owned();
                    let len = e.metadata().map_or(0, |m| m.len());
                    format!("{n}={len}")
                })
                .collect()
        })
        .unwrap_or_default()
}
