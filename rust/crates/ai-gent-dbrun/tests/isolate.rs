//! T012: connector isolation. The child process of every test here is THIS executable, started again with
//! `ISOLATE_ROLE=child` and a mode (it has no libtest harness, so nothing but the child's own line reaches
//! the pipe the parent reads). A fake engine panics, aborts, hangs, prints garbage and floods; the parent
//! must answer with a failure JSON each time, never hang, never echo the child's text, and the child's
//! argv and environment must hold no credential.
//!
//! Run with `cargo test -p ai-gent-dbrun isolate` (test names contain `isolate`).
#![allow(clippy::items_after_statements)]
use ai_gent_dbrun::connector::isolate::{IsolatedConnector, serve_exec};
use ai_gent_dbrun::connector::request::MAX_REQUEST_LINE;
use ai_gent_dbrun::connector::{ConnFacts, Connector, ExecResult, Mode, Request, Stage};
use std::fmt::Write as _;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const PW: &str = "pw-FAKE-s3cr3t";
const USER: &str = "fakeuser9";
const HOST: &str = "db.fake-host.example";

// ---- the child ---------------------------------------------------------------------------------------

struct FakeEngine;
impl Connector for FakeEngine {
    fn engines(&self) -> &'static [&'static str] {
        &["postgresql"]
    }
    fn execute(&self, req: &Request) -> ExecResult {
        let mode = std::env::var("ISOLATE_MODE").unwrap_or_default();
        if let Ok(p) = std::env::var("ISOLATE_PIDFILE") {
            std::fs::write(p, std::process::id().to_string()).unwrap();
        }
        match mode.as_str() {
            "panic" => panic!(
                "driver unwrap failed on {}",
                req.conn.password.as_deref().unwrap_or("")
            ),
            "abort" => std::process::abort(),
            "hang" => {
                std::thread::sleep(Duration::from_secs(120));
                ExecResult::success()
            }
            "dump" => {
                let mut text = String::new();
                for a in std::env::args_os() {
                    let _ = writeln!(text, "ARG {}", a.to_string_lossy());
                }
                for (k, v) in std::env::vars_os() {
                    let _ = writeln!(text, "ENV {}={}", k.to_string_lossy(), v.to_string_lossy());
                }
                std::fs::write(std::env::var("ISOLATE_DUMP").unwrap(), text).unwrap();
                ExecResult::success()
            }
            _ => {
                // "ok": the password must have arrived on stdin, and it is echoed nowhere.
                if req.conn.password.as_deref() == Some(PW) && req.statements == ["SELECT 1"] {
                    let mut r = ExecResult::success();
                    r.server = Some(vec![serde_json::json!("fake 1.0")]);
                    r
                } else {
                    ExecResult::failure(Stage::Execute, "the password did not arrive on stdin")
                }
            }
        }
    }
}

fn child() {
    match std::env::var("ISOLATE_MODE").unwrap_or_default().as_str() {
        "garbage" => {
            let mut s = String::new();
            let _ = std::io::stdin().read_to_string(&mut s);
            println!("this is not json {PW}");
        }
        "flood" => {
            let mut s = String::new();
            let _ = std::io::stdin().read_line(&mut s);
            let chunk = vec![b'x'; 64 * 1024];
            let mut out = std::io::stdout();
            for _ in 0..4096 {
                if out.write_all(&chunk).is_err() {
                    break;
                }
            }
        }
        _ => std::process::exit(serve_exec(&FakeEngine, std::io::stdin(), std::io::stdout())),
    }
}

/// The parent of the environment test: it runs with secrets in ITS environment (set by the starter) and
/// asks an isolated child to dump what it received.
fn env_parent() {
    let dump = PathBuf::from(std::env::var("ISOLATE_DUMP_PATH").unwrap());
    let exe = std::env::current_exe().unwrap();
    let c = connector(&exe, "dump").with_env("ISOLATE_DUMP", &dump);
    let r = c.run(&request(Mode::Read, "SELECT 1"));
    assert!(r.ok, "{r:?}");
}

