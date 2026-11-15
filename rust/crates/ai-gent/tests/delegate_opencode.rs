//! The OpenCode adapter against a fake `opencode` (`examples/fake_opencode.rs`): no real OpenCode call
//! and no network. Covers detection (found, missing, outside `PATH`), the ready probe, stdin closed, the
//! inline permission set (no rule left that would ask), usage summed over the steps, an exit code 1 on a
//! finished run, the privacy refusal, and the PID-file stop.
#![allow(clippy::many_single_char_names, clippy::doc_markdown)]
use ai_gent_core::delegate::adapter::{Adapter, RunRequest, RunResult, track_result};
use ai_gent_core::delegate::opencode::OpenCodeAdapter;
use ai_gent_core::delegate::status::{self, Role, State};
use ai_gent_core::env::Env;
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
        .join(format!("fake_opencode{}", std::env::consts::EXE_SUFFIX));
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

/// A repository, a scratch TMPDIR and a fake home directory.
fn repo() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let d = tempfile::tempdir().unwrap();
    git(d.path(), &["init", "-q", "-b", "main"]);
    let tmp = d.path().join("scratch-tmp");
    std::fs::create_dir_all(&tmp).unwrap();
    let home = d.path().join("home");
    std::fs::create_dir_all(home.join(".claude-work")).unwrap();
    std::fs::write(home.join(".claude.json"), "{}").unwrap();
    (d, tmp, home)
}

fn adapter(root: &Path, tmp: &Path, home: &Path) -> OpenCodeAdapter {
    OpenCodeAdapter {
        program: fake(),
        root: root.to_path_buf(),
        watch_exe: bin(),
        tmpdir: tmp.to_path_buf(),
        model: "opencode/flint-40b-free".into(),
        privacy_ack: true,
        home: Some(home.to_path_buf()),
    }
}

fn request(root: &Path, track: &str, role: Role, mode: &str) -> RunRequest {
    request_with(root, track, role, mode, &[])
}

/// A request whose brief also holds `EDIT:`, `READ:` and `SHELL:` lines the fake attempts.
fn request_with(
    root: &Path,
    track: &str,
    role: Role,
    mode: &str,
    attempts: &[String],
) -> RunRequest {
    let extra: String = attempts.iter().flat_map(|a| ["\n", a.as_str()]).collect();
    RunRequest {
        track: track.into(),
        worktree: root.to_path_buf(),
        brief: format!(
            "Task: T010a do the thing.\nallowed_paths: fake-output.txt\nMODE:{mode}{extra}"
        ),
        tasks: vec!["T010a".into()],
        role,
        stall_secs: 600,
        first_event_wait: Duration::from_secs(0),
    }
}

