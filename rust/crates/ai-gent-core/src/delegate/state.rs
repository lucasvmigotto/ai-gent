//! Where a delegation keeps its files: `.ai-gent/delegate/<track>/` and `.ai-gent/delegation.log`,
//! both git-excluded.
use super::{Error, Res};
use std::path::{Path, PathBuf};

/// The track id of a branch: `/` becomes `-` (`contracts/status.md`).
#[must_use]
pub fn track_for_branch(branch: &str) -> String {
    branch.replace('/', "-")
}

/// Whether `track` is safe as a directory name: lowercase letters, digits, `.`, `_` and `-`, not
/// starting with a dot, at most 100 characters.
#[must_use]
pub fn valid_track(track: &str) -> bool {
    !track.is_empty()
        && track.len() <= 100
        && !track.starts_with('.')
        && track
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'))
}

/// The directory of a track under the repository root.
///
/// # Errors
/// When the track id is not valid.
pub fn track_dir(root: &Path, track: &str) -> Res<PathBuf> {
    if !valid_track(track) {
        return Err(Error::Invalid(format!("invalid track id '{track}'")));
    }
    Ok(root.join(".ai-gent").join("delegate").join(track))
}

/// The delegation log.
#[must_use]
pub fn log_path(root: &Path) -> PathBuf {
    root.join(".ai-gent").join("delegation.log")
}

/// File names inside a track directory.
pub mod files {
    /// The status file (`contracts/status.md`).
    pub const STATUS: &str = "status.json";
    /// How the watcher starts the agent.
    pub const LAUNCH: &str = "launch.json";
    /// The agent's events, one JSON object per line.
    pub const EVENTS: &str = "events.jsonl";
    /// The agent's final message.
    pub const LAST: &str = "last.txt";
    /// The agent's process id.
    pub const PID: &str = "agent.pid";
    /// The exit record.
    pub const EXIT: &str = "exit.json";
    /// Present when the supervisor asked the agent to stop.
    pub const STOPPED: &str = "stopped";
    /// Paths the agent changed in the main checkout instead of its worktree (one per line).
    pub const ESCAPED: &str = "escaped.txt";
    /// Present when the main-checkout guard could not take its snapshot before the run.
    pub const GUARD_UNAVAILABLE: &str = "guard-unavailable.txt";
    /// The agent's standard error (local, never read by the tools).
    pub const STDERR: &str = "stderr.log";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracks() {
        assert_eq!(track_for_branch("feat/delegate-core"), "feat-delegate-core");
        assert!(valid_track("feat-delegate-core"));
        assert!(!valid_track("../x"));
        assert!(!valid_track("A"));
        assert!(!valid_track(""));
        assert!(!valid_track(".hidden"));
        assert!(track_dir(Path::new("/r"), "a/b").is_err());
    }
}
