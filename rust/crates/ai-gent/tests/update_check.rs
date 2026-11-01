//! `ai-gent update --check` (spec 034 US1): the notice, the silences, the detached refresh.
#![cfg(unix)]
mod common;
use common::{cmd, have, install_tree, now, write_cache};
use std::time::{Duration, Instant};

struct Fx {
    _d: tempfile::TempDir,
    home: std::path::PathBuf,
    state: std::path::PathBuf,
    exe: std::path::PathBuf,
}

fn fx(version: &str) -> Fx {
    let d = tempfile::tempdir().unwrap();
    let home = d.path().canonicalize().unwrap().join("home");
    let state = d.path().canonicalize().unwrap().join("state");
    std::fs::create_dir_all(&state).unwrap();
    let exe = install_tree(&home, &[version]);
    Fx {
        _d: d,
        home,
        state,
        exe,
    }
}

fn stdout(c: &mut assert_cmd::Command) -> String {
    String::from_utf8(c.assert().success().get_output().stdout.clone()).unwrap()
}

fn cache_text(state: &std::path::Path) -> String {
    std::fs::read_to_string(state.join("ai-gent/update-check")).unwrap_or_default()
}

fn wait_for(what: &str, mut f: impl FnMut() -> bool) {
    for _ in 0..100 {
        if f() {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    panic!("timed out waiting for {what}");
}

/// A release base holding only `LATEST`.
fn base_with_latest(tag: &str) -> (tempfile::TempDir, String) {
    let b = tempfile::tempdir().unwrap();
    std::fs::write(b.path().join("LATEST"), format!("{tag}\n")).unwrap();
    let url = format!("file://{}", b.path().display());
    (b, url)
}

// Scenario 1
#[test]
fn newer_cached_release_prints_exactly_one_line() {
    let f = fx("2.27.0");
    write_cache(&f.state, now(), "2.28.0");
    let out = stdout(cmd(&f.exe, &f.state).args(["update", "--check"]));
    assert_eq!(
        out,
        "[ai-gent] update available: 2.27.0 -> 2.28.0 - run: ai-gent update (propose first, never self-apply in-session)\n"
    );
}

#[test]
fn same_or_older_release_is_silent() {
    let f = fx("2.27.0");
    write_cache(&f.state, now(), "2.27.0");
    assert_eq!(
        stdout(cmd(&f.exe, &f.state).args(["update", "--check"])),
        ""
    );
    write_cache(&f.state, now(), "2.26.0");
    assert_eq!(
        stdout(cmd(&f.exe, &f.state).args(["update", "--check"])),
        ""
    );
}

// Scenario 2: no cache -> silent, a detached refresh lands the cache, the next run sees it
#[test]
fn no_cache_is_silent_then_the_refresh_lands() {
    if !have("curl") {
        return;
    }
    let f = fx("2.27.0");
    let (_b, base) = base_with_latest("2.28.0");
    let mut c = cmd(&f.exe, &f.state);
    c.env("AI_GENT_RELEASE_BASE", &base)
        .args(["update", "--check"]);
    assert_eq!(stdout(&mut c), "");
    wait_for("the detached refresh", || {
        cache_text(&f.state).ends_with(" 2.28.0\n")
    });
    let mut c = cmd(&f.exe, &f.state);
    c.env("AI_GENT_RELEASE_BASE", &base)
        .args(["update", "--check"]);
    assert!(stdout(&mut c).contains("2.27.0 -> 2.28.0"));
}

// Scenario 3
#[test]
fn stale_cache_notifies_from_the_cache_and_refreshes() {
    if !have("curl") {
        return;
    }
    let f = fx("2.27.0");
    let old = now() - 30 * 86_400;
    write_cache(&f.state, old, "2.28.0");
    let (_b, base) = base_with_latest("2.29.0");
    let mut c = cmd(&f.exe, &f.state);
    c.env("AI_GENT_RELEASE_BASE", &base)
        .args(["update", "--check"]);
    assert!(
        stdout(&mut c).contains("2.27.0 -> 2.28.0"),
        "notice comes from the stale cache"
    );
    wait_for("the refresh", || {
        cache_text(&f.state).ends_with(" 2.29.0\n")
    });
    assert!(!cache_text(&f.state).starts_with(&old.to_string()));
}

// Scenario 4
#[test]
fn silent_when_opted_out_pinned_or_a_development_build() {
    let f = fx("2.27.0");
    write_cache(&f.state, now() - 30 * 86_400, "2.28.0");
    let before = cache_text(&f.state);
    let mut c = cmd(&f.exe, &f.state);
    c.env("AI_GENT_NO_UPDATE_CHECK", "1")
        .args(["update", "--check"]);
    assert_eq!(stdout(&mut c), "");

    std::fs::write(f.home.join("pinned"), "2.27.0\n").unwrap();
    assert_eq!(
        stdout(cmd(&f.exe, &f.state).args(["update", "--check"])),
        ""
    );
    std::fs::remove_file(f.home.join("pinned")).unwrap();

    // the binary straight from target/ has no VERSION beside bin/: a development build
    let mut c = cmd(&common::cargo_bin(), &f.state);
    c.env("AI_GENT_HOME", &f.home).args(["update", "--check"]);
    assert_eq!(stdout(&mut c), "");
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(
        cache_text(&f.state),
        before,
        "no refresh was started by any of them"
    );
}

// Scenario 5
#[test]
fn a_failing_refresh_leaves_the_cache_alone() {
    let f = fx("2.27.0");
    write_cache(&f.state, now() - 30 * 86_400, "2.28.0");
    let before = cache_text(&f.state);
    let mut c = cmd(&f.exe, &f.state);
    c.env("AI_GENT_RELEASE_BASE", "file:///nonexistent-ai-gent-base")
        .args(["update", "--refresh"]);
    assert_eq!(
        c.assert().success().get_output().stdout.len(),
        0,
        "the worker is silent"
    );
    assert_eq!(cache_text(&f.state), before);
}

// SC-001: the foreground never waits for the network. A fake curl that sleeps 10 s sits first on PATH.
#[test]
fn check_returns_fast_while_curl_hangs() {
    let f = fx("2.27.0");
    let fake = f.state.join("fakebin");
    std::fs::create_dir_all(&fake).unwrap();
    let curl = fake.join("curl");
    std::fs::write(&curl, "#!/bin/sh\nsleep 10\n").unwrap();
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&curl, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let path = format!(
        "{}:{}",
        fake.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    for cache in [None, Some(now() - 30 * 86_400)] {
        if let Some(t) = cache {
            write_cache(&f.state, t, "2.27.0");
        }
        let mut c = cmd(&f.exe, &f.state);
        c.env("PATH", &path).args(["update", "--check"]);
        let t0 = Instant::now();
        let out = stdout(&mut c);
        let took = t0.elapsed();
        assert_eq!(out, "");
        // The contract is 50 ms for the command; the test allows for a loaded CI machine.
        assert!(took < Duration::from_millis(1000), "took {took:?}");
        eprintln!("check took {took:?} (cache: {})", cache.is_some());
    }
}

// Scenario 6, and the snapshot of the shape in contracts/cli.md
#[test]
fn json_shape() {
    let f = fx("2.27.0");
    let mut c = cmd(&f.exe, &f.state);
    c.env("AI_GENT_NO_UPDATE_CHECK", "1")
        .args(["update", "--check", "--json"]);
    insta::assert_snapshot!(stdout(&mut c).trim_end(), @r#"{"schema":1,"ok":true,"data":{"installed":"2.27.0","latest":null,"cached_at":null,"stale":false,"pinned":false,"checking":false,"notice":null}}"#);

    write_cache(&f.state, 1_780_000_000, "2.28.0");
    let mut c = cmd(&f.exe, &f.state);
    c.env("AI_GENT_UPDATE_DAYS", "1000000")
        .args(["update", "--check", "--json"]);
    insta::assert_snapshot!(stdout(&mut c).trim_end(), @r#"{"schema":1,"ok":true,"data":{"installed":"2.27.0","latest":"2.28.0","cached_at":"2026-05-28T20:26:40Z","stale":false,"pinned":false,"checking":false,"notice":"[ai-gent] update available: 2.27.0 -> 2.28.0 - run: ai-gent update (propose first, never self-apply in-session)"}}"#);
}

// Coexistence (FR-003): a cache written by the real shell script is read, and the reverse.
#[test]
fn reads_the_cache_the_shell_script_writes() {
    if !have("bash") || !have("curl") {
        return;
    }
    let f = fx("2.27.0");
    // The script's own refresh function with a stubbed curl is exercised in scripts/test-update-check.sh;
    // here its documented line format, including the tag as the API returns it, is the fixture.
    write_cache(&f.state, now(), "v2.28.0");
    assert!(stdout(cmd(&f.exe, &f.state).args(["update", "--check"])).contains("-> 2.28.0"));
}