fn wait_finished(root: &Path, track: &str) -> RunResult {
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

/// The `fake` object of the run's `tool_use` event.
fn fake_report(root: &Path, track: &str) -> Value {
    events(root, track)
        .into_iter()
        .find_map(|e| e.get("fake").cloned())
        .expect("a tool_use event with the fake report")
}

fn config_of(f: &Value) -> Vec<Value> {
    let c: Value = serde_json::from_str(f["config"].as_str().unwrap()).unwrap();
    c["permissions"].as_array().unwrap().clone()
}

/// Whether the fake denied the attempt on `target` (it matched the rules it received).
fn denied(f: &Value, target: &str) -> bool {
    f["attempts"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| {
            a["target"]
                .as_str()
                .is_some_and(|t| t.replace('\\', "/") == target.replace('\\', "/"))
        })
        .unwrap_or_else(|| panic!("no attempt on {target}"))["denied"]
        .as_bool()
        .unwrap()
}

fn listing(dir: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

fn log_lines(root: &Path) -> Vec<Value> {
    std::fs::read_to_string(root.join(".ai-gent/delegation.log"))
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

#[test]
fn detect_found_missing_and_outside_path() {
    let (d, tmp, home) = repo();
    let a = adapter(d.path(), &tmp, &home);
    assert_eq!(a.id(), "opencode");
    let det = a.detect();
    assert!(det.found);
    assert_eq!(det.version.as_deref(), Some("2.2.5-fake"));

    let mut missing = adapter(d.path(), &tmp, &home);
    missing.program = d.path().join("no-such-opencode");
    let det = missing.detect();
    assert!(!det.found && det.version.is_none());

    // The binary sits in the installer's directory under the home directory, which PATH does not name.
    let install = home.join(".opencode/bin");
    std::fs::create_dir_all(&install).unwrap();
    let installed = install.join(format!("opencode{}", std::env::consts::EXE_SUFFIX));
    std::fs::copy(fake(), &installed).unwrap();
    let env = Env::from_pairs([
        ("HOME", home.to_str().unwrap()),
        ("USERPROFILE", home.to_str().unwrap()),
    ]);
    let found = OpenCodeAdapter::new(d.path(), &env);
    assert_eq!(found.program, installed);
    let det = found.detect();
    assert!(det.found, "{det:?}");
    assert_eq!(
        det.path.as_deref(),
        Some(installed.to_string_lossy().replace('\\', "/").as_str())
    );

    // A bare name resolves through the install directory too, whatever PATH holds.
    let mut bare = adapter(d.path(), &tmp, &home);
    bare.program = "opencode-not-on-any-path".into();
    assert!(!bare.detect().found);
    bare.program = "opencode".into();
    assert!(bare.detect().found);
}

#[test]
fn ready_succeeds_and_a_failed_login_is_reported_not_worked_around() {
    let (d, tmp, home) = repo();
    let a = adapter(d.path(), &tmp, &home);
    a.ready().unwrap();
    let p = a.preflight(d.path());
    assert!(p.ok, "{p:?}");
    assert!(p.notes.iter().any(|n| n.contains("does not commit")));
    assert!(p.notes.iter().any(|n| n.contains("no OS sandbox")));
    assert!(!a.preflight(&d.path().join("missing")).ok);

    let out = Command::new(bin())
        .args(["delegate", "--agent", "opencode", "--root"])
        .arg(d.path())
        .arg("ready")
        .env("AI_GENT_OPENCODE_BIN", fake())
        .env("FAKE_LOGGED_IN", "0")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("opencode is not logged in"));

    let mut gone = adapter(d.path(), &tmp, &home);
    gone.program = d.path().join("no-such-opencode");
    assert!(
        gone.ready()
            .unwrap_err()
            .to_string()
            .contains("not available")
    );
}

#[test]
fn a_builder_run_has_stdin_closed_the_inline_permission_set_and_summed_usage() {
    let (d, tmp, home) = repo();
    let a = adapter(d.path(), &tmp, &home);
    let before_home = listing(&home);
    let h = a
        .run(&request(d.path(), "o-build", Role::Builder, "edit"))
        .unwrap();
    assert_eq!(h.track, "o-build");
    let r = wait_finished(d.path(), "o-build");
    assert_eq!(r.state, State::Done);
    assert_eq!(r.exit_code, Some(0));

    let r = a.result("o-build").unwrap();
    assert!(r.message.unwrap().contains("Done: T010a"));
    let u = r.usage.unwrap();
    // input 100 + 50 + 5 (cache) and 200 + 150 (cache); cached 50 + 150; output 10 + 20; reasoning 1 + 2
    assert_eq!(
        (
            u.input_tokens,
            u.cached_input_tokens,
            u.output_tokens,
            u.reasoning_output_tokens
        ),
        (505, 200, 30, 3)
    );
    assert!(
        (a.cost("o-build").unwrap()).abs() < f64::EPSILON,
        "free models cost 0"
    );

    let f = fake_report(d.path(), "o-build");
    assert_eq!(f["stdin_bytes"], 0, "stdin is closed");
    assert_eq!(f["tmpdir"], tmp.to_string_lossy().replace('\\', "/"));
    let cwd = std::fs::canonicalize(f["cwd"].as_str().unwrap()).unwrap();
    assert_eq!(cwd, std::fs::canonicalize(d.path()).unwrap());
    let brief = request(d.path(), "o-build", Role::Builder, "edit").brief;
    assert_eq!(f["brief_len"].as_u64().unwrap(), brief.len() as u64);
    let args: Vec<String> = serde_json::from_value(f["args"].clone()).unwrap();
    assert_eq!(
        args[..args.len() - 1],
        [
            "run",
            "--standalone",
            "--model",
            "opencode/flint-40b-free",
            "--format",
            "json",
            "--auto"
        ]
    );
    assert!(d.path().join("fake-output.txt").exists());

    // The permission set: passed inline, every rule a deny, so --auto has nothing left to approve
    // that should have been asked.
    let rules = config_of(&f);
    assert!(!rules.is_empty());
    assert!(
        rules.iter().all(|r| r["effect"] == "deny"),
        "a rule that would ask is left"
    );
    let has = |a: &str, p: &str| rules.iter().any(|r| r["action"] == a && r["resource"] == p);
    for p in [
        "git push*",
        "git commit*--no-verify*",
        "git reset*",
        "git branch -D*",
        "curl*",
        "wget*",
    ] {
        assert!(has("shell", p), "{p}");
    }
    assert!(has("read", "~/.ssh/*"));
    assert!(has(
        "read",
        &format!(
            "{}/.claude-work/*",
            home.to_string_lossy().replace('\\', "/")
        )
    ));
    assert!(
        !has("shell", "*") && !has("edit", "*"),
        "a builder may edit and run commands"
    );

    // The adapter keeps the permission set inline: it writes nothing into the home directory (the
    // fake does not read HOME, so this checks the adapter, not the child).
    assert_eq!(listing(&home), before_home);
    let st = status::read(d.path(), "o-build").unwrap();
    assert_eq!(st.agent, "opencode:opencode/flint-40b-free");
    assert!(
        !d.path()
            .join(".ai-gent/delegate/o-build/agent.pid")
            .exists()
    );
    assert_eq!(a.status("o-build").unwrap().events, 6);
}

#[test]
fn a_reviewer_and_a_researcher_get_no_edit_and_no_shell() {
    let (d, tmp, home) = repo();
    let a = adapter(d.path(), &tmp, &home);
    for (track, role) in [
        ("o-review", Role::Reviewer),
        ("o-research", Role::Researcher),
    ] {
        // "edit" mode makes the fake try to write fake-output.txt; it also tries a shell command.
        let shell = |c: &str| format!("SHELL:{c}");
        a.run(&request_with(
            d.path(),
            track,
            role,
            "edit",
            &[shell("ls"), shell("cat README.md")],
        ))
        .unwrap();
        wait_finished(d.path(), track);
        let f = fake_report(d.path(), track);
        let rules = config_of(&f);
        assert!(rules.iter().all(|r| r["effect"] == "deny"));
        let has = |a: &str| {
            rules
                .iter()
                .any(|r| r["action"] == a && r["resource"] == "*")
        };
        assert!(has("edit") && has("shell"), "{track}");
        // The fake enforced what it was given: the write did not happen, the commands were refused.
        let out = f["cwd"].as_str().unwrap();
        assert!(denied(&f, &format!("{out}/fake-output.txt")), "{track}");
        assert!(denied(&f, "ls") && denied(&f, "cat README.md"), "{track}");
        assert!(!d.path().join("fake-output.txt").exists(), "{track}");
    }
}

#[test]
fn the_builder_set_refuses_git_writes_secret_readers_and_profile_files() {
    let (d, tmp, home) = repo();
    let a = adapter(d.path(), &tmp, &home);
    let h = home.to_string_lossy().replace('\\', "/");
    let root = d.path().to_string_lossy().replace('\\', "/");
    let refused = [
        "SHELL:git push origin main".to_string(),
        "SHELL:/usr/bin/git push".to_string(),
        "SHELL:env GIT_TRACE=1 git commit -m x".to_string(),
        "SHELL:git commit -m x --no-verify".to_string(),
        "SHELL:cat .env".to_string(),
        "SHELL:/bin/cat config/prod.pem".to_string(),
        "SHELL:base64 secrets/credentials.json".to_string(),
        "SHELL:curl https://example.com".to_string(),
        format!("EDIT:{h}/.claude.json"),
        format!("READ:{h}/.claude.json"),
        format!("EDIT:{h}/.claude-work/settings.json"),
        format!("READ:{root}/app/.env"),
    ];
    let allowed = [
        "SHELL:cargo test".to_string(),
        "SHELL:git status".to_string(),
        "SHELL:cat src/main.rs".to_string(),
        format!("READ:{root}/src/main.rs"),
    ];
    let attempts: Vec<String> = refused.iter().chain(&allowed).cloned().collect();
    a.run(&request_with(
        d.path(),
        "o-deny",
        Role::Builder,
        "ok",
        &attempts,
    ))
    .unwrap();
    wait_finished(d.path(), "o-deny");
    let f = fake_report(d.path(), "o-deny");
    for r in &refused {
        let target = r.split_once(':').unwrap().1;
        assert!(denied(&f, target), "{r} should be refused");
    }
    for r in &allowed {
        let target = r.split_once(':').unwrap().1;
        assert!(!denied(&f, target), "{r} should be allowed");
    }
    assert_eq!(
        std::fs::read_to_string(home.join(".claude.json")).unwrap(),
        "{}"
    );
}

#[test]
fn exit_code_one_on_a_finished_run_is_still_done() {
    let (d, tmp, home) = repo();
    let a = adapter(d.path(), &tmp, &home);
    a.run(&request(d.path(), "o-exit1", Role::Builder, "exit1"))
        .unwrap();
    let r = wait_finished(d.path(), "o-exit1");
    assert_eq!(r.exit_code, Some(1));
    // The watcher cannot read OpenCode's events: it wrote failed and no usage.
    assert_eq!(
        status::read(d.path(), "o-exit1").unwrap().state,
        State::Failed
    );
    assert_eq!(log_lines(d.path())[0]["result"], "failed");
    let r = a.result("o-exit1").unwrap();
    // `result` writes its verdict back: status.json, the log line and `status` agree with it.
    let st = status::read(d.path(), "o-exit1").unwrap();
    assert_eq!(st.state, State::Done);
    assert_eq!(st.tokens, r.usage);
    assert!(st.tokens.is_some());
    let log = log_lines(d.path());
    assert_eq!(log.len(), 1, "the line is corrected, not duplicated");
    assert_eq!(log[0]["result"], "done");
    assert_eq!(log[0]["tokens"]["output_tokens"], 30);
    assert_eq!(a.status("o-exit1").unwrap().state, State::Done);
    let again = a.result("o-exit1").unwrap();
    assert_eq!(again.state, State::Done);
    assert_eq!(log_lines(d.path()).len(), 1);
    assert_eq!(r.exit_code, Some(1));
    assert_eq!(r.state, State::Done);
    assert!(!r.is_error);
    assert_eq!(r.reason.as_deref(), Some("exit-code-1-but-finished"));
    assert!(r.message.unwrap().contains("Done: T010a"));
}

#[test]
fn exit_zero_without_a_final_stop_is_failed() {
    let (d, tmp, home) = repo();
    let a = adapter(d.path(), &tmp, &home);
    a.run(&request(d.path(), "o-nostop", Role::Builder, "nostop"))
        .unwrap();
    let r = wait_finished(d.path(), "o-nostop");
    assert_eq!(r.exit_code, Some(0));
    // `status` runs the same judgement, so it never reports the watcher's `done` for this run.
    assert_eq!(a.status("o-nostop").unwrap().state, State::Failed);
    assert_eq!(
        status::read(d.path(), "o-nostop").unwrap().state,
        State::Failed
    );
    let log = log_lines(d.path());
    assert_eq!((log.len(), log[0]["result"].as_str()), (1, Some("failed")));
    let r = a.result("o-nostop").unwrap();
    assert_eq!(r.state, State::Failed);
    assert!(r.is_error);
    assert_eq!(r.reason.as_deref(), Some("no-final-step-finish"));
    assert!(r.usage.is_some(), "the usage of the steps that ran is kept");
}

#[test]
fn a_free_model_is_refused_without_the_privacy_acknowledgement() {
    let (d, tmp, home) = repo();
    let mut a = adapter(d.path(), &tmp, &home);
    a.privacy_ack = false;
    let e = a
        .run(&request(d.path(), "o-refused", Role::Builder, "ok"))
        .unwrap_err()
        .to_string();
    assert!(
        e.contains("--privacy-ack") && e.contains("free hosted model"),
        "{e}"
    );
    assert!(!d.path().join(".ai-gent/delegate/o-refused").exists());

    // The same through the CLI: refused with the reason, then accepted with the flag.
    let brief = d.path().join("brief.txt");
    std::fs::write(&brief, "Task: T\nMODE:ok").unwrap();
    let cli = |extra: &[&str]| {
        Command::new(bin())
            .args(["delegate", "--agent", "opencode", "--root"])
            .arg(d.path())
            .args([
                "run",
                "--track",
                "o-cli",
                "--worktree",
                d.path().to_str().unwrap(),
                "--first-event-wait",
                "0",
                "--brief-file",
            ])
            .arg(&brief)
            .args(extra)
            .env("AI_GENT_OPENCODE_BIN", fake())
            .output()
            .unwrap()
    };
    let out = cli(&[]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("--privacy-ack"));
    assert!(!d.path().join(".ai-gent/delegate/o-cli").exists());
    let out = cli(&["--privacy-ack"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    wait_finished(d.path(), "o-cli");

    // A model that is not free needs no acknowledgement, and --model is passed through qualified.
    let mut paid = adapter(d.path(), &tmp, &home).with_model("acme/paid-pro");
    paid.privacy_ack = false;
    paid.run(&request(d.path(), "o-paid", Role::Builder, "ok"))
        .unwrap();
    wait_finished(d.path(), "o-paid");
    let args: Vec<String> =
        serde_json::from_value(fake_report(d.path(), "o-paid")["args"].clone()).unwrap();
    assert!(args.windows(2).any(|w| w == ["--model", "acme/paid-pro"]));
}

#[test]
fn stop_uses_the_pid_file() {
    let (d, tmp, home) = repo();
    let a = adapter(d.path(), &tmp, &home);
    a.run(&request(d.path(), "o-stall", Role::Builder, "stall"))
        .unwrap();
    let pid_file = d.path().join(".ai-gent/delegate/o-stall/agent.pid");
    let t = Instant::now();
    while !pid_file.exists() {
        assert!(t.elapsed() < Duration::from_secs(20), "no PID file");
        std::thread::sleep(Duration::from_millis(25));
    }
    assert!(a.status("o-stall").unwrap().running);
    let s = a.stop("o-stall").unwrap();
    assert!(
        s.signalled && s.exited,
        "stop outcome: {s:?}; track files: {:?}",
        track_files(d.path(), "o-stall")
    );
    let r = wait_finished(d.path(), "o-stall");
    assert_eq!(r.state, State::TakenBack);
    assert_eq!(a.result("o-stall").unwrap().state, State::TakenBack);
}

#[test]
fn preflight_of_a_free_model_needs_the_flag_through_the_cli() {
    let (d, tmp, _home) = repo();
    let cli = |extra: &[&str]| {
        Command::new(bin())
            .args(["delegate", "--agent", "opencode", "--root"])
            .arg(d.path())
            .arg("preflight")
            .arg("--worktree")
            .arg(d.path())
            .args(extra)
            .env("AI_GENT_OPENCODE_BIN", fake())
            .env("TMPDIR", &tmp)
            .output()
            .unwrap()
    };
    let out = cli(&[]);
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).contains("privacy_acknowledged"));
    assert!(String::from_utf8_lossy(&out.stdout).contains("--privacy-ack needed"));
    let out = cli(&["--privacy-ack"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    // The model is the one the flag names: a variant of a free model is still free, a paid one is not.
    assert!(!cli(&["--model", "flint-40b-free#fast"]).status.success());
    assert!(cli(&["--model", "acme/paid-pro"]).status.success());
}

#[test]
fn a_relative_binary_override_still_works_from_the_worktree() {
    let (d, tmp, _home) = repo();
    let brief = d.path().join("brief.txt");
    std::fs::write(&brief, "Task: T\nMODE:ok").unwrap();
    let fake = fake();
    // The supervisor runs from the fake's own directory with a relative path; the watcher starts from
    // the worktree, so the path must have been made absolute first.
    let out = Command::new(bin())
        .args(["delegate", "--agent", "opencode", "--root"])
        .arg(d.path())
        .args(["run", "--track", "o-rel", "--worktree"])
        .arg(d.path())
        .args(["--first-event-wait", "0", "--privacy-ack", "--brief-file"])
        .arg(&brief)
        .current_dir(fake.parent().unwrap())
        .env(
            "AI_GENT_OPENCODE_BIN",
            format!("./{}", fake.file_name().unwrap().to_string_lossy()),
        )
        .env("TMPDIR", &tmp)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let r = wait_finished(d.path(), "o-rel");
    assert_eq!(r.exit_code, Some(0));
    assert_eq!(r.state, State::Done);
}

/// A repository with one commit and a linked worktree beside it (spec 041, isolation defect).
fn repo_with_worktree() -> (tempfile::TempDir, PathBuf, PathBuf, PathBuf, PathBuf) {
    let (d, tmp, home) = repo();
    git(d.path(), &["config", "user.email", "t@example.invalid"]);
    git(d.path(), &["config", "user.name", "t"]);
    std::fs::write(d.path().join("tracked.txt"), "x\n").unwrap();
    git(d.path(), &["add", "tracked.txt"]);
    git(d.path(), &["commit", "-q", "-m", "init"]);
    let wt = d.path().join("wt-opencode");
    git(
        d.path(),
        &["worktree", "add", "-q", "-b", "wt", wt.to_str().unwrap()],
    );
    let main = d.path().to_path_buf();
    (d, tmp, home, main, wt)
}

fn worktree_request(wt: &Path, track: &str, attempts: &[String]) -> RunRequest {
    let mut r = request_with(wt, track, Role::Builder, "edit", attempts);
    r.worktree = wt.to_path_buf();
    r
}

fn same(a: &str, b: &Path) -> bool {
    Path::new(a).canonicalize().unwrap() == b.canonicalize().unwrap()
}

#[test]
fn the_child_runs_in_the_worktree_with_pwd_set_and_writes_there_not_in_the_main_checkout() {
    let (d, tmp, home, main, wt) = repo_with_worktree();
    // The main checkout is already dirty before the run: that is not the agent's doing.
    std::fs::write(main.join("preexisting.txt"), "was here\n").unwrap();
    let a = adapter(&main, &tmp, &home);
    a.run(&worktree_request(&wt, "o-iso", &[])).unwrap();
    let r = wait_finished(&main, "o-iso");
    assert_eq!(r.state, State::Done, "{r:?}");
    let f = fake_report(&main, "o-iso");
    assert!(same(f["cwd"].as_str().unwrap(), &wt), "{f}");
    assert!(same(f["pwd"].as_str().unwrap(), &wt), "PWD: {f}");
    assert_eq!(f["env_present"], serde_json::json!([]), "{f}");
    assert!(wt.join("fake-output.txt").exists());
    assert!(!main.join("fake-output.txt").exists());
    let res = a.result("o-iso").unwrap();
    assert_eq!(res.state, State::Done);
    assert!(!main.join(".ai-gent/delegate/o-iso/escaped.txt").exists());
    drop(d);
}

#[test]
fn a_write_into_the_main_checkout_fails_the_track_with_a_clear_message() {
    let (d, tmp, home, main, wt) = repo_with_worktree();
    let leak = main.join("leaked.txt");
    let a = adapter(&main, &tmp, &home);
    let attempt = format!("EDIT:{}", leak.to_string_lossy());
    a.run(&worktree_request(&wt, "o-leak", &[attempt])).unwrap();
    let r = wait_finished(&main, "o-leak");
    assert!(leak.exists(), "the stub did write into the main checkout");
    assert_eq!(r.state, State::Failed, "{r:?}");
    assert!(r.is_error);
    let res = a.result("o-leak").unwrap();
    assert_eq!(res.state, State::Failed);
    let reason = res.reason.unwrap();
    assert!(
        reason.starts_with("wrote-outside-worktree:") && reason.contains("leaked.txt"),
        "{reason}"
    );
    assert!(!reason.contains("fake-output.txt"), "{reason}");
    assert_eq!(status::read(&main, "o-leak").unwrap().state, State::Failed);
    let listed =
        std::fs::read_to_string(main.join(".ai-gent/delegate/o-leak/escaped.txt")).unwrap();
    assert_eq!(listed.trim(), "leaked.txt");
    drop(d);
}

#[test]
fn the_supervisors_directory_variables_do_not_reach_the_child() {
    let (d, tmp, home, main, wt) = repo_with_worktree();
    let _ = home;
    let brief = d.path().join("brief.txt");
    std::fs::write(&brief, "Task: T\nallowed_paths: fake-output.txt\nMODE:edit").unwrap();
    let out = Command::new(bin())
        .args(["delegate", "--agent", "opencode", "--root"])
        .arg(&main)
        .args(["run", "--track", "o-env", "--worktree"])
        .arg(&wt)
        .args(["--first-event-wait", "0", "--privacy-ack", "--brief-file"])
        .arg(&brief)
        .env("AI_GENT_OPENCODE_BIN", fake())
        .env("TMPDIR", &tmp)
        .env("INIT_CWD", main.join("elsewhere"))
        .env("OLDPWD", main.join("elsewhere"))
        .env("GIT_DIR", main.join(".git"))
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let r = wait_finished(&main, "o-env");
    assert_eq!(r.state, State::Done, "{r:?}");
    let f = fake_report(&main, "o-env");
    assert_eq!(f["env_present"], serde_json::json!([]), "{f}");
    assert!(same(f["pwd"].as_str().unwrap(), &wt), "{f}");
}

#[test]
fn a_reused_track_directory_drops_the_stale_escape_record() {
    let (d, tmp, home, main, wt) = repo_with_worktree();
    let a = adapter(&main, &tmp, &home);
    let leak = main.join("leaked.txt");
    let attempt = format!("EDIT:{}", leak.to_string_lossy());
    a.run(&worktree_request(&wt, "o-reuse", &[attempt]))
        .unwrap();
    assert_eq!(wait_finished(&main, "o-reuse").state, State::Failed);
    let record = main.join(".ai-gent/delegate/o-reuse/escaped.txt");
    assert!(record.exists());
    // The same track again, nothing outside the worktree touched this time.
    a.run(&worktree_request(&wt, "o-reuse", &[])).unwrap();
    let r = wait_finished(&main, "o-reuse");
    assert_eq!(r.state, State::Done, "{r:?}");
    assert!(!record.exists(), "stale escaped.txt");
    drop(d);
}

#[test]
fn an_unreadable_main_checkout_after_the_run_fails_the_track_closed() {
    let (d, tmp, home, main, wt) = repo_with_worktree();
    let a = adapter(&main, &tmp, &home);
    // The stub corrupts the main checkout's index: `git status` there fails after the run.
    let attempt = format!("EDIT:{}", main.join(".git/index").to_string_lossy());
    a.run(&worktree_request(&wt, "o-closed", &[attempt]))
        .unwrap();
    let r = wait_finished(&main, "o-closed");
    assert_eq!(r.state, State::Failed, "{r:?}");
    let res = a.result("o-closed").unwrap();
    assert!(
        res.reason
            .unwrap()
            .starts_with("wrote-outside-worktree-unknown:")
    );
    drop(d);
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
