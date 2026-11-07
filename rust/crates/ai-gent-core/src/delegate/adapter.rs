//! The adapter contract (`contracts/adapter.md`): what the skill calls for any delegate agent, and the
//! parts every adapter shares (the background launch, `status`, `stop` by PID file, `result`).
//!
//! An adapter never decides what to build. It starts an agent with stdin closed, keeps its events and
//! final message under `.ai-gent/delegate/<track>/`, and answers from those files only.
use super::state::{self, files};
use super::status::{self, Role, State, Status, Usage};
use super::{Error, Res, io, now_secs};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// What `detect` found.
#[derive(Debug, Clone, Serialize)]
pub struct Detect {
    /// Whether the CLI is on `PATH` and runs.
    pub found: bool,
    /// Where it is.
    pub path: Option<String>,
    /// First line of `--version`.
    pub version: Option<String>,
}

/// One preflight check.
#[derive(Debug, Clone, Serialize)]
pub struct Check {
    /// A stable name.
    pub name: &'static str,
    /// Whether it passed.
    pub ok: bool,
    /// What was found.
    pub detail: String,
}

/// What `preflight` proved.
#[derive(Debug, Clone, Serialize)]
pub struct Preflight {
    /// Every check passed.
    pub ok: bool,
    /// The checks.
    pub checks: Vec<Check>,
    /// Facts the run depends on that cannot be probed.
    pub notes: Vec<String>,
}

/// What `run` is asked to start.
#[derive(Debug, Clone)]
pub struct RunRequest {
    /// The track id.
    pub track: String,
    /// The worktree (absolute).
    pub worktree: PathBuf,
    /// The brief (see `brief::build`).
    pub brief: String,
    /// Task ids.
    pub tasks: Vec<String>,
    /// The role.
    pub role: Role,
    /// Seconds without an event after which the track counts as stalled.
    pub stall_secs: u64,
    /// How long `run` waits for the first event before it stops the agent (zero: do not wait).
    pub first_event_wait: Duration,
}

/// A started run.
#[derive(Debug, Clone, Serialize)]
pub struct RunHandle {
    /// The track id.
    pub track: String,
    /// Its directory.
    pub dir: String,
    /// The agent's process id, when already known.
    pub pid: Option<u32>,
}

/// What `status` answers.
#[derive(Debug, Clone, Serialize)]
pub struct TrackStatus {
    /// The track id.
    pub track: String,
    /// The state, with a silent running agent shown as stalled.
    pub state: State,
    /// Whether the agent is still working (running or stalled).
    pub running: bool,
    /// Events so far.
    pub events: u64,
    /// `item.started` events.
    pub items_started: u64,
    /// `item.completed` events.
    pub items_completed: u64,
    /// Seconds since the last event.
    pub last_event_age_secs: u64,
}

/// What `stop` did.
#[derive(Debug, Clone, Serialize)]
pub struct StopOutcome {
    /// The pid from the PID file, when there was a running agent.
    pub pid: Option<u32>,
    /// Whether a stop signal was sent.
    pub signalled: bool,
    /// Whether the run had ended when `stop` returned.
    pub exited: bool,
}

/// What `result` answers.
#[derive(Debug, Clone, Serialize)]
pub struct RunResult {
    /// The track id.
    pub track: String,
    /// Whether the run has ended.
    pub finished: bool,
    /// The state.
    pub state: State,
    /// The agent's exit code, once it ended.
    pub exit_code: Option<i32>,
    /// The agent reported an error in its final report although it exited 0.
    pub is_error: bool,
    /// Why the run failed when `is_error` (for example `max-turns`).
    pub reason: Option<String>,
    /// The agent's final message.
    pub message: Option<String>,
    /// Usage the agent reported.
    pub usage: Option<Usage>,
    /// Wall time from the adapter's clock.
    pub wall_secs: u64,
}

/// One delegate agent.
pub trait Adapter {
    /// The agent id (`codex`).
    fn id(&self) -> &'static str;
    /// Is the CLI on `PATH`, and does it run? Does not read the agent's auth files.
    fn detect(&self) -> Detect;
    /// A cheap call proving it can start. A failure is "not logged in", never worked around.
    ///
    /// # Errors
    /// [`Error::NotLoggedIn`] or the cause of the failure.
    fn ready(&self) -> Res<()>;
    /// Proves the run can succeed: stdin closed, the worktree writable, the agent starts. It does not
    /// expect the delegate to commit.
    fn preflight(&self, worktree: &Path) -> Preflight;
    /// Starts the agent in the background with stdin closed.
    ///
    /// # Errors
    /// When the request is invalid, the track is already running, or the agent gives no first event.
    fn run(&self, req: &RunRequest) -> Res<RunHandle>;
    /// Last event time, event count, running or exited.
    ///
    /// # Errors
    /// When the track has no readable status file.
    fn status(&self, track: &str) -> Res<TrackStatus>;
    /// Stops by the PID file, never by pattern.
    ///
    /// # Errors
    /// When the track id is invalid.
    fn stop(&self, track: &str) -> Res<StopOutcome>;
    /// The final message, the exit code, usage when the agent reports it.
    ///
    /// # Errors
    /// When the track has no readable status file.
    fn result(&self, track: &str) -> Res<RunResult>;
}

