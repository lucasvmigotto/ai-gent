//! `ai-gent autonomy` end to end in throwaway repositories, on a fixed clock
//! (2026-11-06T12:00:00Z) and a fixed local offset (-03:00).
#![allow(missing_docs)]
use ai_gent_core::guard::grant;
use assert_cmd::Command;
use serde_json::Value;
use std::path::Path;

const NOW: i64 = 1_793_966_400;

fn repo() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    let ok = std::process::Command::new("git")
        .args(["init", "-q", "-b", "dev"])
        .arg(d.path())
        .status()
        .unwrap()
        .success();
    assert!(ok);
    d
}

/// Run with the test-support stand-in for a terminal (`AI_GENT_TEST_TTY=1`).
fn run(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::cargo_bin("ai-gent")
        .unwrap()
        .current_dir(dir)
        .env("AI_GENT_TEST_TTY", "1")
        .env("AI_GENT_TEST_NOW", NOW.to_string())
        .env("AI_GENT_TEST_UTC_OFFSET", "-10800")
        .args(args)
        .output()
        .unwrap()
}

/// Run as a release build sees an agent: no bypass, stdin a pipe.
fn run_piped(dir: &Path, args: &[&str]) -> std::process::Output {
    Command::cargo_bin("ai-gent")
        .unwrap()
        .current_dir(dir)
        .env_remove("AI_GENT_TEST_TTY")
        .env("AI_GENT_TEST_NOW", NOW.to_string())
        .env("AI_GENT_TEST_UTC_OFFSET", "-10800")
        .args(args)
        .output()
        .unwrap()
}

fn code(o: &std::process::Output) -> i32 {
    o.status.code().unwrap()
}

fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

fn file(dir: &Path) -> String {
    std::fs::read_to_string(dir.join(".ai-gent/autonomy")).unwrap()
}

#[test]
fn grant_writes_the_canonical_file_the_parser_accepts() {
    let d = repo();
    let o = run(
        d.path(),
        &[
            "autonomy",
            "grant",
            "--for",
            "4h",
            "--actions",
            "push,mr,workitems",
            "--yes",
        ],
    );
    assert_eq!(code(&o), 0, "{}", text(&o.stderr));
    let t = file(d.path());
    assert_eq!(
        t,
        "expires: 2026-11-06T16:00:00Z\nbranches: develop dev\nactions: push pr issues\n"
    );
    let g = grant::parse(&t, &grant::format_utc(NOW)).expect("active");
    assert!(g.has("push") && g.has("pr") && g.has("issues") && !g.has("merge"));
    assert_eq!(g.branches(), ["develop", "dev"]);
    let out = text(&o.stdout);
    assert!(out.contains("13:00 -03:00"), "{out}");
    assert!(out.contains("2026-11-06T16:00:00Z UTC"), "{out}");
    assert!(
        out.contains("pr lifts nothing on GitLab merge requests"),
        "{out}"
    );
    assert!(
        out.contains("issues lifts nothing on GitLab issues"),
        "{out}"
    );
    let ex = std::fs::read_to_string(d.path().join(".git/info/exclude")).unwrap();
    assert!(ex.contains(".ai-gent/autonomy"));
    assert!(!d.path().join(".ai-gent/.autonomy.tmp").exists());
}

