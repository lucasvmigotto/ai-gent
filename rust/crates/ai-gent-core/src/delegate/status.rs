//! The per-track status file (`contracts/status.md`) and the delegation log (FR-013, FR-016).
//!
//! `status.json` is written atomically (a temp name, then a rename). It never holds a prompt, a brief,
//! credentials or command output: the type has no field for them and refuses unknown ones.
use super::{Error, Res, io, now_secs, state};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::Path;

/// Where a track stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum State {
    /// Waiting for a worker.
    Queued,
    /// A worker is on it.
    Running,
    /// No event for the configured time.
    Stalled,
    /// Finished with exit code 0.
    Done,
    /// Finished with an error.
    Failed,
    /// The supervisor took the task back.
    TakenBack,
}

/// What the worker does on a track.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Role {
    /// Writes the change.
    Builder,
    /// Reviews a builder's diff.
    Reviewer,
    /// Reads and reports.
    Researcher,
}

/// The supervisor's result on a track.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VerdictResult {
    /// Not checked yet.
    Pending,
    /// Passed the checks.
    Accepted,
    /// Refused; the reason is a rule id.
    Rejected,
}

/// The supervisor's verdict: a result and, when refused, a rule id (never a diff or a brief).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Verdict {
    /// Pending, accepted or rejected.
    pub result: VerdictResult,
    /// A rule id from `checks::REASONS`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl Verdict {
    /// Not checked yet.
    #[must_use]
    pub const fn pending() -> Self {
        Self {
            result: VerdictResult::Pending,
            reason: None,
        }
    }
}

/// Usage an agent reports (Codex: `turn.completed.usage`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Usage {
    /// Input tokens, cached ones included.
    #[serde(default)]
    pub input_tokens: u64,
    /// The cached part of the input.
    #[serde(default)]
    pub cached_input_tokens: u64,
    /// Output tokens.
    #[serde(default)]
    pub output_tokens: u64,
    /// Reasoning output tokens.
    #[serde(default)]
    pub reasoning_output_tokens: u64,
}

impl Usage {
    /// Add another report.
    pub const fn add(&mut self, o: &Self) {
        self.input_tokens += o.input_tokens;
        self.cached_input_tokens += o.cached_input_tokens;
        self.output_tokens += o.output_tokens;
        self.reasoning_output_tokens += o.reasoning_output_tokens;
    }
}

/// The contents of `status.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Status {
    /// Track id (the branch with `/` as `-`).
    pub track: String,
    /// `claude`, `claude-subagent`, `codex` or `opencode[:model]`.
    pub agent: String,
    /// Builder, reviewer or researcher.
    pub role: Role,
    /// Task ids from the spec.
    pub tasks: Vec<String>,
    /// Where the track stands.
    pub state: State,
    /// Epoch seconds.
    pub started: u64,
    /// Epoch seconds of the last change.
    pub updated: u64,
    /// Events so far.
    pub events: u64,
    /// Path relative to the repository root.
    pub worktree: String,
    /// The supervisor's result.
    pub verdict: Verdict,
    /// Usage the agent reported, or null.
    pub tokens: Option<Usage>,
}

/// Whether `agent` is one of the contract's values.
#[must_use]
pub fn valid_agent(agent: &str) -> bool {
    match agent {
        "claude" | "claude-subagent" | "codex" | "opencode" => true,
        _ => agent.strip_prefix("opencode:").is_some_and(|m| {
            !m.is_empty()
                && m.chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/'))
        }),
    }
}

/// Whether `id` looks like a task id (`T010`, `T010a`).
#[must_use]
pub fn valid_task_id(id: &str) -> bool {
    let rest = id.strip_prefix('T').unwrap_or("");
    let digits = rest.chars().take_while(char::is_ascii_digit).count();
    let tail = &rest[digits..];
    digits > 0
        && (tail.is_empty() || (tail.len() == 1 && tail.chars().all(|c| c.is_ascii_lowercase())))
}

fn relative_ok(p: &str) -> bool {
    !p.is_empty()
        && !p.starts_with('/')
        && !p.starts_with('\\')
        && !p.contains(':')
        && !p.split(['/', '\\']).any(|s| s == "..")
}

impl Status {
    /// A new status in `state` with the verdict pending.
    #[must_use]
    pub fn new(
        track: &str,
        agent: &str,
        role: Role,
        tasks: &[String],
        worktree: &str,
        state: State,
    ) -> Self {
        let now = now_secs();
        Self {
            track: track.to_string(),
            agent: agent.to_string(),
            role,
            tasks: tasks.to_vec(),
            state,
            started: now,
            updated: now,
            events: 0,
            worktree: worktree.to_string(),
            verdict: Verdict::pending(),
            tokens: None,
        }
    }

