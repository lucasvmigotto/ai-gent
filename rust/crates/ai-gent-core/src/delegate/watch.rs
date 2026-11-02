//! The watcher: a small background process (`ai-gent delegate _watch <dir>`) that starts the agent from
//! `launch.json`, copies its events to `events.jsonl`, keeps `status.json` current, and writes the exit
//! record, the final status and the delegation log line when the agent ends. It is what lets `run`
//! return at once while the adapter's clock, event count and usage stay correct.
use super::adapter::{BRIEF_FILE, ExitRecord, Launch};
use super::state::files;
use super::status::{self, LogLine, State, Status, Usage};
use super::{Error, Res, io, now_secs};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};

/// What one agent event adds up to.
#[derive(Debug, Default, Clone, Copy)]
pub struct Tally {
    /// Events seen.
    pub events: u64,
    /// Usage from `turn.completed` events.
    pub usage: Option<Usage>,
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

    let mut tally = Tally::default();
    let code = run_agent(dir, &root, &track, &launch, &args, &mut tally);
    let ended = now_secs();
    let stopped = dir.join(files::STOPPED).exists();
    let _ = std::fs::remove_file(dir.join(files::PID));
    update(&root, &track, |s| {
        s.state = if stopped {
            State::TakenBack
        } else if code == 0 {
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
        serde_json::to_string(&ExitRecord { code, ended })
            .unwrap_or_default()
            .as_bytes(),
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
