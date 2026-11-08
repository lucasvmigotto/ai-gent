//! The process connector against fake `_exec` commands (shell scripts) and, for sqlite only, the real
//! `plugins/db/scripts/dbrun.py _exec` when `python3` is available.
#![cfg(unix)]
use ai_gent_dbrun::connector::process::ProcessConnector;
use ai_gent_dbrun::connector::{ConnFacts, Connector, Mode, Request, Stage};
use std::path::Path;
use std::time::{Duration, Instant};

const PASSWORD: &str = "pw-Sup3rS3cret";

fn request() -> Request {
    let conn = ConnFacts {
        host: Some("db.internal.example".into()),
        user: Some("alice_app".into()),
        password: Some(PASSWORD.into()),
        ..ConnFacts::default()
    };
    Request::new("postgresql", conn, Mode::Test, vec![])
}

/// A connector over `sh <script> <dump_dir>`, the script written to `dir`.
fn fake(dir: &Path, script: &str) -> ProcessConnector {
    let path = dir.join("fake.sh");
    std::fs::write(&path, script).unwrap();
    ProcessConnector::new("sh", [path.as_os_str(), dir.as_os_str()])
}

#[test]
fn a_normal_result_comes_back() {
    let dir = tempfile::tempdir().unwrap();
    let c = fake(
        dir.path(),
        "cat >/dev/null\necho '{\"ok\":true,\"results\":[{\"rowcount\":-1,\"columns\":[\"a\"],\"rows\":[[1]]}],\"seconds\":0.1}'\n",
    );
    let r = c.run(&request());
    assert!(r.ok, "{r:?}");
    assert_eq!(r.results.unwrap()[0].rows.as_ref().unwrap()[0][0], 1);
}

#[test]
fn a_failed_result_from_the_child_is_redacted() {
    let dir = tempfile::tempdir().unwrap();
    let c = fake(
        dir.path(),
        "cat >/dev/null\necho '{\"ok\":false,\"stage\":\"connect\",\"error\":\"bad login for alice_app at db.internal.example\"}'\n",
    );
    let r = c.run(&request());
    assert_eq!(r.stage, Some(Stage::Connect));
    assert_eq!(r.error.as_deref(), Some("bad login for <user> at <host>"));
}

#[test]
fn a_missing_program_is_a_connect_failure() {
    let c = ProcessConnector::new("/nonexistent/ai-gent-fake", Vec::<&str>::new());
    let r = c.run(&request());
    assert_eq!((r.ok, r.stage), (false, Some(Stage::Connect)));
    assert!(!r.error.unwrap().contains("nonexistent"));
}

#[test]
fn a_non_zero_exit_without_a_result_is_a_connect_failure() {
    let dir = tempfile::tempdir().unwrap();
    let c = fake(dir.path(), "cat >/dev/null\nexit 3\n");
    let r = c.run(&request());
    assert_eq!((r.ok, r.stage), (false, Some(Stage::Connect)));
    assert!(r.error.unwrap().contains("exited with status 3"));
}

#[test]
fn invalid_json_is_a_connect_failure_without_echoing_it() {
    let dir = tempfile::tempdir().unwrap();
    let c = fake(dir.path(), "cat >/dev/null\necho 'not json leak-me'\n");
    let r = c.run(&request());
    assert_eq!((r.ok, r.stage), (false, Some(Stage::Connect)));
    assert!(!r.error.unwrap().contains("leak-me"));
}

#[test]
fn a_result_followed_by_a_hang_is_a_failure() {
    let dir = tempfile::tempdir().unwrap();
    let c = fake(
        dir.path(),
        "cat >/dev/null\necho '{\"ok\":true,\"results\":[],\"seconds\":0.1}'\nexec sleep 30\n",
    )
    .with_timeout(Duration::from_millis(500));
    let t = Instant::now();
    let r = c.run(&request());
    assert!(t.elapsed() < Duration::from_secs(10));
    assert_eq!((r.ok, r.stage), (false, Some(Stage::Connect)));
}

#[test]
fn a_password_cut_by_the_stderr_limit_does_not_leak_a_prefix() {
    let dir = tempfile::tempdir().unwrap();
    // The capture keeps 8192 bytes: padding then the first 5 bytes of the password.
    let c = fake(
        dir.path(),
        "cat >/dev/null\nhead -c 8187 /dev/zero | tr '\\0' ' ' >&2\nprintf 'pw-Sup3rS3cret' >&2\nexit 3\n",
    );
    let r = c.run(&request());
    let e = r.error.unwrap();
    assert!(!e.contains("pw-S"), "{e}");
    assert!(e.contains("exited with status 3"), "{e}");
}

