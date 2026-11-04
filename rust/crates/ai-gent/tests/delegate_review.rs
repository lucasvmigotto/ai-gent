//! `ai-gent delegate review` against the fake agents (`fake_claude`, `fake_agent`): no real call and no
//! network. Covers the FR-010 refusal, the prompt (diff, spec, no builder report), the review track in
//! role `reviewer`, `--dry-run` launching nothing, and `--out`.
use ai_gent_core::delegate::status::{self, Role, State, Status};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn example(name: &str) -> PathBuf {
    let exe = std::env::current_exe().unwrap();
    let p = exe
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples")
        .join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    assert!(p.exists(), "{} (cargo test builds examples)", p.display());
    p
}

fn git(dir: &Path, args: &[&str]) {
    let s = Command::new("git")
        .args(["-c", "user.name=t", "-c", "user.email=t@t"])
        .args(args)
        .current_dir(dir)
        .status()
        .unwrap();
    assert!(s.success(), "{args:?}");
}

/// A repository with one commit and an uncommitted change, a builder track `b-track` of `agent`
/// holding a report, a spec file and two profile directories.
fn setup(agent: &str) -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    git(p, &["init", "-q", "-b", "main"]);
    // like a real project: the supervisor's state directory is git-excluded
    std::fs::write(p.join(".git/info/exclude"), ".ai-gent/\n").unwrap();
    std::fs::write(p.join("a.txt"), "old line\n").unwrap();
    git(p, &["add", "a.txt"]);
    git(p, &["commit", "-q", "-m", "init"]);
    std::fs::write(p.join("a.txt"), "old line\nnew line from builder\n").unwrap();
    std::fs::write(p.join("spec.md"), "FR-777 the thing must work\nMODE:ok\n").unwrap();
    for n in ["profile-a", "profile-b", "scratch-tmp"] {
        std::fs::create_dir_all(p.join(n)).unwrap();
    }
    let st = Status::new(
        "b-track",
        agent,
        Role::Builder,
        &["T010c".into()],
        ".",
        State::Done,
    );
    status::write(p, &st).unwrap();
    std::fs::write(
        p.join(".ai-gent/delegate/b-track/last.txt"),
        "BUILDER_REPORT_MARKER all good",
    )
    .unwrap();
    d
}

/// `review_raw` with `--allow '**'` added unless the case brings `--allow` or `--no-check`.
fn review(d: &Path, extra: &[&str], builder_profile: Option<&str>) -> Output {
    let mut all = extra.to_vec();
    if !extra.iter().any(|a| *a == "--allow" || *a == "--no-check") {
        all.extend(["--allow", "**"]);
    }
    review_raw(d, &all, builder_profile)
}

fn review_raw(d: &Path, extra: &[&str], builder_profile: Option<&str>) -> Output {
    let mut c = Command::new(assert_cmd::cargo::cargo_bin("ai-gent"));
    c.args(["--json", "delegate", "--root"])
        .arg(d)
        .args(["review", "--builder-track", "b-track", "--worktree"])
        .arg(d)
        .args(extra)
        .env("AI_GENT_CODEX_BIN", example("ai-gent-fake-agent"))
        .env("AI_GENT_CLAUDE_BIN", example("fake_claude"))
        .env("AI_GENT_OPENCODE_BIN", example("fake_opencode"))
        .env("HOME", d)
        .env("TMPDIR", d.join("scratch-tmp"))
        .env_remove("CLAUDE_CONFIG_DIR")
        .env_remove("AI_GENT_CLAUDE_CONFIG_DIR");
    if let Some(b) = builder_profile {
        c.env("AI_GENT_CLAUDE_CONFIG_DIR", d.join(b));
    }
    c.output().unwrap()
}

fn data(o: &Output) -> Value {
    let v: Value = serde_json::from_slice(&o.stdout)
        .unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(&o.stdout)));
    v["data"].clone()
}

#[test]
fn the_reviewer_is_never_the_builder() {
    let d = setup("codex");
    let o = review(d.path(), &["--reviewer", "codex"], None);
    assert_eq!(
        o.status.code(),
        Some(4),
        "{}",
        String::from_utf8_lossy(&o.stdout)
    );
    assert!(String::from_utf8_lossy(&o.stdout).contains("the reviewer must not be the builder"));
    assert!(!d.path().join(".ai-gent/delegate/review-b-track").exists());
}