/// How the watcher starts the agent (`launch.json`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Launch {
    /// The agent program.
    pub program: String,
    /// Its arguments; the single argument `{brief}` is replaced by `brief.txt`.
    pub args: Vec<String>,
    /// The working directory.
    pub cwd: String,
    /// Extra environment variables.
    pub env: Vec<(String, String)>,
    /// Seconds of silence after which the track counts as stalled.
    pub stall_secs: u64,
    /// Inherited variables removed from the agent's environment (they could name another checkout).
    #[serde(default)]
    pub env_remove: Vec<String>,
    /// The main checkout the watcher guards: its `git status` before and after the run is compared,
    /// and a file that changed there fails the track (`wrote-outside-worktree`). Empty: no guard.
    #[serde(default)]
    pub guard_root: Option<String>,
}

/// The file holding the brief of a track (local, git-excluded).
pub const BRIEF_FILE: &str = "brief.txt";

fn read_json<T: for<'a> Deserialize<'a>>(path: &Path) -> Option<T> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

/// The exit record (`exit.json`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExitRecord {
    /// The agent's exit code (-1 when it was killed by a signal or never started).
    pub code: i32,
    /// Epoch seconds.
    pub ended: u64,
    /// The agent exited 0 but reported `is_error: true` (Claude's final object).
    #[serde(default)]
    pub is_error: bool,
    /// Why such a run failed (`max-turns`, ...).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// Launch `launch` in the background through the watcher (`watch_exe delegate _watch <dir>`).