#[test]
fn a_hang_is_killed_at_the_timeout() {
    let dir = tempfile::tempdir().unwrap();
    // `exec` so the kill reaches the sleeping process itself.
    let c = fake(dir.path(), "exec sleep 30\n").with_timeout(Duration::from_millis(300));
    let t = Instant::now();
    let r = c.run(&request());
    assert!(t.elapsed() < Duration::from_secs(10));
    assert_eq!((r.ok, r.stage), (false, Some(Stage::Connect)));
    assert!(r.error.unwrap().contains("timed out"));
}

#[test]
fn a_huge_result_line_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let c = fake(
        dir.path(),
        "cat >/dev/null\nyes xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx | tr -d '\\n'\n",
    )
    .with_max_output(1024 * 1024)
    .with_timeout(Duration::from_secs(20));
    let t = Instant::now();
    let r = c.run(&request());
    assert!(t.elapsed() < Duration::from_secs(15));
    assert_eq!((r.ok, r.stage), (false, Some(Stage::Connect)));
    assert!(r.error.unwrap().contains("larger than"));
}

#[test]
fn stderr_with_the_password_is_redacted_and_bounded() {
    let dir = tempfile::tempdir().unwrap();
    let script = format!(
        "cat >/dev/null\necho \"login failed password {PASSWORD} host db.internal.example user alice_app\" >&2\nyes padding-padding | head -c 200000 >&2\nexit 1\n"
    );
    let r = fake(dir.path(), &script).run(&request());
    let err = r.error.unwrap();
    assert!(!err.contains(PASSWORD), "{err}");
    assert!(
        !err.contains("db.internal.example") && !err.contains("alice_app"),
        "{err}"
    );
    assert!(err.contains("<password>"), "{err}");
    assert!(err.len() < 600, "{}", err.len());
}

#[test]
fn credentials_go_on_stdin_only() {
    let dir = tempfile::tempdir().unwrap();
    let c = fake(
        dir.path(),
        "d=\"$1\"\nprintf '%s\\n' \"$0\" \"$@\" >\"$d/argv\"\nenv >\"$d/env\"\ncat >\"$d/stdin\"\necho '{\"ok\":true,\"results\":[]}'\n",
    );
    assert!(c.run(&request()).ok);
    let read = |n: &str| std::fs::read_to_string(dir.path().join(n)).unwrap();
    for (name, text) in [("argv", read("argv")), ("env", read("env"))] {
        assert!(!text.contains(PASSWORD), "{name} holds the password");
        assert!(!text.contains("alice_app"), "{name} holds the user");
    }
    assert!(
        read("stdin").contains(PASSWORD),
        "stdin carries the request"
    );
}

#[cfg(feature = "sqlite")]
fn python3_available() -> bool {
    std::process::Command::new("python3")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// The shared contract cases through the real `dbrun.py _exec` (sqlite only; stdlib, no packages).
#[cfg(feature = "sqlite")]
#[test]
fn process_connector_passes_the_sqlite_contract_through_dbrun_py() {
    use ai_gent_dbrun::connector::contract;
    if !python3_available() {
        eprintln!("SKIP: python3 not found; the real dbrun.py _exec contract run did not happen");
        return;
    }
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../plugins/db/scripts/dbrun.py");
    assert!(script.is_file(), "{}", script.display());
    let connector = ProcessConnector::host_python(script);
    let mut dirs = Vec::new();
    let ran = contract::run_exec_cases(
        &connector,
        "sqlite",
        contract::Profile::Original,
        |case, setup| {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join(format!("{}.db", case.file));
            rusqlite::Connection::open(&path)
                .unwrap()
                .execute_batch(&setup.join(";\n"))
                .unwrap();
            dirs.push(dir);
            ConnFacts {
                path: Some(path.to_string_lossy().into_owned()),
                ..ConnFacts::default()
            }
        },
    );
    assert_eq!(ran, 19, "exec cases run");
    eprintln!("ran {ran} sqlite contract cases through python3 dbrun.py _exec");
}
