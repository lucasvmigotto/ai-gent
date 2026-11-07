//! The Codex adapter (verified in trials 1 and 2, `contracts/adapter.md`).
//!
//! `codex exec --cd <worktree> -s workspace-write --add-dir <TMPDIR> -o <last> --json "<brief>"` with
//! stdin closed. Only a builder gets `workspace-write` and the `--add-dir`; a reviewer or researcher runs
//! with `-s read-only` and no `--add-dir`, so it cannot write the worktree (the adapter contract). The
//! sandbox keeps `.git` read-only whatever `--add-dir` says (trial 2), so the delegate
//! edits the working tree and stops; the supervisor commits (`commit::commit_changes`). Progress comes
//! from `item.started` / `item.completed`, usage from `turn.completed.usage`.
use super::adapter::{
    Adapter, Check, Detect, Launch, Preflight, RunHandle, RunRequest, RunResult, StopOutcome,
    TrackStatus, launch, stop_track, track_result, track_status,
};
use super::state::{self, files};
use super::status::Role;
use super::{Error, Res, gitx, slash};
use crate::env::Env;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// The Codex adapter.
#[derive(Debug, Clone)]
pub struct CodexAdapter {
    /// The `codex` program.
    pub program: PathBuf,
    /// The repository root (where `.ai-gent/` lives).
    pub root: PathBuf,
    /// The `ai-gent` binary that runs the watcher.
    pub watch_exe: PathBuf,
    /// `TMPDIR` for the delegate.
    pub tmpdir: PathBuf,
    /// Also pass the common git dir with `--add-dir`. Off: trial 2 showed it does not make `.git`
    /// writable, and it would widen what the delegate may write.
    pub add_git_dir: bool,
}

impl CodexAdapter {
    /// An adapter for the repository at `root`: `codex` from `AI_GENT_CODEX_BIN` or `PATH`, this
    /// binary as the watcher, `TMPDIR` or the system temp directory.
    #[must_use]
    pub fn new(root: &Path, env: &Env) -> Self {
        let program = env
            .path("AI_GENT_CODEX_BIN")
            .or_else(|| crate::exec::which("codex", env))
            .unwrap_or_else(|| PathBuf::from("codex"));
        Self {
            program,
            root: root.to_path_buf(),
            watch_exe: std::env::current_exe().unwrap_or_else(|_| PathBuf::from("ai-gent")),
            tmpdir: env.path("TMPDIR").unwrap_or_else(std::env::temp_dir),
            add_git_dir: false,
        }
    }

    fn probe(&self, args: &[&str], secs: u64) -> Option<crate::exec::Output> {
        let mut c = Command::new(&self.program);
        c.args(args);
        crate::exec::run(c, &Env::real(), None, Duration::from_secs(secs)).ok()
    }

    /// The arguments of `codex exec` for a track (the brief is the single `{brief}` placeholder, which
    /// the watcher fills from `brief.txt`).
    ///
    /// # Errors
    /// When `add_git_dir` is set and git cannot name the common dir.
    pub fn exec_args(&self, worktree: &Path, last: &Path, role: Role) -> Res<Vec<String>> {
        let mut a: Vec<String> = ["exec", "--cd"].map(String::from).to_vec();
        a.push(slash(worktree));
        if matches!(role, Role::Builder) {
            a.extend(["-s", "workspace-write"].map(String::from));
            if self.add_git_dir {
                a.push("--add-dir".into());
                a.push(slash(&gitx::common_dir(worktree)?));
            }
            a.push("--add-dir".into());
            a.push(slash(&self.tmpdir));
        } else {
            a.extend(["-s", "read-only"].map(String::from));
        }
        a.push("-o".into());
        a.push(slash(last));
        a.push("--json".into());
        a.push("{brief}".into());
        Ok(a)
    }
}