    /// What is wrong with this status (empty when it satisfies the contract).
    #[must_use]
    pub fn problems(&self) -> Vec<String> {
        let mut p = Vec::new();
        if !state::valid_track(&self.track) {
            p.push(format!("track '{}' is not a valid track id", self.track));
        }
        if !valid_agent(&self.agent) {
            p.push(format!("agent '{}' is not a known agent", self.agent));
        }
        for t in &self.tasks {
            if !valid_task_id(t) {
                p.push(format!("task '{t}' is not a task id"));
            }
        }
        if !relative_ok(&self.worktree) {
            p.push("worktree must be a path relative to the repository root".to_string());
        }
        if self.updated < self.started {
            p.push("updated is before started".to_string());
        }
        match (&self.verdict.result, &self.verdict.reason) {
            (VerdictResult::Rejected, None) => p.push("a rejected verdict needs a reason".into()),
            (VerdictResult::Rejected, Some(r)) if !rule_id(r) => {
                p.push(format!("verdict reason '{r}' is not a rule id"));
            }
            (VerdictResult::Pending | VerdictResult::Accepted, Some(_)) => {
                p.push("only a rejected verdict has a reason".into());
            }
            _ => {}
        }
        p
    }
}

fn rule_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 40
        && s.starts_with(|c: char| c.is_ascii_lowercase())
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Write `bytes` to `path` through a temp name in the same directory, then rename.
///
/// # Errors
/// When the directory cannot be created or the write or rename fails.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Res<()> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    io(
        format!("create {}", dir.display()),
        std::fs::create_dir_all(dir),
    )?;
    let name = path
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().to_string());
    let tmp = dir.join(format!(".{name}.tmp-{}", std::process::id()));
    let mut f = io(
        format!("create {}", tmp.display()),
        std::fs::File::create(&tmp),
    )?;
    io("write", f.write_all(bytes))?;
    io("sync", f.sync_all())?;
    drop(f);
    // On Windows the rename fails while a reader has the target open; readers are quick, so retry.
    let mut tries = 0;
    let renamed = loop {
        match std::fs::rename(&tmp, path) {
            Err(_) if tries < 80 => {
                tries += 1;
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
            r => break r,
        }
    };
    io(format!("rename to {}", path.display()), renamed).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

/// Write a track's status file atomically, refusing one that breaks the contract.
///
/// # Errors
/// [`Error::Invalid`] for a status with problems; I/O errors.
pub fn write(root: &Path, status: &Status) -> Res<()> {
    let problems = status.problems();
    if !problems.is_empty() {
        return Err(Error::Invalid(format!("status: {}", problems.join("; "))));
    }
    let path = state::track_dir(root, &status.track)?.join(state::files::STATUS);
    let mut text =
        serde_json::to_string_pretty(status).map_err(|e| Error::Invalid(format!("status: {e}")))?;
    text.push('\n');
    atomic_write(&path, text.as_bytes())
}

/// Read a track's status file.
///
/// # Errors
/// When the file is missing, unreadable, malformed or breaks the contract.
pub fn read(root: &Path, track: &str) -> Res<Status> {
    let path = state::track_dir(root, track)?.join(state::files::STATUS);
    let text = io(
        format!("read {}", path.display()),
        std::fs::read_to_string(&path),
    )?;
    let s: Status = serde_json::from_str(&text)
        .map_err(|e| Error::Invalid(format!("{}: {e}", path.display())))?;
    let problems = s.problems();
    if problems.is_empty() {
        Ok(s)
    } else {
        Err(Error::Invalid(format!(
            "{}: {}",
            path.display(),
            problems.join("; ")
        )))
    }
}

/// One line of `.ai-gent/delegation.log`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LogLine {
    /// Epoch seconds when the run ended.
    pub ts: u64,
    /// Track id.
    pub track: String,
    /// The worker.
    pub agent: String,
    /// Task ids.
    pub tasks: Vec<String>,
    /// Wall time from the adapter's own clock.
    pub wall_secs: u64,
    /// Usage when the agent reports it.
    pub tokens: Option<Usage>,
    /// The final state.
    pub result: State,
    /// Token tags (`shared/token-tags.json`): always `["delegate"]`.
    pub tags: Vec<String>,
}

impl LogLine {
    /// The line for a finished (or taken back) track.
    #[must_use]
    pub fn from_status(s: &Status, ended: u64) -> Self {
        Self {
            ts: ended,
            track: s.track.clone(),
            agent: s.agent.clone(),
            tasks: s.tasks.clone(),
            wall_secs: ended.saturating_sub(s.started),
            tokens: s.tokens,
            result: s.state,
            tags: vec!["delegate".to_string()],
        }
    }
}

