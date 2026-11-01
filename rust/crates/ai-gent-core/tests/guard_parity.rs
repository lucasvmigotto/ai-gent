//! Parity of the Rust guard evaluator with the shared fixtures and, on Unix, with the shell
//! guards themselves (the oracle) on every fixture (spec 033, SC-001).
//!
//! The fixtures live in `shared/guard-fixtures/` at the repository root; a published crate has no
//! such directory and the tests skip with a message.
use ai_gent_core::guard::branch::git_branch;
use ai_gent_core::guard::{Ctx, Event, GuardId, Level, Shell, evaluate, evaluate_all};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn fixtures_dir() -> Option<PathBuf> {
    let d = repo_root().join("shared/guard-fixtures");
    d.is_dir().then_some(d)
}

fn load(name: &str) -> Value {
    let p = fixtures_dir().unwrap().join(format!("{name}.json"));
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

struct Sandbox {
    _dir: tempfile::TempDir,
    paths: Vec<(&'static str, String)>,
}

fn sandbox() -> Sandbox {
    let dir = tempfile::tempdir().unwrap();
    let mut paths = Vec::new();
    for (k, b) in [
        ("main", "main"),
        ("staging", "staging"),
        ("dev", "dev"),
        ("feat", "feat/x"),
    ] {
        let p = dir.path().join(k);
        let st = Command::new("git")
            .args(["init", "-q", "-b", b])
            .arg(&p)
            .status()
            .unwrap();
        assert!(st.success());
        paths.push((k, p.to_string_lossy().into_owned()));
    }
    Sandbox { _dir: dir, paths }
}

impl Sandbox {
    fn sub(&self, s: &str) -> String {
        let mut out = s.to_string();
        for (k, p) in &self.paths {
            out = out.replace(&format!("{{{k}}}"), p);
        }
        out
    }
    fn cwd(&self, c: &str) -> String {
        match c.strip_prefix('@') {
            Some(k) => self.paths.iter().find(|(n, _)| *n == k).unwrap().1.clone(),
            None => c.to_string(),
        }
    }
}

fn event_for(case: &Value, sb: &Sandbox, command: Option<&str>) -> Event {
    let tool_input = match command {
        Some(c) => json!({ "command": sb.sub(c) }),
        None => case["tool_input"].clone(),
    };
    Event {
        tool_name: case["tool"].as_str().unwrap().to_string(),
        cwd: sb.cwd(case["cwd"].as_str().unwrap()),
        tool_input,
    }
}

fn plugin_script(guard: &str) -> String {
    let dir = if guard == "containers" {
        "devcontainer"
    } else {
        guard
    };
    format!("plugins/{dir}/hooks/guard.sh")
}

/// Run the shell guard on the event; (decision, reason).
#[cfg(unix)]
fn oracle(guard: &str, ev: &Event) -> (String, String) {
    use std::io::Write;
    use std::process::Stdio;
    let input = json!({"hook_event_name": "PreToolUse", "tool_name": ev.tool_name, "cwd": ev.cwd, "tool_input": ev.tool_input});
    let mut child = Command::new("sh")
        .arg(repo_root().join(plugin_script(guard)))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.to_string().as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    if text.trim().is_empty() {
        return ("none".into(), String::new());
    }
    let v: Value = serde_json::from_str(text.trim()).unwrap();
    let o = &v["hookSpecificOutput"];
    (
        o["permissionDecision"].as_str().unwrap().to_string(),
        o["permissionDecisionReason"].as_str().unwrap().to_string(),
    )
}

fn run_guard(guard: &str, shell: Shell, only_sh_variant_free: bool) -> (usize, usize) {
    let sb = sandbox();
    let branch = |d: &str| git_branch(d);
    let ctx = Ctx { branch_of: &branch };
    let data = load(guard);
    let id = GuardId::parse(guard).unwrap();
    let (mut run, mut oracle_run) = (0, 0);
    for case in data["cases"].as_array().unwrap() {
        let want = case["want"].as_str().unwrap();
        let reason = case["reason"].as_str().unwrap_or("");
        let command = case.get("command").and_then(Value::as_str);
        let command = match (shell, command) {
            (Shell::Sh, c) => c,
            (s, Some(_)) => match case["variants"][s.name()].as_str() {
                Some(v) => Some(v),
                None => continue,
            },
            (_, None) => continue,
        };
        let _ = only_sh_variant_free;
        let ev = event_for(case, &sb, command);
        let d = evaluate(id, &ev, shell, &ctx);
        assert_eq!(
            d.level.name(),
            want,
            "{} ({}): {:?}",
            case["id"],
            shell.name(),
            command
        );
        assert_eq!(d.reason, reason, "{} reason", case["id"]);
        run += 1;
        #[cfg(unix)]
        if shell == Shell::Sh {
            let (od, or) = oracle(guard, &ev);
            assert_eq!(
                d.level.name(),
                od,
                "{} differs from the shell guard",
                case["id"]
            );
            assert_eq!(
                d.reason, or,
                "{} reason differs from the shell guard",
                case["id"]
            );
            oracle_run += 1;
        }
    }
    (run, oracle_run)
}

macro_rules! guard_test {
    ($name:ident, $guard:literal) => {
        #[test]
        fn $name() {
            if fixtures_dir().is_none() {
                eprintln!("skipped: shared/guard-fixtures is absent (published crate)");
                return;
            }
            let (run, oracle_run) = run_guard($guard, Shell::Sh, false);
            println!(
                "{}: {run} fixtures, {oracle_run} compared with the shell guard",
                $guard
            );
            assert!(run > 0);
        }
    };
}

guard_test!(git, "git");
guard_test!(db, "db");
guard_test!(secrets, "secrets");
guard_test!(repo, "repo");
guard_test!(containers, "containers");

/// The opencode adapter cases through `evaluate_all` and the opencode entry point.
#[test]
fn adapter() {
    if fixtures_dir().is_none() {
        eprintln!("skipped: shared/guard-fixtures is absent");
        return;
    }
    let sb = sandbox();
    let branch = |d: &str| git_branch(d);
    let ctx = Ctx { branch_of: &branch };
    for case in load("adapter")["cases"].as_array().unwrap() {
        let e = &case["event"];
        let resources: Vec<String> = e["resources"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r.as_str().unwrap().to_string())
            .collect();
        let d = ai_gent_core::guard::evaluate_opencode(
            e["action"].as_str().unwrap(),
            &resources,
            &sb.cwd(case["cwd"].as_str().unwrap()),
            Shell::Sh,
            &ctx,
        );
        assert_eq!(
            d.level.name(),
            case["want"].as_str().unwrap(),
            "{}",
            case["id"]
        );
    }
}

/// `evaluate_all` agrees with the single guards: the strictest of them, deny first.
#[test]
fn evaluate_all_is_the_strictest_single_guard() {
    if fixtures_dir().is_none() {
        return;
    }
    let sb = sandbox();
    let branch = |d: &str| git_branch(d);
    let ctx = Ctx { branch_of: &branch };
    let mut n = 0;
    for g in GuardId::ALL {
        for case in load(g.name())["cases"].as_array().unwrap() {
            let ev = event_for(case, &sb, case.get("command").and_then(Value::as_str));
            let all = evaluate_all(&ev, Shell::Sh, &ctx);
            let worst = GuardId::ALL
                .iter()
                .map(|x| evaluate(*x, &ev, Shell::Sh, &ctx).level)
                .fold(Level::None, |a, b| match (a, b) {
                    (Level::Deny, _) | (_, Level::Deny) => Level::Deny,
                    (Level::Ask, _) | (_, Level::Ask) => Level::Ask,
                    _ => Level::None,
                });
            assert_eq!(all.level, worst, "{}", case["id"]);
            n += 1;
        }
    }
    println!("evaluate_all: {n} fixtures");
}
