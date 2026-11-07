//! The watcher: a small background process (`ai-gent delegate _watch <dir>`) that starts the agent from
//! `launch.json`, copies its events to `events.jsonl`, keeps `status.json` current, and writes the exit
//! record, the final status and the delegation log line when the agent ends. It is what lets `run`
//! return at once while the adapter's clock, event count and usage stay correct.
use super::adapter::{BRIEF_FILE, ExitRecord, Launch};
use super::state::files;
use super::status::{self, LogLine, State, Status, Usage};
use super::{Error, Res, io, now_secs};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// What one agent event adds up to.
#[derive(Debug, Default, Clone)]
pub struct Tally {
    /// Events seen.
    pub events: u64,
    /// Usage from `turn.completed` events.
    pub usage: Option<Usage>,
    /// The Claude final object (`type:"result"`), when seen.
    pub result: Option<ClaudeResult>,
}

/// What the Claude final JSON object says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaudeResult {
    /// `result`: the final message.
    pub text: String,
    /// `is_error`.
    pub is_error: bool,
    /// `subtype` (`error_max_turns`, ...), when present.
    pub subtype: Option<String>,
}

impl ClaudeResult {
    /// Why an `is_error` run failed: the subtype without `error_` (`max-turns`), else `error`.
    #[must_use]
    pub fn reason(&self) -> String {
        self.subtype
            .as_deref()
            .map(|t| t.trim_start_matches("error_").replace('_', "-"))
            .filter(|t| !t.is_empty() && t != "success")
            .unwrap_or_else(|| "error".to_string())
    }
}

impl Tally {
    /// Count one stdout line; only JSON objects are events.
    pub fn line(&mut self, line: &str) -> bool {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            return false;
        };
        if !v.is_object() {
            return false;
        }
        self.events += 1;
        if v["type"] == "turn.completed" {
            let u = &v["usage"];
            let n = |k: &str| u[k].as_u64().unwrap_or(0);
            let mut total = self.usage.unwrap_or_default();
            total.add(&Usage {
                input_tokens: n("input_tokens"),
                cached_input_tokens: n("cached_input_tokens"),
                output_tokens: n("output_tokens"),
                reasoning_output_tokens: n("reasoning_output_tokens"),
            });
            self.usage = Some(total);
        } else if v["type"] == "result" {
            let u = &v["usage"];
            let n = |k: &str| u[k].as_u64().unwrap_or(0);
            let cached = n("cache_read_input_tokens");
            self.usage = Some(Usage {
                input_tokens: n("input_tokens") + cached,
                cached_input_tokens: cached,
                output_tokens: n("output_tokens"),
                reasoning_output_tokens: 0,
            });
            self.result = Some(ClaudeResult {
                text: v["result"].as_str().unwrap_or_default().to_string(),
                is_error: v["is_error"] == true,
                subtype: v["subtype"].as_str().map(str::to_string),
            });
        }
        true
    }
}

fn update(root: &Path, track: &str, f: impl FnOnce(&mut Status)) {
    if let Ok(mut s) = status::read(root, track) {
        f(&mut s);
        s.updated = now_secs().max(s.started);
        let _ = status::write(root, &s);
    }
}

fn run_agent(
    dir: &Path,
    root: &Path,
    track: &str,
    launch: &Launch,
    args: &[String],
    tally: &mut Tally,
) -> i32 {
    let mut cmd = Command::new(&launch.program);
    cmd.args(args).current_dir(&launch.cwd);
    // `current_dir` does not touch `PWD`: a shell's `cd` does, and a tool that trusts `PWD` would
    // otherwise see the supervisor's directory (the main checkout).
    for k in &launch.env_remove {
        cmd.env_remove(k);
    }
    cmd.env("PWD", &launch.cwd);
    for (k, v) in &launch.env {
        cmd.env(k, v);
    }
    let stderr =
        std::fs::File::create(dir.join(files::STDERR)).map_or_else(|_| Stdio::null(), Stdio::from);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(stderr);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    match crate::exec::spawn_retry(&mut cmd) {
        Err(e) => {
            let _ = std::fs::write(
                dir.join(files::STDERR),
                format!("could not start the agent: {e}\n"),
            );
            -1
        }
        Ok(mut child) => {
            let _ = std::fs::write(dir.join(files::PID), child.id().to_string());
            let out = child.stdout.take();
            let mut events = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(dir.join(files::EVENTS))
                .ok();
            if let Some(out) = out {
                for line in BufReader::new(out).lines().map_while(Result::ok) {
                    if let Some(f) = events.as_mut() {
                        let _ = writeln!(f, "{line}");
                        let _ = f.flush();
                    }
                    if tally.line(&line) {
                        let (n, u) = (tally.events, tally.usage);
                        update(root, track, |s| {
                            s.events = n;
                            s.tokens = u;
                        });
                    }
                }
            }
            child.wait().ok().and_then(|s| s.code()).unwrap_or(-1)
        }
    }
}

/// The reason recorded when the agent changed files outside its worktree.
#[must_use]
pub fn escape_reason(paths: &[String]) -> String {
    let shown: Vec<&str> = paths.iter().take(10).map(String::as_str).collect();
    let more = paths.len().saturating_sub(shown.len());
    format!(
        "wrote-outside-worktree: the agent changed the main checkout instead of its worktree: {}{}",
        shown.join(", "),
        if more > 0 {
            format!(" (and {more} more)")
        } else {
            String::new()
        }
    )
}