#[cfg(unix)]
#[test]
fn the_file_is_private() {
    use std::os::unix::fs::PermissionsExt;
    let d = repo();
    let o = run(
        d.path(),
        &[
            "autonomy",
            "grant",
            "--for",
            "30m",
            "--actions",
            "push",
            "--yes",
        ],
    );
    assert_eq!(code(&o), 0);
    let mode = std::fs::metadata(d.path().join(".ai-gent/autonomy"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
}

#[test]
fn until_takes_local_time_and_rfc3339() {
    let d = repo();
    let o = run(
        d.path(),
        &[
            "autonomy",
            "grant",
            "--until",
            "2026-11-06 18:00",
            "--actions",
            "push",
            "--branches",
            "develop,release/1",
            "--yes",
        ],
    );
    assert_eq!(code(&o), 0, "{}", text(&o.stderr));
    // 18:00 at -03:00 is 21:00 UTC.
    assert_eq!(
        file(d.path()),
        "expires: 2026-11-06T21:00:00Z\nbranches: develop release/1\nactions: push\n"
    );
    let o = run(
        d.path(),
        &[
            "autonomy",
            "grant",
            "--until",
            "2026-11-06T20:00:00+02:00",
            "--actions",
            "pr",
            "--yes",
        ],
    );
    assert_eq!(code(&o), 0);
    assert!(file(d.path()).starts_with("expires: 2026-11-06T18:00:00Z\n"));
}

#[test]
#[allow(clippy::too_many_lines)]
fn refusals_exit_2_and_write_nothing() {
    let d = repo();
    let cases: &[&[&str]] = &[
        &["autonomy", "grant", "--actions", "push", "--yes"],
        &["autonomy", "grant", "--for", "4h", "--yes"],
        &[
            "autonomy",
            "grant",
            "--for",
            "4h",
            "--actions",
            "deploy",
            "--yes",
        ],
        &[
            "autonomy",
            "grant",
            "--for",
            "31d",
            "--actions",
            "push",
            "--yes",
        ],
        &[
            "autonomy",
            "grant",
            "--for",
            "4x",
            "--actions",
            "push",
            "--yes",
        ],
        &[
            "autonomy",
            "grant",
            "--until",
            "2026-11-06 08:00",
            "--actions",
            "push",
            "--yes",
        ],
        &[
            "autonomy",
            "grant",
            "--until",
            "2026-11-06 08:00",
            "--for",
            "1h",
            "--actions",
            "push",
        ],
        &[
            "autonomy",
            "grant",
            "--for",
            "4h",
            "--actions",
            "push",
            "--branches",
            "dev,main",
            "--yes",
        ],
        &[
            "autonomy",
            "grant",
            "--for",
            "4h",
            "--actions",
            "push",
            "--branches",
            "staging",
            "--yes",
        ],
        // No terminal and no --yes: nothing to ask on.
        &["autonomy", "grant", "--for", "4h", "--actions", "push"],
        // No flags and no terminal.
        &["autonomy", "grant"],
        &["autonomy", "grant", "--yes"],
    ];
    for args in cases {
        let o = run(d.path(), args);
        assert_eq!(code(&o), 2, "{args:?}: {}", text(&o.stderr));
        assert!(!d.path().join(".ai-gent/autonomy").exists(), "{args:?}");
    }
    let o = run(
        d.path(),
        &[
            "autonomy",
            "grant",
            "--for",
            "31d",
            "--actions",
            "push",
            "--yes",
        ],
    );
    assert!(text(&o.stderr).contains("30 days"));
    let o = run(
        d.path(),
        &[
            "autonomy",
            "grant",
            "--for",
            "1h",
            "--actions",
            "push",
            "--branches",
            "main",
            "--yes",
        ],
    );
    assert!(text(&o.stderr).contains("main"));
    // Exactly 30 days is allowed.
    let o = run(
        d.path(),
        &[
            "autonomy",
            "grant",
            "--for",
            "30d",
            "--actions",
            "push",
            "--yes",
        ],
    );
    assert_eq!(code(&o), 0);
}

#[test]
fn not_in_a_repository_is_a_runtime_error() {
    let d = tempfile::tempdir().unwrap();
    let o = run(
        d.path(),
        &[
            "autonomy",
            "grant",
            "--for",
            "1h",
            "--actions",
            "push",
            "--yes",
        ],
    );
    assert_eq!(code(&o), 1);
    assert!(text(&o.stderr).contains("not inside a git repository"));
}

#[test]
fn show_reports_each_state() {
    let d = repo();
    let o = run(d.path(), &["autonomy", "show"]);
    assert_eq!(code(&o), 0);
    assert!(text(&o.stdout).contains("no active grant: missing"));

    run(
        d.path(),
        &[
            "autonomy",
            "grant",
            "--for",
            "4h",
            "--actions",
            "push,merge",
            "--yes",
        ],
    );
    let o = run(d.path(), &["autonomy", "show"]);
    let out = text(&o.stdout);
    assert!(out.contains("active grant"), "{out}");
    assert!(out.contains("actions:  push merge"), "{out}");
    assert!(
        out.contains("2026-11-06T16:00:00Z UTC, 2026-11-06 13:00 -03:00"),
        "{out}"
    );
    assert!(out.contains("left:     4h 0m"), "{out}");

    let o = run(d.path(), &["autonomy", "show", "--json"]);
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["active"], true);
    assert_eq!(v["seconds_left"], 14_400);
    assert_eq!(v["actions"], serde_json::json!(["push", "merge"]));
    assert_eq!(v["branches"], serde_json::json!(["develop", "dev"]));

    // Expired.
    std::fs::write(
        d.path().join(".ai-gent/autonomy"),
        "expires: 2026-11-06T11:00:00Z\nactions: push\n",
    )
    .unwrap();
    let o = run(d.path(), &["autonomy", "show"]);
    assert!(text(&o.stdout).contains("no active grant: expired at 2026-11-06T11:00:00Z"));
    let v: Value =
        serde_json::from_slice(&run(d.path(), &["autonomy", "show", "--json"]).stdout).unwrap();
    assert_eq!(
        (v["active"].clone(), v["reason"].clone()),
        (false.into(), "expired".into())
    );

    // Unreadable.
    std::fs::write(d.path().join(".ai-gent/autonomy"), "hello\n").unwrap();
    let o = run(d.path(), &["autonomy", "show"]);
    assert!(text(&o.stdout).contains("no active grant: unreadable"));
    assert_eq!(code(&o), 0);
}

#[test]
fn show_reads_and_never_writes() {
    let d = repo();
    let o = run(d.path(), &["autonomy", "show"]);
    assert_eq!(code(&o), 0);
    assert!(!d.path().join(".ai-gent").exists());
}

#[test]
fn revoke_removes_the_file() {
    let d = repo();
    let o = run(d.path(), &["autonomy", "revoke"]);
    assert_eq!(code(&o), 3, "nothing to remove");
    run(
        d.path(),
        &[
            "autonomy",
            "grant",
            "--for",
            "1h",
            "--actions",
            "push",
            "--yes",
        ],
    );
    let o = run(d.path(), &["autonomy", "revoke"]);
    assert_eq!(code(&o), 0);
    assert!(text(&o.stdout).starts_with("removed "));
    assert!(!d.path().join(".ai-gent/autonomy").exists());
    let o = run(d.path(), &["autonomy", "show"]);
    assert!(text(&o.stdout).contains("missing"));
}

#[test]
fn grant_json_and_help() {
    let d = repo();
    let o = run(
        d.path(),
        &[
            "--json",
            "autonomy",
            "grant",
            "--for",
            "1h",
            "--actions",
            "pr",
            "--yes",
        ],
    );
    let v: Value = serde_json::from_slice(&o.stdout).unwrap();
    assert_eq!(v["ok"], true);
    assert_eq!(v["actions"], serde_json::json!(["pr"]));
    assert_eq!(v["lifts_nothing"].as_array().unwrap().len(), 1);

    let o = run(d.path(), &["autonomy", "grant", "--help"]);
    assert_eq!(code(&o), 0);
    let h = text(&o.stdout);
    assert!(h.contains("workitems") && h.contains("GitHub only"), "{h}");
}

#[test]
fn without_a_terminal_grant_and_revoke_refuse_even_with_yes() {
    let d = repo();
    let target = d.path().join(".ai-gent").join("autonomy");
    let g = ["grant", "--for", "1h", "--actions", "push"];
    let cases: Vec<Vec<&str>> = vec![
        [&["autonomy"][..], &g, &["--yes"]].concat(),
        [&["autonomy", "--json"][..], &g, &["--yes"]].concat(),
        [&["autonomy"][..], &g].concat(),
        vec!["autonomy", "revoke"],
    ];
    for args in &cases {
        let o = run_piped(d.path(), args);
        assert_eq!(code(&o), 2, "{args:?}: {}", text(&o.stderr));
        assert!(text(&o.stderr).contains("real terminal"), "{args:?}");
        assert!(!target.exists(), "{args:?}");
    }
    // A grant that exists survives a refused revoke.
    let o = run(d.path(), &[&["autonomy"][..], &g, &["--yes"]].concat());
    assert_eq!(code(&o), 0);
    let before = file(d.path());
    assert_eq!(code(&run_piped(d.path(), &["autonomy", "revoke"])), 2);
    assert_eq!(file(d.path()), before);
    // Reading is not gated.
    assert_eq!(code(&run_piped(d.path(), &["autonomy", "show"])), 0);
}

#[test]
fn a_new_grant_names_the_one_it_replaces() {
    let d = repo();
    let o = run(
        d.path(),
        &[
            "autonomy",
            "grant",
            "--for",
            "1h",
            "--actions",
            "push",
            "--yes",
        ],
    );
    assert!(
        !text(&o.stderr).contains("replacing"),
        "{}",
        text(&o.stderr)
    );
    let o = run(
        d.path(),
        &[
            "autonomy",
            "grant",
            "--for",
            "2h",
            "--actions",
            "pr",
            "--yes",
        ],
    );
    assert_eq!(code(&o), 0);
    let e = text(&o.stderr);
    assert!(e.contains("replacing the active grant"), "{e}");
    assert!(e.contains("actions: push"), "{e}");
    assert!(e.contains("branches: develop dev"), "{e}");
    assert!(e.contains("2026-11-06T13:00:00Z"), "{e}");
}

#[test]
fn protected_branches_are_refused_in_any_spelling() {
    let d = repo();
    let target = d.path().join(".ai-gent").join("autonomy");
    for b in [
        "Main",
        "origin/main",
        "refs/heads/main",
        "STAGING",
        "origin/Homolog",
    ] {
        let o = run(
            d.path(),
            &[
                "autonomy",
                "grant",
                "--for",
                "1h",
                "--actions",
                "push",
                "--branches",
                b,
                "--yes",
            ],
        );
        assert_eq!(code(&o), 2, "{b}");
        assert!(!target.exists(), "{b}");
    }
}