// ---- the parent side of the tests -----------------------------------------------------------------

fn connector(exe: &Path, mode: &str) -> IsolatedConnector {
    IsolatedConnector::new(exe, Vec::<&str>::new())
        .with_env("ISOLATE_ROLE", "child")
        .with_env("ISOLATE_MODE", mode)
}

fn request(mode: Mode, sql: &str) -> Request {
    let mut r = Request::new(
        "postgresql",
        ConnFacts {
            host: Some(HOST.into()),
            user: Some(USER.into()),
            password: Some(PW.into()),
            database: Some("shop".into()),
            ..ConnFacts::default()
        },
        mode,
        vec![sql.to_string()],
    );
    r.timeout = 2.0;
    r
}

fn exe() -> PathBuf {
    std::env::current_exe().unwrap()
}

fn clean(r: &ExecResult) {
    let text = serde_json::to_string(r).unwrap();
    for secret in [PW, USER, HOST, "driver unwrap failed", "this is not json"] {
        assert!(!text.contains(secret), "{secret:?} leaked in {text}");
    }
}

fn isolate_a_round_trip_carries_the_password_on_stdin_only() {
    let r = connector(&exe(), "ok").run(&request(Mode::Read, "SELECT 1"));
    assert!(r.ok, "{r:?}");
    assert_eq!(r.server.as_ref().unwrap()[0], "fake 1.0");
}

fn isolate_a_panic_in_the_driver_is_a_failure_json_without_its_message() {
    let r = connector(&exe(), "panic").run(&request(Mode::Write, "SELECT 1"));
    assert!(!r.ok);
    assert_eq!(r.stage, Some(Stage::Execute), "{r:?}");
    assert!(
        r.error.as_deref().unwrap().contains("failed unexpectedly"),
        "{r:?}"
    );
    clean(&r);
}

fn isolate_an_abort_is_a_failure_json_and_a_write_is_marked_unconfirmed() {
    let read = connector(&exe(), "abort").run(&request(Mode::Read, "SELECT 1"));
    assert!(!read.ok);
    assert_eq!(read.stage, Some(Stage::Connect), "{read:?}");
    assert!(
        read.error.as_deref().unwrap().contains("without a result"),
        "{read:?}"
    );
    clean(&read);
    let write = connector(&exe(), "abort").run(&request(Mode::Write, "SELECT 1"));
    assert_eq!(write.stage, Some(Stage::Execute), "{write:?}");
    assert!(
        write
            .error
            .as_deref()
            .unwrap()
            .contains("check the database state"),
        "{write:?}"
    );
    clean(&write);
}