#[test]
fn a_claude_profile_is_the_builder_only_with_the_same_config_dir() {
    let d = setup("claude");
    let same = review(
        d.path(),
        &[
            "--reviewer",
            "claude",
            "--reviewer-profile",
            d.path().join("profile-a").to_str().unwrap(),
            "--dry-run",
        ],
        Some("profile-a"),
    );
    assert_eq!(same.status.code(), Some(4));
    assert!(data(&same)["refusal"].is_string());
    let other = review(
        d.path(),
        &[
            "--reviewer",
            "claude",
            "--reviewer-profile",
            d.path().join("profile-b").to_str().unwrap(),
            "--dry-run",
        ],
        Some("profile-a"),
    );
    assert_eq!(other.status.code(), Some(0));
    assert_eq!(data(&other)["allowed"], true);
}

#[test]
fn dry_run_prints_the_prompt_and_launches_nothing() {
    let d = setup("codex");
    let o = review(
        d.path(),
        &[
            "--reviewer",
            "claude",
            "--reviewer-profile",
            d.path().join("profile-b").to_str().unwrap(),
            "--spec",
            d.path().join("spec.md").to_str().unwrap(),
            "--dry-run",
        ],
        None,
    );
    assert_eq!(o.status.code(), Some(0));
    let v = data(&o);
    let prompt = v["prompt"].as_str().unwrap();
    assert!(prompt.contains("+new line from builder"), "{prompt}");
    assert!(prompt.contains("FR-777"));
    assert!(prompt.contains("file:line"));
    assert!(!prompt.contains("BUILDER_REPORT_MARKER"));
    assert!(!d.path().join(".ai-gent/delegate/review-b-track").exists());
}

#[test]
fn a_review_runs_as_a_reviewer_track_and_writes_out() {
    let d = setup("codex");
    let out = d.path().join("out/review.txt");
    let o = review(
        d.path(),
        &[
            "--reviewer",
            "claude",
            "--reviewer-profile",
            d.path().join("profile-b").to_str().unwrap(),
            "--spec",
            d.path().join("spec.md").to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
        ],
        None,
    );
    assert_eq!(
        o.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&o.stdout)
    );
    assert!(
        std::fs::read_to_string(&out)
            .unwrap()
            .contains("Done: T010c")
    );
    let v = data(&o);
    assert_eq!(v["track"], "review-b-track");
    assert_eq!(v["usage"]["input_tokens"], 1000);
    let st = status::read(d.path(), "review-b-track").unwrap();
    assert_eq!(st.role, Role::Reviewer);
    assert_eq!(st.agent, "claude");
    assert_eq!(st.state, State::Done);
    assert_eq!(st.tokens.unwrap().output_tokens, 50);
    let brief =
        std::fs::read_to_string(d.path().join(".ai-gent/delegate/review-b-track/brief.txt"))
            .unwrap();
    assert!(brief.contains("+new line from builder"));
    assert!(brief.contains("FR-777"));
    assert!(!brief.contains("BUILDER_REPORT_MARKER"));
    // the default output path is used when --out is absent
    let o2 = review(
        d.path(),
        &[
            "--reviewer",
            "claude",
            "--reviewer-profile",
            d.path().join("profile-b").to_str().unwrap(),
        ],
        None,
    );
    assert_eq!(
        o2.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&o2.stdout)
    );
    assert!(
        d.path()
            .join(".ai-gent/delegate/review-b-track/review.md")
            .is_file()
    );
}

fn commit_all(d: &Path, msg: &str) {
    git(d, &["add", "-A"]);
    git(d, &["commit", "-q", "-m", msg]);
}

fn dry(d: &Path, extra: &[&str]) -> Output {
    let pb = d.join("profile-b");
    let mut args = vec![
        "--reviewer",
        "claude",
        "--reviewer-profile",
        pb.to_str().unwrap(),
        "--dry-run",
    ];
    args.extend_from_slice(extra);
    review(d, &args, None)
}

#[test]
fn a_changed_tracked_env_file_is_left_out_of_the_prompt() {
    let d = setup("codex");
    let p = d.path();
    std::fs::write(p.join(".env"), "TOKEN=old\n").unwrap();
    commit_all(p, "env");
    std::fs::write(p.join(".env"), "TOKEN=TRACKED_SECRET_VALUE\n").unwrap();
    let o = dry(p, &[]);
    assert_eq!(o.status.code(), Some(0));
    let v = data(&o);
    let prompt = v["prompt"].as_str().unwrap();
    assert!(!prompt.contains("TRACKED_SECRET_VALUE"), "{prompt}");
    assert!(prompt.contains("[tracked .env: left out"), "{prompt}");
}

