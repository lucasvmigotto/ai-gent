//! `ai-gent delegate` against a fake agent (`examples/fake_agent.rs`): no real Codex and no network.
//! Covers the Codex adapter (stdin closed, PID-file stop, progress and usage from events, a delegate
//! that cannot commit) and the CLI surface of the brief, the checks, the commit and the status files.
#![allow(
    clippy::too_many_lines,
    clippy::many_single_char_names,
    clippy::cast_possible_truncation
)]
use ai_gent_core::delegate::adapter::{Adapter, RunRequest, track_result};
use ai_gent_core::delegate::codex::CodexAdapter;
use ai_gent_core::delegate::status::{self, Role, State};
use ai_gent_core::delegate::{commit, gitx};
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
        .join(format!(
            "ai-gent-fake-agent{}",
            std::env::consts::EXE_SUFFIX
        ));
    assert!(p.exists(), "{} (cargo test builds examples)", p.display());
    p
}

fn bin() -> PathBuf {
    assert_cmd::cargo::cargo_bin("ai-gent")
}

fn git(dir: &Path, args: &[&str]) {
    let s = Command::new("git")
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(dir)
        .status()
        .unwrap();
    assert!(s.success(), "{args:?}");
}

/// A repository with one commit; returns it with a scratch TMPDIR.
fn repo() -> (tempfile::TempDir, PathBuf) {
    let d = tempfile::tempdir().unwrap();
    git(d.path(), &["init", "-q", "-b", "main"]);
    // the supervisor commit runs plain `git commit`: give the repository its own identity
    git(d.path(), &["config", "user.name", "t"]);
    git(d.path(), &["config", "user.email", "t@t"]);
    git(d.path(), &["config", "commit.gpgsign", "false"]);
    std::fs::write(d.path().join("seed.txt"), "seed\n").unwrap();
    git(d.path(), &["add", "seed.txt"]);
    git(d.path(), &["commit", "-q", "-m", "chore: seed"]);
    let tmp = d.path().join("scratch-tmp");
    std::fs::create_dir_all(&tmp).unwrap();
    (d, tmp)
}

fn adapter(root: &Path, tmp: &Path) -> CodexAdapter {
    CodexAdapter {
        program: fake(),
        root: root.to_path_buf(),
        watch_exe: bin(),
        tmpdir: tmp.to_path_buf(),
        add_git_dir: false,
    }
}