fn isolate_a_hang_is_killed_at_the_deadline_and_the_child_is_gone() {
    let dir = tempfile::tempdir().unwrap();
    let pidfile = dir.path().join("pid");
    let c = connector(&exe(), "hang")
        .with_env("ISOLATE_PIDFILE", &pidfile)
        .with_timeout(Duration::from_millis(1500));
    let t0 = Instant::now();
    let r = c.run(&request(Mode::Read, "SELECT 1"));
    assert!(t0.elapsed() < Duration::from_secs(20), "{:?}", t0.elapsed());
    assert!(!r.ok);
    assert!(
        r.error
            .as_deref()
            .unwrap()
            .contains("timed out and was killed"),
        "{r:?}"
    );
    clean(&r);
    let pid = std::fs::read_to_string(&pidfile).expect("the child started");
    #[cfg(target_os = "linux")]
    {
        let proc = format!("/proc/{}", pid.trim());
        let mut gone = false;
        for _ in 0..50 {
            if !Path::new(&proc).exists() {
                gone = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        assert!(gone, "child {pid} still runs after the kill");
    }
    #[cfg(not(target_os = "linux"))]
    let _ = pid;
}

fn isolate_garbage_output_is_a_failure_and_is_never_echoed() {
    let r = connector(&exe(), "garbage").run(&request(Mode::Read, "SELECT 1"));
    assert!(!r.ok);
    assert!(
        r.error.as_deref().unwrap().contains("invalid result"),
        "{r:?}"
    );
    clean(&r);
}

fn isolate_a_flood_without_a_newline_is_cut_at_the_size_cap() {
    let c = connector(&exe(), "flood").with_max_output(4096);
    let t0 = Instant::now();
    let r = c.run(&request(Mode::Read, "SELECT 1"));
    assert!(t0.elapsed() < Duration::from_secs(20));
    assert!(!r.ok);
    assert!(
        r.error
            .as_deref()
            .unwrap()
            .contains("larger than the allowed size"),
        "{r:?}"
    );
}

fn isolate_a_missing_program_is_a_fixed_failure_that_names_no_path() {
    let c = IsolatedConnector::new("/no/such/dir/ai-gent-fake", ["db", "_exec"]);
    let r = c.run(&request(Mode::Read, "SELECT 1"));
    assert!(!r.ok);
    assert_eq!(r.stage, Some(Stage::Connect));
    assert!(!r.error.as_deref().unwrap().contains("/no/such"), "{r:?}");
    clean(&r);
}

fn isolate_the_child_has_no_secret_in_argv_or_environment() {
    let dir = tempfile::tempdir().unwrap();
    let dump = dir.path().join("dump");
    // A parent that has credentials in its own environment, as a shell session would.
    let status = std::process::Command::new(exe())
        .env("ISOLATE_ROLE", "env_parent")
        .env("ISOLATE_DUMP_PATH", &dump)
        .env("PGPASSWORD", PW)
        .env("MYSQL_PWD", PW)
        .env("ORACLE_PASSWORD", PW)
        .env("GITHUB_TOKEN", "ghp_FAKEFAKEFAKE")
        .env("AI_GENT_SOMETHING_SECRET", PW)
        .stdout(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(status.success(), "the env parent failed");
    let text = std::fs::read_to_string(&dump).unwrap();
    for secret in [PW, USER, HOST, "ghp_FAKE"] {
        assert!(
            !text.contains(secret),
            "{secret} reached the child:\n{text}"
        );
    }
    for name in [
        "PGPASSWORD",
        "MYSQL_PWD",
        "ORACLE_PASSWORD",
        "GITHUB_TOKEN",
        "AI_GENT_SOMETHING_SECRET",
    ] {
        assert!(
            !text.contains(&format!("ENV {name}=")),
            "{name} inherited:\n{text}"
        );
    }
    assert!(
        text.contains("ENV ISOLATE_MODE=dump"),
        "the explicit variables arrive"
    );
    assert!(text.contains("ENV PATH="), "the allowlisted PATH arrives");
    // argv is the program alone: no request text.
    let args: Vec<&str> = text.lines().filter(|l| l.starts_with("ARG ")).collect();
    assert_eq!(args.len(), 1, "{args:?}");
}

/// Counts what `serve_exec` reads of an endless stdin.
struct Endless(u64);
impl Read for Endless {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        buf.fill(b'a');
        self.0 += buf.len() as u64;
        Ok(buf.len())
    }
}

fn isolate_the_exec_reader_bounds_its_own_read() {
    let mut input = Endless(0);
    let mut out = Vec::new();
    let code = serve_exec(&FakeEngine, &mut input, &mut out);
    assert_eq!(code, 0);
    let consumed = input.0;
    assert!(
        consumed <= MAX_REQUEST_LINE as u64 + 2 + 64 * 1024,
        "read {consumed} bytes of an endless stdin"
    );
    let line = String::from_utf8(out).unwrap();
    assert_eq!(line.matches('\n').count(), 1);
    let r: ExecResult = serde_json::from_str(line.trim()).unwrap();
    assert_eq!((r.ok, r.stage), (false, Some(Stage::Request)));
    assert!(r.error.unwrap().contains("too large"));
}

fn isolate_the_exec_reader_answers_malformed_input_with_a_line_that_names_no_value() {
    for input in [
        format!("not json {PW}").into_bytes(),
        format!("[\"{PW}\"]").into_bytes(),
        format!("{{\"engine\": \"{PW}\", \"mode\": \"read\"}}").into_bytes(),
        vec![0xff, 0xfe, b'{'],
        Vec::new(),
    ] {
        let mut out = Vec::new();
        assert_eq!(serve_exec(&FakeEngine, input.as_slice(), &mut out), 0);
        let line = String::from_utf8(out).unwrap();
        assert_eq!(line.matches('\n').count(), 1, "{line}");
        let r: ExecResult = serde_json::from_str(line.trim()).unwrap();
        assert!(!r.ok);
        assert!(!line.contains(PW), "{line}");
    }
    // A closed stdout is reported by the exit code, not a panic.
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::ErrorKind::BrokenPipe.into())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    assert_eq!(serve_exec(&FakeEngine, b"{}".as_slice(), Broken), 1);
}

// ---- the harness -----------------------------------------------------------------------------------

type TestFn = fn();

fn main() {
    match std::env::var("ISOLATE_ROLE").as_deref() {
        Ok("child") => return child(),
        Ok("env_parent") => return env_parent(),
        _ => {}
    }
    let tests: &[(&str, TestFn)] = &[
        (
            "isolate_a_round_trip_carries_the_password_on_stdin_only",
            isolate_a_round_trip_carries_the_password_on_stdin_only,
        ),
        (
            "isolate_a_panic_in_the_driver_is_a_failure_json_without_its_message",
            isolate_a_panic_in_the_driver_is_a_failure_json_without_its_message,
        ),
        (
            "isolate_an_abort_is_a_failure_json_and_a_write_is_marked_unconfirmed",
            isolate_an_abort_is_a_failure_json_and_a_write_is_marked_unconfirmed,
        ),
        (
            "isolate_a_hang_is_killed_at_the_deadline_and_the_child_is_gone",
            isolate_a_hang_is_killed_at_the_deadline_and_the_child_is_gone,
        ),
        (
            "isolate_garbage_output_is_a_failure_and_is_never_echoed",
            isolate_garbage_output_is_a_failure_and_is_never_echoed,
        ),
        (
            "isolate_a_flood_without_a_newline_is_cut_at_the_size_cap",
            isolate_a_flood_without_a_newline_is_cut_at_the_size_cap,
        ),
        (
            "isolate_a_missing_program_is_a_fixed_failure_that_names_no_path",
            isolate_a_missing_program_is_a_fixed_failure_that_names_no_path,
        ),
        (
            "isolate_the_child_has_no_secret_in_argv_or_environment",
            isolate_the_child_has_no_secret_in_argv_or_environment,
        ),
        (
            "isolate_the_exec_reader_bounds_its_own_read",
            isolate_the_exec_reader_bounds_its_own_read,
        ),
        (
            "isolate_the_exec_reader_answers_malformed_input_with_a_line_that_names_no_value",
            isolate_the_exec_reader_answers_malformed_input_with_a_line_that_names_no_value,
        ),
    ];
    let filters: Vec<String> = std::env::args()
        .skip(1)
        .filter(|a| !a.starts_with('-'))
        .collect();
    let selected: Vec<_> = tests
        .iter()
        .filter(|(n, _)| filters.is_empty() || filters.iter().any(|f| n.contains(f.as_str())))
        .collect();
    println!("running {} tests", selected.len());
    let mut failed = 0;
    for (name, f) in selected {
        if std::panic::catch_unwind(f).is_ok() {
            println!("test {name} ... ok");
        } else {
            failed += 1;
            println!("test {name} ... FAILED");
        }
    }
    println!(
        "\ntest result: {}. {failed} failed",
        if failed == 0 { "ok" } else { "FAILED" }
    );
    if failed > 0 {
        std::process::exit(1);
    }
}