/// The directories the guard leaves out: the worktree, when it sits inside the guarded checkout.
fn guard_skip(launch: &Launch) -> Vec<PathBuf> {
    vec![PathBuf::from(&launch.cwd)]
}

/// The guarded checkout's state before the run, when the launch asks for the guard and the checkout
/// is not the working directory itself.
fn guard_before(launch: &Launch) -> Option<BTreeMap<String, String>> {
    let main = Path::new(launch.guard_root.as_deref()?);
    if same_dir(main, Path::new(&launch.cwd)) {
        return None;
    }
    super::gitx::snapshot(main, &guard_skip(launch)).ok()
}

/// What changed in the guarded checkout since `before`; written to `escaped.txt` when not empty.
fn guard_after(
    dir: &Path,
    launch: &Launch,
    before: Option<&BTreeMap<String, String>>,
) -> Vec<String> {
    let (Some(before), Some(main)) = (before, launch.guard_root.as_deref()) else {
        return Vec::new();
    };
    let Ok(after) = super::gitx::snapshot(Path::new(main), &guard_skip(launch)) else {
        return Vec::new();
    };
    let changed = super::gitx::changed_since(before, &after);
    if !changed.is_empty() {
        let _ = std::fs::write(dir.join(files::ESCAPED), changed.join("\n"));
    }
    changed
}

fn same_dir(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

/// Run the agent of the track directory `dir` to its end.
///
/// # Errors
/// When `launch.json` or the status file is unreadable. A failure to start the agent is recorded as a
/// failed track, not returned.
pub fn watch(dir: &Path) -> Res<()> {
    let root = dir
        .ancestors()
        .nth(3)
        .ok_or_else(|| Error::Invalid("not a track directory".into()))?
        .to_path_buf();
    let track = dir
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| Error::Invalid("not a track directory".into()))?
        .to_string();
    let launch: Launch = serde_json::from_str(&io(
        "read launch.json",
        std::fs::read_to_string(dir.join(files::LAUNCH)),
    )?)
    .map_err(|e| Error::Invalid(format!("launch.json: {e}")))?;
    let brief = std::fs::read_to_string(dir.join(BRIEF_FILE)).unwrap_or_default();
    let args: Vec<String> = launch
        .args
        .iter()
        .map(|a| {
            if a == "{brief}" {
                brief.clone()
            } else {
                a.clone()
            }
        })
        .collect();

    let guard = guard_before(&launch);
    let mut tally = Tally::default();
    let code = run_agent(dir, &root, &track, &launch, &args, &mut tally);
    let escaped = guard_after(dir, &launch, guard.as_ref());
    let ended = now_secs();
    let stopped = dir.join(files::STOPPED).exists();
    let claude = tally.result.clone();
    if let Some(r) = &claude
        && !r.text.trim().is_empty()
        && !dir.join(files::LAST).exists()
    {
        let _ = std::fs::write(dir.join(files::LAST), &r.text);
    }
    let errored = claude.as_ref().is_some_and(|r| r.is_error) || !escaped.is_empty();
    let reason = if escaped.is_empty() {
        claude
            .as_ref()
            .filter(|r| r.is_error)
            .map(ClaudeResult::reason)
    } else {
        Some(escape_reason(&escaped))
    };
    let _ = std::fs::remove_file(dir.join(files::PID));
    update(&root, &track, |s| {
        s.state = if stopped {
            State::TakenBack
        } else if code == 0 && !errored && escaped.is_empty() {
            State::Done
        } else {
            State::Failed
        };
        s.events = tally.events;
        s.tokens = tally.usage;
    });
    if let Ok(s) = status::read(&root, &track) {
        let _ = status::append_log(&root, &LogLine::from_status(&s, ended));
    }
    let _ = status::atomic_write(
        &dir.join(files::EXIT),
        serde_json::to_string(&ExitRecord {
            code,
            ended,
            is_error: errored,
            reason,
        })
        .unwrap_or_default()
        .as_bytes(),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tally_reads_the_claude_result_object() {
        let mut t = Tally::default();
        assert!(t.line(
            r#"{"type":"result","subtype":"error_max_turns","is_error":true,"result":"stopped","usage":{"input_tokens":200,"cache_read_input_tokens":800,"output_tokens":50}}"#
        ));
        assert_eq!(t.events, 1);
        assert_eq!(
            t.usage,
            Some(Usage {
                input_tokens: 1000,
                cached_input_tokens: 800,
                output_tokens: 50,
                reasoning_output_tokens: 0
            })
        );
        let r = t.result.unwrap();
        assert!(r.is_error);
        assert_eq!(r.reason(), "max-turns");
        assert_eq!(r.text, "stopped");
    }

    #[test]
    fn tally_counts_json_events_and_sums_usage() {
        let mut t = Tally::default();
        assert!(t.line(r#"{"type":"thread.started"}"#));
        assert!(!t.line("Reading additional input from stdin..."));
        assert!(!t.line("42"));
        assert!(t.line(r#"{"type":"item.started","item":{}}"#));
        assert!(t.line(r#"{"type":"error","message":"apply_patch verification failed"}"#));
        assert!(t.line(
            r#"{"type":"turn.completed","usage":{"input_tokens":1000,"cached_input_tokens":800,"output_tokens":50,"reasoning_output_tokens":10,"new_field":1}}"#
        ));
        assert_eq!(t.events, 4);
        assert_eq!(
            t.usage,
            Some(Usage {
                input_tokens: 1000,
                cached_input_tokens: 800,
                output_tokens: 50,
                reasoning_output_tokens: 10
            })
        );
    }
}