#[cfg(unix)]
#[test]
fn untracked_symlinks_are_never_followed() {
    let d = setup("codex");
    let p = d.path();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("o.txt"), "OUTSIDE_SECRET\n").unwrap();
    std::os::unix::fs::symlink(outside.path().join("o.txt"), p.join("link-out.txt")).unwrap();
    std::fs::write(p.join(".env"), "CRED_SECRET\n").unwrap();
    std::os::unix::fs::symlink(p.join(".env"), p.join("link-env.txt")).unwrap();
    let o = dry(p, &[]);
    assert_eq!(o.status.code(), Some(0));
    let v = data(&o);
    let prompt = v["prompt"].as_str().unwrap();
    assert!(!prompt.contains("OUTSIDE_SECRET"), "{prompt}");
    assert!(!prompt.contains("CRED_SECRET"), "{prompt}");
    assert!(
        prompt.contains("link-out.txt: skipped, a symlink"),
        "{prompt}"
    );
    assert!(
        prompt.contains("link-env.txt: skipped, a symlink"),
        "{prompt}"
    );
}

#[test]
fn an_untracked_netrc_and_other_credential_names_are_left_out() {
    let d = setup("codex");
    let p = d.path();
    for n in [
        ".netrc",
        ".npmrc",
        ".pypirc",
        ".git-credentials",
        "id_ecdsa",
        "x.pfx",
    ] {
        std::fs::write(p.join(n), format!("NETRC_SECRET_{n}\n")).unwrap();
    }
    let o = dry(p, &[]);
    assert_eq!(o.status.code(), Some(0));
    let prompt = data(&o)["prompt"].as_str().unwrap().to_string();
    assert!(!prompt.contains("NETRC_SECRET"), "{prompt}");
    assert!(prompt.contains("[untracked .netrc: left out"), "{prompt}");
}

#[cfg(unix)]
#[test]
fn a_symlink_to_a_credential_file_is_not_read_by_the_check() {
    let d = setup("codex");
    let p = d.path();
    std::fs::write(p.join(".env"), "CRED_SECRET_VALUE=abc123\n").unwrap();
    std::os::unix::fs::symlink(p.join(".env"), p.join("notes.txt")).unwrap();
    std::fs::write(p.join(".netrc"), "machine x password NETRC_PASS\n").unwrap();
    let o = dry(p, &["--allow", "**"]);
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(
        !out.contains("CRED_SECRET_VALUE") && !out.contains("NETRC_PASS"),
        "{out}"
    );
}

#[test]
fn without_allow_the_review_is_refused_unless_no_check() {
    let d = setup("codex");
    let p = d.path();
    let pb = p.join("profile-b");
    let base = [
        "--reviewer",
        "claude",
        "--reviewer-profile",
        pb.to_str().unwrap(),
    ];
    let mut args = base.to_vec();
    args.push("--dry-run");
    let o = review_raw(p, &args, None);
    assert_eq!(
        o.status.code(),
        Some(2),
        "{}",
        String::from_utf8_lossy(&o.stdout)
    );
    assert!(String::from_utf8_lossy(&o.stdout).contains("--allow is required"));
    let mut real = base.to_vec();
    let o = review_raw(p, &real, None);
    assert_eq!(o.status.code(), Some(2));
    assert!(!p.join(".ai-gent/delegate/review-b-track").exists());
    real.extend(["--no-check", "--dry-run"]);
    let o = review_raw(p, &real, None);
    assert_eq!(o.status.code(), Some(0));
    let prompt = data(&o)["prompt"].as_str().unwrap().to_string();
    assert!(prompt.contains("check was skipped on request"), "{prompt}");
}

#[test]
fn a_one_mebibyte_diff_is_cut_to_fit_one_argument() {
    let d = setup("codex");
    let p = d.path();
    std::fs::write(p.join("big.txt"), "BIGLINE\n".repeat(150_000)).unwrap();
    git(p, &["add", "big.txt"]);
    let o = dry(p, &[]);
    assert_eq!(o.status.code(), Some(0));
    let prompt = data(&o)["prompt"].as_str().unwrap().to_string();
    assert!(prompt.len() <= 100 * 1024, "{}", prompt.len());
    assert!(prompt.contains("[truncated: "), "cut note");
    // and a real run still launches the reviewer with it
    let pb = p.join("profile-b");
    let r = review(
        p,
        &[
            "--reviewer",
            "claude",
            "--reviewer-profile",
            pb.to_str().unwrap(),
        ],
        None,
    );
    assert_eq!(
        r.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&r.stdout)
    );
}