fn request(root: &Path, track: &str, wait: u64) -> RunRequest {
    RunRequest {
        track: track.into(),
        worktree: root.to_path_buf(),
        brief: "Task: T010 do the thing.\nallowed_paths: fake-output.txt".into(),
        tasks: vec!["T010".into()],
        role: Role::Builder,
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

/// Run `ai-gent delegate` with a fake-agent mode; stdout as JSON.
fn cli(root: &Path, mode: &str, args: &[&str]) -> (i32, Value) {
    // stdout goes to a file, not a pipe: on Windows the background watcher and agent inherit the
    // CLI's handles, and a pipe would stay open (so `output()` would block) until the agent ends.
    let out_path = root
        .join("scratch-tmp")
        .join(format!("cli-out-{}.txt", std::process::id()));
    let out_file = std::fs::File::create(&out_path).unwrap();
    let status = Command::new(bin())
        .arg("--json")
        .arg("delegate")
        .arg("--root")
        .arg(root)
        .args(args)
        .env("AI_GENT_CODEX_BIN", fake())
        .env("FAKE_AGENT_MODE", mode)
        .env("TMPDIR", root.join("scratch-tmp"))
        .stdin(std::process::Stdio::null())
        .stdout(out_file)
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    let text = std::fs::read_to_string(&out_path).unwrap_or_default();
    let _ = std::fs::remove_file(&out_path);
    let v = serde_json::from_str(text.lines().next().unwrap_or("null")).unwrap_or(Value::Null);
    (status.code().unwrap(), v)
}

fn events(root: &Path, track: &str) -> Vec<Value> {
    std::fs::read_to_string(
        root.join(".ai-gent/delegate")
            .join(track)
            .join("events.jsonl"),
    )
    .unwrap()
    .lines()
    .map(|l| serde_json::from_str(l).unwrap())
    .collect()
}

#[test]
fn detect_ready_and_preflight() {
    let (d, tmp) = repo();
    let a = adapter(d.path(), &tmp);
    let det = a.detect();
    assert!(det.found);
    assert_eq!(det.version.as_deref(), Some("codex-cli 0.0.0-fake"));
    a.ready().unwrap();
    // a worktree: here the repository itself (it has .git)
    let p = a.preflight(d.path());
    assert!(p.ok, "{p:?}");
    assert!(
        p.checks
            .iter()
            .any(|c| c.name == "agent_starts_with_stdin_closed" && c.ok)
    );
    assert!(p.notes.iter().any(|n| n.contains("does not commit")));
    // no probe files left behind
    assert!(std::fs::read_dir(d.path()).unwrap().all(|e| {
        !e.unwrap()
            .file_name()
            .to_string_lossy()
            .contains("preflight")
    }));
    let bad = a.preflight(&d.path().join("missing"));
    assert!(!bad.ok);
    let mut missing = adapter(d.path(), &tmp);
    missing.program = d.path().join("no-such-codex");
    assert!(!missing.detect().found);
}

#[test]
fn not_logged_in_is_reported_not_worked_around() {
    let (d, _tmp) = repo();
    let out = Command::new(bin())
        .args(["delegate", "--root"])
        .arg(d.path())
        .arg("ready")
        .env("AI_GENT_CODEX_BIN", fake())
        .env("FAKE_LOGGED_IN", "0")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("codex is not logged in"));
}

#[test]
fn a_run_streams_progress_and_usage_with_stdin_closed() {
    let (d, tmp) = repo();
    let a = adapter(d.path(), &tmp);
    let h = a.run(&request(d.path(), "feat-x", 30)).unwrap();
    assert_eq!(h.track, "feat-x");
    let r = wait_finished(d.path(), "feat-x");
    // the fake reads stdin to its end first: finishing at all proves stdin was closed
    assert_eq!(r.state, State::Done);
    assert_eq!(r.exit_code, Some(0));
    assert!(r.message.unwrap().contains("Done: T010"));
    let u = r.usage.unwrap();
    assert_eq!(
        (
            u.input_tokens,
            u.cached_input_tokens,
            u.output_tokens,
            u.reasoning_output_tokens
        ),
        (1000, 800, 50, 10)
    );
    let ev = events(d.path(), "feat-x");
    let env = ev.iter().find(|e| e["type"] == "fake.env").unwrap();
    assert_eq!(env["stdin_bytes"], 0);
    assert_eq!(env["tmpdir"], tmp.to_string_lossy().replace('\\', "/"));
    let brief = request(d.path(), "feat-x", 0).brief;
    assert_eq!(
        env["brief_len"].as_u64().unwrap() as usize,
        brief.len(),
        "the brief reaches the agent whole"
    );
    let args: Vec<String> = serde_json::from_value(env["args"].clone()).unwrap();
    assert_eq!(&args[..1], ["exec"]);
    assert!(args.windows(2).any(|w| w == ["-s", "workspace-write"]));
    assert!(args.iter().any(|a| a == "--json"));
    assert_eq!(
        args.iter().filter(|a| *a == "--add-dir").count(),
        1,
        "only TMPDIR; the git dir stays closed"
    );

    // progress from item events; the single error event in the middle is not a stall
    let s = a.status("feat-x").unwrap();
    assert_eq!((s.items_started, s.items_completed), (3, 3));
    assert!(!s.running);
    assert_eq!(s.events, ev.len() as u64);

    // the status file and the log line
    let st = status::read(d.path(), "feat-x").unwrap();
    assert_eq!(st.state, State::Done);
    assert_eq!(st.agent, "codex");
    assert_eq!(st.tokens.unwrap().input_tokens, 1000);
    let log = status::read_log(d.path()).unwrap();
    assert_eq!(log.len(), 1);
    assert_eq!(log[0].tags, ["delegate"]);
    assert_eq!(log[0].tokens.unwrap().output_tokens, 50);
    assert!(!d.path().join(".ai-gent/delegate/feat-x/agent.pid").exists());
}

#[test]
fn a_failing_agent_is_failed_with_its_exit_code() {
    let (d, _tmp) = repo();
    let (code, v) = cli(
        d.path(),
        "fail",
        &[
            "run",
            "--track",
            "t-fail",
            "--worktree",
            d.path().to_str().unwrap(),
            "--brief-file",
            &brief_file(d.path()),
            "--tasks",
            "T010",
        ],
    );
    assert_eq!(code, 0, "{v}");
    let r = wait_finished(d.path(), "t-fail");
    assert_eq!((r.state, r.exit_code), (State::Failed, Some(3)));
}

fn brief_file(root: &Path) -> String {
    let p = root.join("brief-in.txt");
    std::fs::write(&p, "Task: T010 something.\n").unwrap();
    p.to_string_lossy().into_owned()
}

#[test]
fn stop_uses_the_pid_file_and_never_a_pattern() {
    let (d, _tmp) = repo();
    // a look-alike process (same program, same arguments) that a pattern kill would also hit
    let mut decoy = Command::new(fake())
        .args(["exec", "--cd", ".", "--json", "x"])
        .env("FAKE_AGENT_MODE", "stall")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .spawn()
        .unwrap();
    let (code, v) = cli(
        d.path(),
        "stall",
        &[
            "run",
            "--track",
            "t-stall",
            "--worktree",
            d.path().to_str().unwrap(),
            "--brief-file",
            &brief_file(d.path()),
            "--tasks",
            "T010",
        ],
    );
    assert_eq!(code, 0, "{v}");
    let pid = v["data"]["pid"].as_u64().unwrap();
    let (_, s) = cli(d.path(), "stall", &["status", "t-stall"]);
    assert_eq!(s["data"]["running"], true);
    assert_eq!(s["data"]["items_started"], 1);
    let recorded: u64 =
        std::fs::read_to_string(d.path().join(".ai-gent/delegate/t-stall/agent.pid"))
            .unwrap()
            .trim()
            .parse()
            .unwrap();
    assert_eq!(recorded, pid);

    let (code, v) = cli(d.path(), "stall", &["stop", "t-stall"]);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["data"]["pid"].as_u64(), Some(pid));
    assert_eq!(v["data"]["signalled"], true);
    assert_eq!(v["data"]["exited"], true);
    let r = wait_finished(d.path(), "t-stall");
    assert_eq!(r.state, State::TakenBack);
    assert!(
        decoy.try_wait().unwrap().is_none(),
        "the look-alike must survive"
    );
    decoy.kill().unwrap();
    let _ = decoy.wait();
    // a second stop finds nothing to do
    let (_, v) = cli(d.path(), "stall", &["stop", "t-stall"]);
    assert_eq!(v["data"]["signalled"], false);
    assert_eq!(
        status::read_log(d.path()).unwrap()[0].result,
        State::TakenBack
    );
}

