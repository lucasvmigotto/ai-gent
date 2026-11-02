//! `guard replay` (spec 033 T043): evaluate the shell commands found in Claude Code transcripts,
//! and optionally compare with the shell guards.
//!
//! Read-only and offline. A command is never returned: the report holds counts and the first 16
//! hex digits of the command's SHA-256, the same short hash the shadow log uses.
use super::shadow::sha256_hex;
use super::{Ctx, Event, GuardId, Shell, evaluate};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// One shell command taken from a transcript.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// `Bash` or `PowerShell`.
    pub tool: String,
    /// The session's working directory when it ran.
    pub cwd: String,
    /// The command text (kept in memory only).
    pub command: String,
}

/// Every `*.jsonl` file under `dir`, sorted.
#[must_use]
pub fn transcript_files(dir: &Path) -> Vec<PathBuf> {
    fn walk(d: &Path, out: &mut Vec<PathBuf>) {
        let Ok(rd) = std::fs::read_dir(d) else { return };
        for e in rd.flatten() {
            let p = e.path();
            // `file_type` does not follow symlinks: a link out of the tree is not walked.
            match e.file_type() {
                Ok(t) if t.is_dir() => walk(&p, out),
                Ok(t) if t.is_file() && p.extension().is_some_and(|x| x == "jsonl") => {
                    out.push(p);
                }
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, &mut out);
    out.sort();
    out
}

/// The shell commands one transcript line asks to run (assistant `tool_use` blocks).
#[must_use]
pub fn commands_in_line(line: &str) -> Vec<Found> {
    let Ok(v) = serde_json::from_str::<Value>(line) else {
        return Vec::new();
    };
    let cwd = v.get("cwd").and_then(Value::as_str).unwrap_or("");
    let Some(blocks) = v.pointer("/message/content").and_then(Value::as_array) else {
        return Vec::new();
    };
    blocks
        .iter()
        .filter(|b| b.get("type").and_then(Value::as_str) == Some("tool_use"))
        .filter_map(|b| {
            let tool = b.get("name").and_then(Value::as_str)?;
            if tool != "Bash" && tool != "PowerShell" {
                return None;
            }
            let command = b.pointer("/input/command").and_then(Value::as_str)?;
            (!command.is_empty()).then(|| Found {
                tool: tool.to_string(),
                cwd: cwd.to_string(),
                command: command.to_string(),
            })
        })
        .collect()
}

/// Counts for one guard.
#[derive(Debug, Default, Clone, Serialize, PartialEq, Eq)]
pub struct GuardCounts {
    /// Commands the evaluator denied.
    pub deny: u64,
    /// Commands it asked about.
    pub ask: u64,
    /// Commands it let through.
    pub none: u64,
    /// Commands also run through the shell guard.
    pub compared: u64,
    /// Commands the two decided differently (level or reason).
    pub differ: u64,
}

/// A command the evaluator and the shell guard decide differently.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Difference {
    /// Guard id.
    pub guard: String,
    /// First 16 hex digits of the command's SHA-256.
    pub cmd_sha: String,
    /// The evaluator's decision.
    pub rust_decision: String,
    /// The shell guard's decision.
    pub shell_decision: String,
    /// The reasons differ although the decisions agree.
    pub reason_only: bool,
}

/// The replay result.
#[derive(Debug, Default, Clone, Serialize, PartialEq, Eq)]
pub struct Report {
    /// Transcript files read.
    pub files: u64,
    /// Shell commands found.
    pub commands: u64,
    /// Counts per guard id.
    pub guards: BTreeMap<String, GuardCounts>,
    /// Differences with the shell guard (empty without an oracle).
    pub differences: Vec<Difference>,
}

/// Decision of the shell guard: (level, reason).
pub type ShellDecision = (String, String);

/// Runs the shell guard for a guard and event; `None` when it cannot be run.
pub type Oracle<'a> = &'a dyn Fn(GuardId, &Event) -> Option<ShellDecision>;

/// Replay `files`. `oracle` runs the shell guard for (guard, event), or `None` when it cannot.
pub fn replay(files: &[PathBuf], guards: &[GuardId], ctx: &Ctx, oracle: Option<Oracle>) -> Report {
    let mut rep = Report::default();
    for g in guards {
        rep.guards
            .insert(g.name().to_string(), GuardCounts::default());
    }
    for f in files {
        let Ok(text) = std::fs::read_to_string(f) else {
            continue;
        };
        rep.files += 1;
        for line in text.lines() {
            // Cheap filter before parsing the JSON: most lines are not tool calls.
            if !line.contains("\"tool_use\"") {
                continue;
            }
            for found in commands_in_line(line) {
                rep.commands += 1;
                let sha = sha256_hex(found.command.as_bytes())[..16].to_string();
                let shell = if found.tool == "PowerShell" {
                    Shell::PowerShell
                } else {
                    Shell::Sh
                };
                let ev = Event {
                    tool_name: found.tool,
                    cwd: found.cwd,
                    tool_input: json!({ "command": found.command }),
                };
                for &g in guards {
                    let d = evaluate(g, &ev, shell, ctx);
                    let c = rep.guards.entry(g.name().to_string()).or_default();
                    match d.level.name() {
                        "deny" => c.deny += 1,
                        "ask" => c.ask += 1,
                        _ => c.none += 1,
                    }
                    let Some(run) = oracle else { continue };
                    let Some((level, reason)) = run(g, &ev) else {
                        continue;
                    };
                    c.compared += 1;
                    let level_differs = level != d.level.name();
                    if level_differs || reason != d.reason {
                        c.differ += 1;
                        rep.differences.push(Difference {
                            guard: g.name().to_string(),
                            cmd_sha: sha.clone(),
                            rust_decision: d.level.name().to_string(),
                            shell_decision: level,
                            reason_only: !level_differs,
                        });
                    }
                }
            }
        }
    }
    rep
}

/// Run a plugin's `guard.sh` on the event (Unix `sh`). `hooks` is the directory that holds the
/// plugins (`plugins/`); the containers guard lives in `devcontainer`.
#[cfg(unix)]
#[must_use]
pub fn run_shell_guard(hooks: &Path, guard: GuardId, ev: &Event) -> Option<ShellDecision> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let plugin = if guard == GuardId::Containers {
        "devcontainer"
    } else {
        guard.name()
    };
    let script = hooks.join(plugin).join("hooks/guard.sh");
    let input = json!({"hook_event_name": "PreToolUse", "tool_name": ev.tool_name, "cwd": ev.cwd, "tool_input": ev.tool_input});
    let mut cmd = Command::new("sh");
    cmd.arg(script)
        .env("AI_GENT_NO_SHADOW", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = crate::exec::spawn_retry(&mut cmd).ok()?;
    let mut stdin = child.stdin.take()?;
    let data = input.to_string().into_bytes();
    let writer = std::thread::spawn(move || {
        let _ = stdin.write_all(&data);
    });
    let out = child.wait_with_output().ok()?;
    let _ = writer.join();
    let text = String::from_utf8_lossy(&out.stdout);
    if text.trim().is_empty() {
        return Some(("none".into(), String::new()));
    }
    let v: Value = serde_json::from_str(text.trim()).ok()?;
    let o = &v["hookSpecificOutput"];
    Some((
        o["permissionDecision"].as_str()?.to_string(),
        o["permissionDecisionReason"].as_str()?.to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(tool: &str, command: &str) -> String {
        json!({"type":"assistant","cwd":"/work","message":{"content":[
            {"type":"text","text":"x"},
            {"type":"tool_use","name":tool,"input":{"command":command}}]}})
        .to_string()
    }

    #[test]
    fn extracts_shell_tool_calls_only() {
        let f = commands_in_line(&line("Bash", "git status"));
        assert_eq!(
            f,
            vec![Found {
                tool: "Bash".into(),
                cwd: "/work".into(),
                command: "git status".into()
            }]
        );
        assert!(commands_in_line(&line("Read", "x")).is_empty());
        assert!(commands_in_line(&line("Bash", "")).is_empty());
        assert!(commands_in_line("not json").is_empty());
        assert!(commands_in_line(r#"{"message":{"content":"text"}}"#).is_empty());
    }

    #[test]
    fn counts_decisions_and_never_returns_a_command() {
        let d = tempfile::tempdir().unwrap();
        let sub = d.path().join("proj");
        std::fs::create_dir_all(&sub).unwrap();
        let text = [
            line("Bash", "git status"),
            line("Bash", "git push --force origin main"),
            line("Bash", "echo hi"),
            "garbage".to_string(),
        ]
        .join("\n");
        std::fs::write(sub.join("s.jsonl"), text).unwrap();
        std::fs::write(sub.join("ignored.txt"), "x").unwrap();
        let files = transcript_files(d.path());
        assert_eq!(files.len(), 1);
        let branch = |_: &str| Some("feature".to_string());
        let ctx = Ctx { branch_of: &branch };
        let rep = replay(&files, &GuardId::ALL, &ctx, None);
        assert_eq!((rep.files, rep.commands), (1, 3));
        let git = &rep.guards["git"];
        assert_eq!(git.deny + git.ask + git.none, 3);
        assert!(git.deny + git.ask >= 1, "the force push is not let through");
        assert_eq!(rep.guards["db"].none, 3);
        assert!(rep.differences.is_empty());
        let shown = serde_json::to_string(&rep).unwrap();
        assert!(!shown.contains("force") && !shown.contains("echo"));
    }

    #[test]
    fn an_oracle_that_disagrees_is_listed_by_hash() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("s.jsonl"), line("Bash", "echo hi")).unwrap();
        let files = transcript_files(d.path());
        let branch = |_: &str| None;
        let ctx = Ctx { branch_of: &branch };
        let sh_ask = |_: GuardId, _: &Event| Some(("ask".to_string(), "r".to_string()));
        let rep = replay(&files, &[GuardId::Db], &ctx, Some(&sh_ask));
        assert_eq!(rep.guards["db"].compared, 1);
        assert_eq!(rep.guards["db"].differ, 1);
        let diff = &rep.differences[0];
        assert_eq!(diff.cmd_sha, sha256_hex(b"echo hi")[..16]);
        assert!(!diff.reason_only);
        let agree = |_: GuardId, _: &Event| Some(("none".to_string(), String::new()));
        assert!(
            replay(&files, &[GuardId::Db], &ctx, Some(&agree))
                .differences
                .is_empty()
        );
    }
}
