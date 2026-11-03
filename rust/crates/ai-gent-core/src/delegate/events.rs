//! Completion events (spec 041, T022): an append-only `.ai-gent/delegate/events.jsonl` with one line
//! per terminal or attention transition of a track (`done`, `failed`, `stalled`, `taken-back`), and
//! `.ai-gent/delegate/acks.jsonl` with the ids a coordinator has acknowledged.
//!
//! An event holds only the fields of `status.json` a coordinator needs to act: no brief, no result
//! text, no path. Its id is `track:started:state`, so an event of an earlier run of the same track is
//! told apart from the current one and a restart never appends a transition twice.
use super::adapter::Launch;
use super::state::{self, files};
use super::status::{Role, State, Status, VerdictResult};
use super::{Error, Res, io, now_secs};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// One completion event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    /// `track:started:state`.
    pub id: String,
    /// Epoch seconds when the event was recorded.
    pub ts: u64,
    /// Track id.
    pub track: String,
    /// The worker.
    pub agent: String,
    /// The worker's role.
    pub role: Role,
    /// Task ids.
    pub tasks: Vec<String>,
    /// The transition: done, failed, stalled or taken-back.
    pub state: State,
    /// The verdict's reason, else its result when not pending.
    pub result: Option<String>,
}

fn state_name(s: State) -> String {
    serde_json::to_value(s)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// Whether a transition into `s` is announced.
#[must_use]
pub const fn announced(s: State) -> bool {
    matches!(
        s,
        State::Done | State::Failed | State::Stalled | State::TakenBack
    )
}

/// The event id of a track's run (`started`) reaching `state`.
#[must_use]
pub fn event_id(track: &str, started: u64, state: State) -> String {
    format!("{track}:{started}:{}", state_name(state))
}

impl Event {
    /// The event for `s` reaching `state` at `ts`.
    #[must_use]
    pub fn new(s: &Status, state: State, ts: u64) -> Self {
        let result = s.verdict.reason.clone().or_else(|| match s.verdict.result {
            VerdictResult::Pending => None,
            VerdictResult::Accepted => Some("accepted".to_string()),
            VerdictResult::Rejected => Some("rejected".to_string()),
        });
        Self {
            id: event_id(&s.track, s.started, state),
            ts,
            track: s.track.clone(),
            agent: s.agent.clone(),
            role: s.role,
            tasks: s.tasks.clone(),
            state,
            result,
        }
    }

    /// One greppable line: `event id=... ts=... track=... agent=... role=... tasks=... state=... result=...`.
    #[must_use]
    pub fn line(&self) -> String {
        let role = serde_json::to_value(self.role)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_default();
        format!(
            "event id={} ts={} track={} agent={} role={} tasks={} state={} result={}",
            self.id,
            self.ts,
            self.track,
            self.agent,
            role,
            self.tasks.join(","),
            state_name(self.state),
            self.result.as_deref().unwrap_or("-")
        )
    }
}

/// `.ai-gent/delegate/events.jsonl`.
#[must_use]
pub fn events_path(root: &Path) -> PathBuf {
    root.join(".ai-gent").join("delegate").join("events.jsonl")
}

/// `.ai-gent/delegate/acks.jsonl`.
#[must_use]
pub fn acks_path(root: &Path) -> PathBuf {
    root.join(".ai-gent").join("delegate").join("acks.jsonl")
}

/// An exclusive advisory lock on a lock file next to the log. The operating system releases it when
/// the file closes (drop, or the process dying), so a crash never leaves it held and there is no age
/// heuristic. The lock file itself stays, empty.
struct Lock(#[allow(dead_code)] std::fs::File);

/// Take the lock of `path` (`File::try_lock`, stable since Rust 1.89; MSRV is 1.92, toolchain 1.98.1).
fn lock(path: &Path) -> Res<Lock> {
    if let Some(dir) = path.parent() {
        io(
            format!("create {}", dir.display()),
            std::fs::create_dir_all(dir),
        )?;
    }
    let lock_path = path.with_extension("lock");
    let f = io(
        format!("open {}", lock_path.display()),
        std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock_path),
    )?;
    let begin = Instant::now();
    loop {
        match f.try_lock() {
            Ok(()) => return Ok(Lock(f)),
            Err(std::fs::TryLockError::WouldBlock) => {
                if begin.elapsed() > Duration::from_secs(15) {
                    return Err(Error::Invalid(format!("{} is locked", path.display())));
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(std::fs::TryLockError::Error(source)) => {
                return Err(Error::Io {
                    context: format!("lock {}", lock_path.display()),
                    source,
                });
            }
        }
    }
}

/// Append one line (a single `write`, so concurrent writers never interleave).
fn append_line(path: &Path, line: &str) -> Res<()> {
    let mut f = io(
        format!("open {}", path.display()),
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path),
    )?;
    let mut bytes = line.as_bytes().to_vec();
    bytes.push(b'\n');
    io("append", f.write_all(&bytes))?;
    io("sync", f.sync_data())
}

fn parse_lines<T: for<'de> Deserialize<'de>>(path: &Path) -> Vec<T> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

/// Every event in the log, oldest first (malformed lines are skipped).
#[must_use]
pub fn read_all(root: &Path) -> Vec<Event> {
    parse_lines(&events_path(root))
}

/// Append `ev` unless its id is anywhere in the log (read whole, under the lock).
///
/// # Errors
/// I/O errors, or a lock that stays taken.
///
/// Returns `Ok(false)` for a duplicate, which is refused and leaves the log unchanged.
pub fn append(root: &Path, ev: &Event) -> Res<bool> {
    let path = events_path(root);
    let _guard = lock(&path)?;
    let seen: HashSet<String> = read_all(root).into_iter().map(|e| e.id).collect();
    if seen.contains(&ev.id) {
        return Ok(false);
    }
    let line = serde_json::to_string(ev).map_err(|e| Error::Invalid(e.to_string()))?;
    append_line(&path, &line)?;
    Ok(true)
}

/// Record the transition a status file now shows, when it is one that is announced.
///
/// # Errors
/// As [`append`].
pub fn record(root: &Path, s: &Status) -> Res<bool> {
    if announced(s.state) {
        append(root, &Event::new(s, s.state, now_secs()))
    } else {
        Ok(false)
    }
}

/// Look at every track and record the transitions not yet in the log, `stalled` included (it is
/// computed from the clock, never written to `status.json`). Returns how many events were appended.
///
/// # Errors
/// As [`append`].
pub fn sync(root: &Path) -> Res<usize> {
    sync_with(root, |_| {})
}

/// The state a status shows now: `running` past its launch's `stall_secs` reads `stalled`.
fn effective_state(root: &Path, s: &Status) -> State {
    if s.state != State::Running {
        return s.state;
    }
    let stall = state::track_dir(root, &s.track)
        .ok()
        .and_then(|d| std::fs::read_to_string(d.join(files::LAUNCH)).ok())
        .and_then(|t| serde_json::from_str::<Launch>(&t).ok())
        .map_or(600, |l| l.stall_secs);
    if now_secs().saturating_sub(s.updated) > stall {
        State::Stalled
    } else {
        State::Running
    }
}

/// [`sync`] calling `after_read(track)` once a track's status is read (a test seam for a retry
/// replacing it then). The event comes from that one read, so state, `started` and tasks are one run.
fn sync_with(root: &Path, mut after_read: impl FnMut(&str)) -> Res<usize> {
    let dir = root.join(".ai-gent").join("delegate");
    let Ok(rd) = std::fs::read_dir(&dir) else {
        return Ok(0);
    };
    let mut names: Vec<String> = rd
        .flatten()
        .filter(|e| e.path().join("status.json").is_file())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    let mut n = 0;
    for name in names {
        let Ok(s) = super::status::read(root, &name) else {
            continue;
        };
        after_read(&name);
        let state = effective_state(root, &s);
        if announced(state) && append(root, &Event::new(&s, state, now_secs()))? {
            n += 1;
        }
    }
    Ok(n)
}

/// One line of `acks.jsonl`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ack {
    /// The acknowledged event id.
    pub id: String,
    /// Epoch seconds.
    pub ts: u64,
}

/// The acknowledged ids.
#[must_use]
pub fn acked(root: &Path) -> HashSet<String> {
    parse_lines::<Ack>(&acks_path(root))
        .into_iter()
        .map(|a| a.id)
        .collect()
}

/// The events not yet acknowledged, oldest first.
#[must_use]
pub fn pending(root: &Path) -> Vec<Event> {
    let done = acked(root);
    read_all(root)
        .into_iter()
        .filter(|e| !done.contains(&e.id))
        .collect()
}

/// Acknowledge `ids` (all pending events when `ids` is empty and `all` is set). Returns the ids newly
/// acknowledged.
///
/// # Errors
/// [`Error::Invalid`] for an id that is not in the log; I/O errors.
pub fn ack(root: &Path, ids: &[String], all: bool) -> Res<Vec<String>> {
    let path = acks_path(root);
    let _guard = lock(&path)?;
    let events = read_all(root);
    let done = acked(root);
    let wanted: Vec<String> = if all {
        events.iter().map(|e| e.id.clone()).collect()
    } else {
        for id in ids {
            if !events.iter().any(|e| &e.id == id) {
                return Err(Error::Invalid(format!("no event with id '{id}'")));
            }
        }
        ids.to_vec()
    };
    let mut out: Vec<String> = Vec::new();
    for id in wanted {
        if done.contains(&id) || out.contains(&id) {
            continue;
        }
        let line = serde_json::to_string(&Ack {
            id: id.clone(),
            ts: now_secs(),
        })
        .map_err(|e| Error::Invalid(e.to_string()))?;
        append_line(&path, &line)?;
        out.push(id);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::delegate::status::{self, Verdict};

    fn status(track: &str, started: u64, state: State) -> Status {
        let mut s = Status::new(
            track,
            "codex",
            Role::Builder,
            &["T001".to_string()],
            ".worktrees/x",
            state,
        );
        s.started = started;
        s.updated = started;
        s
    }

    #[test]
    fn id_names_the_run_and_the_transition() {
        let s = status("feat-a", 100, State::TakenBack);
        assert_eq!(
            Event::new(&s, s.state, 5).id,
            "feat-a:100:taken-back".to_string()
        );
    }

    #[test]
    fn result_is_the_reason_else_the_result() {
        let mut s = status("a", 1, State::Done);
        assert_eq!(Event::new(&s, s.state, 1).result, None);
        s.verdict = Verdict {
            result: VerdictResult::Rejected,
            reason: Some("path-outside".into()),
        };
        assert_eq!(
            Event::new(&s, s.state, 1).result.as_deref(),
            Some("path-outside")
        );
    }

    #[test]
    fn a_duplicate_is_refused() {
        let d = tempfile::tempdir().unwrap();
        let ev = Event::new(&status("a", 1, State::Done), State::Done, 1);
        assert!(append(d.path(), &ev).unwrap());
        assert!(!append(d.path(), &ev).unwrap());
        assert_eq!(read_all(d.path()).len(), 1);
    }

    #[test]
    fn status_write_records_one_event_per_transition() {
        let d = tempfile::tempdir().unwrap();
        status::write(d.path(), &status("a", 1, State::Running)).unwrap();
        assert!(read_all(d.path()).is_empty());
        let done = status("a", 1, State::Done);
        status::write(d.path(), &done).unwrap();
        status::write(d.path(), &done).unwrap();
        assert_eq!(sync(d.path()).unwrap(), 0);
        assert_eq!(read_all(d.path()).len(), 1);
        // A later run of the same track is another id.
        status::write(d.path(), &status("a", 9, State::Done)).unwrap();
        let ids: Vec<_> = read_all(d.path()).into_iter().map(|e| e.id).collect();
        assert_eq!(ids, ["a:1:done", "a:9:done"]);
    }

    #[test]
    fn concurrent_appends_do_not_interleave() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path().to_path_buf();
        let hs: Vec<_> = (0..4)
            .map(|t| {
                let root = root.clone();
                std::thread::spawn(move || {
                    for i in 0..25 {
                        let s = status(&format!("t{t}"), i, State::Done);
                        assert!(append(&root, &Event::new(&s, State::Done, 1)).unwrap());
                    }
                })
            })
            .collect();
        for h in hs {
            h.join().unwrap();
        }
        let text = std::fs::read_to_string(events_path(&root)).unwrap();
        assert_eq!(text.lines().count(), 100);
        assert!(
            text.lines()
                .all(|l| serde_json::from_str::<Event>(l).is_ok())
        );
    }

    #[test]
    fn dedupe_covers_the_whole_log() {
        let d = tempfile::tempdir().unwrap();
        for i in 0..600 {
            status::write(d.path(), &status(&format!("t{i:03}"), 1, State::Done)).unwrap();
        }
        assert_eq!(read_all(d.path()).len(), 600);
        assert_eq!(sync(d.path()).unwrap(), 0);
        assert_eq!(sync(d.path()).unwrap(), 0);
        assert_eq!(read_all(d.path()).len(), 600);
    }

    #[test]
    fn an_event_comes_from_one_snapshot() {
        let d = tempfile::tempdir().unwrap();
        // Written straight to disk so no event is recorded yet.
        let first = status("a", 1, State::Done);
        let dir = d.path().join(".ai-gent/delegate/a");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("status.json"),
            serde_json::to_string(&first).unwrap(),
        )
        .unwrap();
        let root = d.path().to_path_buf();
        // A retry replaces the status right after sync read it.
        let n = sync_with(&root, |_| {
            let mut retry = status("a", 9, State::Running);
            retry.tasks = vec!["T099".into()];
            std::fs::write(
                root.join(".ai-gent/delegate/a/status.json"),
                serde_json::to_string(&retry).unwrap(),
            )
            .unwrap();
        })
        .unwrap();
        assert_eq!(n, 1);
        let all = read_all(d.path());
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, "a:1:done");
        assert_eq!(all[0].state, State::Done);
        assert_eq!(all[0].tasks, ["T001"]);
    }

    #[test]
    fn two_threads_append_one_line() {
        let d = tempfile::tempdir().unwrap();
        let ev = Event::new(&status("a", 1, State::Done), State::Done, 1);
        let hs: Vec<_> = (0..8)
            .map(|_| {
                let (root, ev) = (d.path().to_path_buf(), ev.clone());
                std::thread::spawn(move || append(&root, &ev).unwrap())
            })
            .collect();
        let results: Vec<bool> = hs.into_iter().map(|h| h.join().unwrap()).collect();
        assert_eq!(results.iter().filter(|w| **w).count(), 1);
        let text = std::fs::read_to_string(events_path(d.path())).unwrap();
        assert_eq!(text.lines().count(), 1);
    }

    #[test]
    fn ack_and_pending() {
        let d = tempfile::tempdir().unwrap();
        for n in ["a", "b"] {
            status::write(d.path(), &status(n, 1, State::Failed)).unwrap();
        }
        assert_eq!(pending(d.path()).len(), 2);
        assert_eq!(
            ack(d.path(), &["a:1:failed".into()], false).unwrap().len(),
            1
        );
        assert!(
            ack(d.path(), &["a:1:failed".into()], false)
                .unwrap()
                .is_empty()
        );
        assert!(ack(d.path(), &["nope".into()], false).is_err());
        let p = pending(d.path());
        assert_eq!(p.len(), 1);
        assert_eq!(p[0].track, "b");
        assert_eq!(ack(d.path(), &[], true).unwrap(), ["b:1:failed"]);
        assert!(pending(d.path()).is_empty());
    }
}