#[test]
fn a_silent_agent_is_stopped_when_no_first_event_comes() {
    let (d, tmp) = repo();
    let _ = tmp;
    let (code, v) = cli(
        d.path(),
        "silent",
        &[
            "run",
            "--track",
            "t-silent",
            "--worktree",
            d.path().to_str().unwrap(),
            "--brief-file",
            &brief_file(d.path()),
            "--tasks",
            "T010",
            "--first-event-wait",
            "1",
        ],
    );
    assert_eq!(code, 1, "{v}");
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("no first event")
    );
    let r = wait_finished(d.path(), "t-silent");
    assert_eq!(r.state, State::TakenBack);
}

#[test]
fn a_track_that_is_running_is_not_started_twice() {
    let (d, tmp) = repo();
    let a = adapter(d.path(), &tmp);
    let (code, _) = cli(
        d.path(),
        "stall",
        &[
            "run",
            "--track",
            "t-twice",
            "--worktree",
            d.path().to_str().unwrap(),
            "--brief-file",
            &brief_file(d.path()),
            "--tasks",
            "T010",
        ],
    );
    assert_eq!(code, 0);
    let e = a.run(&request(d.path(), "t-twice", 5)).unwrap_err();
    assert!(e.to_string().contains("already running"), "{e}");
    a.stop("t-twice").unwrap();
    wait_finished(d.path(), "t-twice");
}