impl Adapter for CodexAdapter {
    fn id(&self) -> &'static str {
        "codex"
    }

    fn detect(&self) -> Detect {
        let found = if self.program.components().count() > 1 {
            self.program.is_file().then(|| self.program.clone())
        } else {
            crate::exec::which(&self.program.to_string_lossy(), &Env::real())
        };
        let version = found
            .as_ref()
            .and_then(|_| self.probe(&["--version"], 10))
            .filter(|o| o.code == 0)
            .map(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string()
            });
        Detect {
            found: version.is_some(),
            path: found.map(|p| slash(&p)),
            version,
        }
    }

    fn ready(&self) -> Res<()> {
        match self.probe(&["login", "status"], 15) {
            Some(o) if o.code == 0 => Ok(()),
            Some(_) => Err(Error::NotLoggedIn("codex".into())),
            None => Err(Error::Invalid("codex could not be started".into())),
        }
    }

    fn preflight(&self, worktree: &Path) -> Preflight {
        let mut checks = Vec::new();
        let mut add =
            |name: &'static str, ok: bool, detail: String| checks.push(Check { name, ok, detail });
        let git_here = worktree.join(".git").exists();
        add(
            "worktree",
            worktree.is_dir() && git_here,
            if git_here {
                "a git worktree".into()
            } else {
                "not a git worktree".into()
            },
        );
        let writable = |dir: &Path| -> bool {
            let probe = dir.join(format!(".ai-gent-preflight-{}", std::process::id()));
            let ok = std::fs::write(&probe, b"").is_ok();
            let _ = std::fs::remove_file(&probe);
            ok
        };
        add(
            "worktree_writable",
            writable(worktree),
            "a probe file could be created and removed".into(),
        );
        let state_dir = self.root.join(".ai-gent").join("delegate");
        let state_ok = std::fs::create_dir_all(&state_dir).is_ok() && writable(&state_dir);
        add("state_dir_writable", state_ok, slash(&state_dir));
        add(
            "tmpdir_writable",
            self.tmpdir.is_dir() && writable(&self.tmpdir),
            slash(&self.tmpdir),
        );
        // Same spawn path as `run`: stdin is /dev/null, so an agent that waits on stdin hangs here.
        let started = self.probe(&["--version"], 10);
        add(
            "agent_starts_with_stdin_closed",
            started.as_ref().is_some_and(|o| o.code == 0),
            started.map_or_else(
                || "did not start".to_string(),
                |o| {
                    if o.timed_out {
                        "timed out waiting (stdin?)".into()
                    } else {
                        format!("exit {}", o.code)
                    }
                },
            ),
        );
        Preflight {
            ok: checks.iter().all(|c| c.ok),
            checks,
            notes: vec![
                "the delegate does not commit: the sandbox keeps .git read-only, the supervisor stages by explicit path and commits".into(),
                "network: none inside the Codex sandbox (not probed); verification runs in the supervisor".into(),
            ],
        }
    }

    fn run(&self, req: &RunRequest) -> Res<RunHandle> {
        let dir = state::track_dir(&self.root, &req.track)?;
        if req.brief.trim().is_empty() {
            return Err(Error::Invalid("the brief is empty".into()));
        }
        if !req.worktree.is_dir() {
            return Err(Error::Invalid(format!(
                "worktree {} does not exist",
                req.worktree.display()
            )));
        }
        let l = Launch {
            program: slash(&self.program),
            args: self.exec_args(&req.worktree, &dir.join(files::LAST), req.role)?,
            cwd: slash(&req.worktree),
            env: vec![("TMPDIR".into(), slash(&self.tmpdir))],
            stall_secs: req.stall_secs,
            env_remove: Vec::new(),
            guard_root: None,
        };
        launch(&self.root, &self.watch_exe, req, "codex", &l)
    }

    fn status(&self, track: &str) -> Res<TrackStatus> {
        track_status(&self.root, track)
    }

    fn stop(&self, track: &str) -> Res<StopOutcome> {
        stop_track(&self.root, track)
    }

    fn result(&self, track: &str) -> Res<RunResult> {
        track_result(&self.root, track)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exec_args_match_the_contract() {
        let a = CodexAdapter {
            program: "codex".into(),
            root: "/r".into(),
            watch_exe: "ai-gent".into(),
            tmpdir: "/home/u/tmp".into(),
            add_git_dir: false,
        };
        let args = a
            .exec_args(
                Path::new("/r/.worktrees/x"),
                Path::new("/r/.ai-gent/delegate/x/last.txt"),
                Role::Builder,
            )
            .unwrap();
        assert_eq!(
            args,
            [
                "exec",
                "--cd",
                "/r/.worktrees/x",
                "-s",
                "workspace-write",
                "--add-dir",
                "/home/u/tmp",
                "-o",
                "/r/.ai-gent/delegate/x/last.txt",
                "--json",
                "{brief}"
            ]
        );
    }

    #[test]
    fn reviewers_and_researchers_are_read_only() {
        let a = CodexAdapter {
            program: "codex".into(),
            root: "/r".into(),
            watch_exe: "ai-gent".into(),
            tmpdir: "/home/u/tmp".into(),
            add_git_dir: true,
        };
        for role in [Role::Reviewer, Role::Researcher] {
            let args = a
                .exec_args(Path::new("/r/w"), Path::new("/r/last.txt"), role)
                .unwrap();
            assert!(args.contains(&"read-only".to_string()), "{role:?}");
            assert!(!args.contains(&"workspace-write".to_string()), "{role:?}");
            assert!(!args.contains(&"--add-dir".to_string()), "{role:?}");
        }
    }
}