///
/// # Errors
/// See [`Adapter::run`].
pub fn launch(
    root: &Path,
    watch_exe: &Path,
    req: &RunRequest,
    agent: &str,
    launch: &Launch,
) -> Res<RunHandle> {
    let dir = state::track_dir(root, &req.track)?;
    if let Ok(s) = status::read(root, &req.track)
        && matches!(s.state, State::Running | State::Stalled)
        && dir.join(files::PID).exists()
        && !dir.join(files::EXIT).exists()
    {
        return Err(Error::Invalid(format!(
            "track '{}' is already running",
            req.track
        )));
    }
    io(
        format!("create {}", dir.display()),
        std::fs::create_dir_all(&dir),
    )?;
    for f in [
        files::EVENTS,
        files::LAST,
        files::PID,
        files::EXIT,
        files::STOPPED,
        files::STDERR,
    ] {
        let _ = std::fs::remove_file(dir.join(f));
    }
    let canon = |p: &Path| {
        let c = std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
        PathBuf::from(super::slash(&c).trim_start_matches("//?/"))
    };
    let (root_c, wt_c) = (canon(root), canon(&req.worktree));
    let rel = wt_c.strip_prefix(&root_c).map_err(|_| {
        Error::Invalid(format!(
            "worktree {} is not inside the repository root {}",
            req.worktree.display(),
            root.display()
        ))
    })?;
    let rel = if rel.as_os_str().is_empty() {
        ".".to_string()
    } else {
        super::slash(rel)
    };
    let st = Status::new(
        &req.track,
        agent,
        req.role,
        &req.tasks,
        &rel,
        State::Running,
    );
    status::write(root, &st)?;
    io(
        "write the brief",
        std::fs::write(dir.join(BRIEF_FILE), &req.brief),
    )?;
    let text = serde_json::to_string_pretty(launch).map_err(|e| Error::Invalid(e.to_string()))?;
    status::atomic_write(&dir.join(files::LAUNCH), text.as_bytes())?;

    let mut cmd = Command::new(watch_exe);
    cmd.args(["delegate", "_watch"])
        .arg(&dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    detach(&mut cmd);
    let _child = io("start the watcher", crate::exec::spawn_retry(&mut cmd))?;

    let wait = req.first_event_wait;
    let begin = Instant::now();
    if !wait.is_zero() {
        loop {
            let s = status::read(root, &req.track)?;
            if s.events > 0 {
                break;
            }
            if !matches!(s.state, State::Running) {
                let code = read_json::<ExitRecord>(&dir.join(files::EXIT)).map_or(-1, |e| e.code);
                return Err(Error::Invalid(format!(
                    "the agent ended before any event (exit code {code}); see {}",
                    dir.join(files::STDERR).display()
                )));
            }
            if begin.elapsed() >= wait {
                let _ = stop_track(root, &req.track);
                return Err(Error::NoFirstEvent(wait.as_secs()));
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }
    let pid = std::fs::read_to_string(dir.join(files::PID))
        .ok()
        .and_then(|t| t.trim().parse().ok());
    Ok(RunHandle {
        track: req.track.clone(),
        dir: super::slash(&dir),
        pid,
    })
}

#[cfg(unix)]
fn detach(_cmd: &mut Command) {}

#[cfg(windows)]
fn detach(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    // DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP
    cmd.creation_flags(0x0000_0008 | 0x0000_0200);
}

/// `status` for any adapter: from `status.json` and `events.jsonl` only.
///
/// # Errors
/// When the status file is missing or malformed.
pub fn track_status(root: &Path, track: &str) -> Res<TrackStatus> {
    let s = status::read(root, track)?;
    let dir = state::track_dir(root, track)?;
    let age = now_secs().saturating_sub(s.updated);
    let stall = read_json::<Launch>(&dir.join(files::LAUNCH)).map_or(600, |l| l.stall_secs);
    let state = if s.state == State::Running && age > stall {
        State::Stalled
    } else {
        s.state
    };
    let (mut started, mut completed) = (0, 0);
    if let Ok(text) = std::fs::read_to_string(dir.join(files::EVENTS)) {
        for line in text.lines() {
            match serde_json::from_str::<serde_json::Value>(line)
                .ok()
                .as_ref()
                .and_then(|v| v["type"].as_str().map(str::to_string))
                .as_deref()
            {
                Some("item.started") => started += 1,
                Some("item.completed") => completed += 1,
                _ => {}
            }
        }
    }
    Ok(TrackStatus {
        track: track.to_string(),
        state,
        running: matches!(state, State::Running | State::Stalled),
        events: s.events,
        items_started: started,
        items_completed: completed,
        last_event_age_secs: age,
    })
}

#[cfg(unix)]
fn signal_group(pid: u32, sig: &str) -> bool {
    Command::new("kill")
        .args(["-s", sig, "--"])
        .arg(format!("-{pid}"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

#[cfg(windows)]
fn signal_group(pid: u32, _sig: &str) -> bool {
    Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// `stop` for any adapter: the PID file names the process; nothing is matched by pattern.
///
/// # Errors
/// When the track id is invalid.
pub fn stop_track(root: &Path, track: &str) -> Res<StopOutcome> {
    let dir = state::track_dir(root, track)?;
    let pid: Option<u32> = std::fs::read_to_string(dir.join(files::PID))
        .ok()
        .and_then(|t| t.trim().parse().ok());
    let ended = || dir.join(files::EXIT).exists();
    let Some(pid) = pid.filter(|_| !ended()) else {
        return Ok(StopOutcome {
            pid: None,
            signalled: false,
            exited: ended(),
        });
    };
    let _ = std::fs::write(dir.join(files::STOPPED), b"");
    let signalled = signal_group(pid, "TERM");
    let wait = |secs: u64| {
        let t = Instant::now();
        while t.elapsed() < Duration::from_secs(secs) {
            if ended() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        ended()
    };
    let mut exited = wait(5);
    if !exited {
        signal_group(pid, "KILL");
        exited = wait(5);
    }
    Ok(StopOutcome {
        pid: Some(pid),
        signalled,
        exited,
    })
}

/// `result` for any adapter.
///
/// # Errors
/// When the status file is missing or malformed.
pub fn track_result(root: &Path, track: &str) -> Res<RunResult> {
    let s = status::read(root, track)?;
    let dir = state::track_dir(root, track)?;
    let exit = read_json::<ExitRecord>(&dir.join(files::EXIT));
    let message = std::fs::read_to_string(dir.join(files::LAST))
        .ok()
        .filter(|m| !m.trim().is_empty());
    let end = exit.as_ref().map_or_else(now_secs, |e| e.ended);
    Ok(RunResult {
        track: track.to_string(),
        finished: exit.is_some(),
        state: s.state,
        is_error: exit.as_ref().is_some_and(|e| e.is_error),
        reason: exit.as_ref().and_then(|e| e.reason.clone()),
        exit_code: exit.map(|e| e.code),
        message,
        usage: s.tokens,
        wall_secs: end.saturating_sub(s.started),
    })
}