#[test]
fn the_delegate_leaves_changes_and_the_supervisor_commits_inside_allowed_paths() {
    let (d, _tmp) = repo();
    let before = gitx::git(d.path(), &["rev-list", "--count", "HEAD"]).unwrap();
    let (code, v) = cli(
        d.path(),
        "edit",
        &[
            "run",
            "--track",
            "t-edit",
            "--worktree",
            d.path().to_str().unwrap(),
            "--brief-file",
            &brief_file(d.path()),
            "--tasks",
            "T010",
        ],
    );
    assert_eq!(code, 0, "{v}");
    wait_finished(d.path(), "t-edit");
    // no commit by the delegate; one untracked file in the working tree (.ai-gent is not ours to stage)
    assert_eq!(
        gitx::git(d.path(), &["rev-list", "--count", "HEAD"]).unwrap(),
        before
    );
    std::fs::write(
        d.path().join(".git/info/exclude"),
        ".ai-gent/\nscratch-tmp/\nbrief-in.txt\n",
    )
    .unwrap();
    assert_eq!(
        commit::changed_paths(d.path()).unwrap(),
        ["fake-output.txt"]
    );

    // the checks run before anything is committed
    let w = d.path().to_str().unwrap();
    let (code, v) = cli(
        d.path(),
        "edit",
        &[
            "check",
            "--worktree",
            w,
            "--allow",
            "fake-output.txt",
            "--track",
            "t-edit",
        ],
    );
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["data"]["clean"], true);
    assert_eq!(
        status::read(d.path(), "t-edit").unwrap().verdict.result,
        status::VerdictResult::Accepted
    );
    let (code, v) = cli(
        d.path(),
        "edit",
        &[
            "check",
            "--worktree",
            w,
            "--allow",
            "src/**",
            "--track",
            "t-edit",
        ],
    );
    assert_eq!(code, 4, "{v}");
    assert_eq!(v["data"]["findings"][0]["reason"], "path-outside-allowed");
    let verdict = status::read(d.path(), "t-edit").unwrap().verdict;
    assert_eq!(verdict.reason.as_deref(), Some("path-outside-allowed"));

    // a commit message with a marker, or a path outside allowed_paths, is refused
    let (code, _) = cli(
        d.path(),
        "edit",
        &[
            "commit",
            "--worktree",
            w,
            "--allow",
            "src/**",
            "-m",
            "feat: x",
        ],
    );
    assert_eq!(code, 4);
    let (code, _) = cli(
        d.path(),
        "edit",
        &[
            "commit",
            "--worktree",
            w,
            "--allow",
            "fake-output.txt",
            "-m",
            "feat: x\n\nCo-Authored-By: A <a@a>",
        ],
    );
    assert_eq!(code, 4);
    let (code, v) = cli(
        d.path(),
        "edit",
        &[
            "commit",
            "--worktree",
            w,
            "--allow",
            "fake-output.txt",
            "-m",
            "feat(delegate): add output",
        ],
    );
    assert_eq!(code, 0, "{v}");
    let shown = gitx::git(d.path(), &["show", "--name-only", "--format=%B", "HEAD"]).unwrap();
    let lines: Vec<&str> = shown.lines().filter(|l| !l.is_empty()).collect();
    assert_eq!(lines, ["feat(delegate): add output", "fake-output.txt"]);
}

#[test]
fn brief_and_mark_commands() {
    let (d, _tmp) = repo();
    let task = d.path().join("task.txt");
    std::fs::write(&task, "Add the bridge.\n").unwrap();
    let (code, v) = cli(
        d.path(),
        "ok",
        &[
            "brief",
            "--repo",
            "ai-gent",
            "--branch",
            "feat/x",
            "--spec",
            "specs/041",
            "--tasks",
            "T010,T011",
            "--task-file",
            task.to_str().unwrap(),
            "--allow",
            "rust/crates/**",
            "--verify",
            "cargo test",
            "--tmpdir",
            "/t",
        ],
    );
    assert_eq!(code, 0, "{v}");
    let b = v["data"]["brief"].as_str().unwrap();
    assert!(b.contains("Task: specs/041 T010, T011: Add the bridge."));
    assert!(b.contains("allowed_paths: rust/crates/**;"));
    let (code, v) = cli(
        d.path(),
        "ok",
        &[
            "brief",
            "--repo",
            "r",
            "--branch",
            "b",
            "--spec",
            "s",
            "--tasks",
            "T1",
            "--task-file",
            task.to_str().unwrap(),
            "--allow",
            "plugins/**",
        ],
    );
    assert_eq!(code, 1);
    assert!(
        v["error"]["message"]
            .as_str()
            .unwrap()
            .contains("protected")
    );

    // the supervisor writes its own tracks
    let (code, v) = cli(
        d.path(),
        "ok",
        &[
            "mark",
            "feat-y",
            "--as",
            "claude",
            "--role",
            "builder",
            "--tasks",
            "T014",
            "--worktree",
            ".worktrees/feat-y",
        ],
    );
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["data"]["state"], "running");
    let (code, _) = cli(
        d.path(),
        "ok",
        &[
            "mark",
            "feat-y",
            "--state",
            "done",
            "--verdict",
            "rejected",
            "--reason",
            "weakened-test",
        ],
    );
    assert_eq!(code, 0);
    let s = status::read(d.path(), "feat-y").unwrap();
    assert_eq!(
        (s.state, s.verdict.reason.as_deref()),
        (State::Done, Some("weakened-test"))
    );
    let (code, _) = cli(d.path(), "ok", &["mark", "feat-z"]);
    assert_eq!(code, 1, "a new track needs its fields");
    let (code, _) = cli(
        d.path(),
        "ok",
        &[
            "mark",
            "feat-y",
            "--verdict",
            "rejected",
            "--reason",
            "a long diff text",
        ],
    );
    assert_eq!(code, 1);
    let (code, v) = cli(d.path(), "ok", &["log"]);
    assert_eq!(code, 0);
    assert_eq!(v["data"]["runs"].as_array().unwrap().len(), 0);
}