#[cfg(unix)]
#[test]
fn a_spec_that_is_a_symlink_or_outside_the_root_is_refused() {
    let d = setup("codex");
    let p = d.path();
    let outside = tempfile::tempdir().unwrap();
    let o_file = outside.path().join("o.md");
    std::fs::write(&o_file, "OUTSIDE_SPEC\n").unwrap();
    std::os::unix::fs::symlink(&o_file, p.join("s-out.md")).unwrap();
    std::fs::write(p.join(".env"), "CRED_SPEC\n").unwrap();
    std::os::unix::fs::symlink(p.join(".env"), p.join("s-env.md")).unwrap();
    for spec in [
        p.join("s-out.md"),
        p.join("s-env.md"),
        o_file.clone(),
        p.join(".env"),
    ] {
        let o = dry(p, &["--spec", spec.to_str().unwrap()]);
        let out = String::from_utf8_lossy(&o.stdout);
        assert_eq!(o.status.code(), Some(4), "{}: {out}", spec.display());
        assert!(
            !out.contains("OUTSIDE_SPEC") && !out.contains("CRED_SPEC"),
            "{out}"
        );
    }
}

#[test]
fn check_findings_stop_the_review_before_any_launch() {
    let d = setup("codex");
    let p = d.path();
    let pb = p.join("profile-b");
    // a.txt changed but only other/** is allowed: an FR-011 finding
    let o = review(
        p,
        &[
            "--reviewer",
            "claude",
            "--reviewer-profile",
            pb.to_str().unwrap(),
            "--allow",
            "other/**",
        ],
        None,
    );
    assert_eq!(
        o.status.code(),
        Some(4),
        "{}",
        String::from_utf8_lossy(&o.stdout)
    );
    let v = data(&o);
    assert_eq!(v["launched"], false);
    assert!(!v["findings"].as_array().unwrap().is_empty());
    assert!(!p.join(".ai-gent/delegate/review-b-track").exists());
}

#[test]
fn dry_run_reports_blocking_findings_as_not_allowed() {
    let d = setup("codex");
    let p = d.path();
    let o = dry(p, &["--allow", "other/**"]);
    assert_eq!(
        o.status.code(),
        Some(4),
        "{}",
        String::from_utf8_lossy(&o.stdout)
    );
    let v = data(&o);
    assert_eq!(v["allowed"], false);
    assert_eq!(v["launched"], false);
    assert!(!v["findings"].as_array().unwrap().is_empty());
    assert!(!p.join(".ai-gent/delegate/review-b-track").exists());
}

#[test]
fn a_tracked_sensitive_file_is_never_quoted_by_a_finding() {
    let d = setup("codex");
    let p = d.path();
    std::fs::write(p.join("secrets.txt"), "x\n").unwrap();
    commit_all(p, "s");
    std::fs::write(
        p.join("secrets.txt"),
        "no network sandbox_password=PRIVATE_VALUE\n",
    )
    .unwrap();
    for extra in [&["--allow", "**"][..], &["--allow", "other/**"][..]] {
        let o = dry(p, extra);
        let out = String::from_utf8_lossy(&o.stdout);
        assert!(!out.contains("PRIVATE_VALUE"), "{out}");
    }
    let pb = p.join("profile-b");
    let o = review(
        p,
        &[
            "--reviewer",
            "claude",
            "--reviewer-profile",
            pb.to_str().unwrap(),
            "--allow",
            "other/**",
        ],
        None,
    );
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(!out.contains("PRIVATE_VALUE"), "{out}");
}

#[test]
fn an_opencode_builder_model_is_compared_with_the_reviewer_model() {
    let d = setup("opencode:opencode/model-x");
    let p = d.path();
    let same = review(
        p,
        &[
            "--reviewer",
            "opencode",
            "--reviewer-model",
            "model-x",
            "--privacy-ack",
            "--dry-run",
        ],
        None,
    );
    assert_eq!(same.status.code(), Some(4));
    let other = review(
        p,
        &[
            "--reviewer",
            "opencode",
            "--reviewer-model",
            "model-y",
            "--privacy-ack",
            "--dry-run",
        ],
        None,
    );
    assert_eq!(
        other.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&other.stdout)
    );
}

#[test]
fn a_finished_opencode_reviewer_with_exit_one_is_a_review() {
    let d = setup("codex");
    let p = d.path();
    std::fs::write(p.join("spec-exit1.md"), "FR-1\nMODE:exit1\n").unwrap();
    let out = p.join("out/r.md");
    let o = review(
        p,
        &[
            "--reviewer",
            "opencode",
            "--privacy-ack",
            "--spec",
            p.join("spec-exit1.md").to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
        ],
        None,
    );
    assert_eq!(
        o.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&o.stdout)
    );
    assert!(!std::fs::read_to_string(&out).unwrap().trim().is_empty());
    let st = status::read(p, "review-b-track").unwrap();
    assert_eq!(st.state, State::Done);
}