/// Append one line to the delegation log.
///
/// # Errors
/// When the log cannot be opened or written.
pub fn append_log(root: &Path, line: &LogLine) -> Res<()> {
    let path = state::log_path(root);
    if let Some(dir) = path.parent() {
        io(
            format!("create {}", dir.display()),
            std::fs::create_dir_all(dir),
        )?;
    }
    let mut text = serde_json::to_string(line).map_err(|e| Error::Invalid(e.to_string()))?;
    text.push('\n');
    let mut f = io(
        format!("open {}", path.display()),
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path),
    )?;
    io("write the delegation log", f.write_all(text.as_bytes()))
}

/// Every well-formed line of the delegation log (a malformed line is skipped).
///
/// # Errors
/// When the log exists and cannot be read.
pub fn read_log(root: &Path) -> Res<Vec<LogLine>> {
    let path = state::log_path(root);
    match std::fs::read_to_string(&path) {
        Ok(t) => Ok(t
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => io(format!("read {}", path.display()), Err(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Status {
        Status::new(
            "feat-x",
            "codex",
            Role::Builder,
            &["T001".into(), "T010a".into()],
            ".worktrees/feat-x",
            State::Running,
        )
    }

    #[test]
    fn roundtrip_is_atomic_and_leaves_no_temp() {
        let d = tempfile::tempdir().unwrap();
        let s = sample();
        write(d.path(), &s).unwrap();
        assert_eq!(read(d.path(), "feat-x").unwrap(), s);
        let dir = d.path().join(".ai-gent/delegate/feat-x");
        let names: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, vec![std::ffi::OsString::from("status.json")]);
    }

    #[test]
    fn schema_has_exactly_the_contract_fields() {
        let v = serde_json::to_value(sample()).unwrap();
        let mut keys: Vec<_> = v.as_object().unwrap().keys().cloned().collect();
        keys.sort();
        assert_eq!(
            keys,
            [
                "agent", "events", "role", "started", "state", "tasks", "tokens", "track",
                "updated", "verdict", "worktree"
            ]
        );
        assert!(v["tokens"].is_null());
        assert_eq!(v["state"], "running");
    }

    #[test]
    fn states_use_the_contract_spelling() {
        let s = serde_json::to_string(&State::TakenBack).unwrap();
        assert_eq!(s, "\"taken-back\"");
    }

    #[test]
    fn unknown_fields_are_refused() {
        let mut v = serde_json::to_value(sample()).unwrap();
        v["prompt"] = "the brief".into();
        assert!(serde_json::from_value::<Status>(v).is_err());
    }

    #[test]
    fn contract_violations() {
        let mut s = sample();
        s.agent = "gpt".into();
        s.tasks = vec!["x1".into()];
        s.worktree = "/abs".into();
        s.track = "A/b".into();
        assert_eq!(s.problems().len(), 4);
        let d = tempfile::tempdir().unwrap();
        assert!(write(d.path(), &s).is_err());
        let mut s = sample();
        s.verdict = Verdict {
            result: VerdictResult::Rejected,
            reason: None,
        };
        assert_eq!(s.problems().len(), 1);
        s.verdict.reason = Some("path-outside-allowed".into());
        assert!(s.problems().is_empty());
        s.verdict.reason = Some("a long diff text here".into());
        assert_eq!(s.problems().len(), 1);
    }

    #[test]
    fn agents() {
        for a in [
            "claude",
            "claude-subagent",
            "codex",
            "opencode",
            "opencode:flint-40b-free",
        ] {
            assert!(valid_agent(a), "{a}");
        }
        assert!(!valid_agent("opencode:"));
        assert!(!valid_agent("cursor"));
    }

    #[test]
    fn log_lines_carry_the_delegate_tag() {
        let d = tempfile::tempdir().unwrap();
        let mut s = sample();
        s.state = State::Done;
        s.started = 100;
        s.tokens = Some(Usage {
            input_tokens: 5,
            ..Usage::default()
        });
        let l = LogLine::from_status(&s, 160);
        append_log(d.path(), &l).unwrap();
        append_log(d.path(), &l).unwrap();
        let back = read_log(d.path()).unwrap();
        assert_eq!(back.len(), 2);
        assert_eq!(back[0].wall_secs, 60);
        assert_eq!(back[0].tags, ["delegate"]);
        assert_eq!(read_log(&d.path().join("none")).unwrap().len(), 0);
    }
}
