//! The Claude-profile adapter (verified in trial 5, `contracts/adapter.md`): a second Claude Code
//! profile, a config directory passed as `CLAUDE_CONFIG_DIR`, works as a delegate.
//!
//! `claude -p "<brief>" --output-format json --allowedTools <list> --disallowedTools <list> --max-turns N`
//! with stdin closed and the worktree as the working directory. The CLI prints one JSON object when it
//! ends (`result`, `is_error`, `num_turns`, `usage`), so the watcher keeps it as the single event and
//! `result` reads the final message and the usage from it. Nothing inside the config directory is read
//! or copied; the delegate never commits (the supervisor does).
use super::adapter::{
    Adapter, Check, Detect, Launch, Preflight, RunHandle, RunRequest, RunResult, StopOutcome,
    TrackStatus, launch, stop_track, track_result, track_status,
};
use super::state::{self, files};
use super::status::{Role, Usage};
use super::{Error, Res, slash};
use crate::env::Env;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// The one-line probe of `ready`.
const READY_PROMPT: &str = "Reply with exactly: READY";

/// Tools a builder may use: read, search, edit, a narrow set of cargo commands and read-only git.
/// `cargo build`, `check` and `test` still run build scripts and proc macros: subprocesses these
/// rules do not cover (see [`ClaudeAdapter::run_args`]).
const BUILDER_ALLOWED: [&str; 12] = [
    "Read",
    "Grep",
    "Glob",
    "Edit",
    "Write",
    "Bash(cargo test:*)",
    "Bash(cargo clippy:*)",
    "Bash(cargo fmt:*)",
    "Bash(cargo build:*)",
    "Bash(cargo check:*)",
    "Bash(git status:*)",
    "Bash(git diff:*)",
];

/// Git for a builder is read-only (`status`, `diff`, `log`). The CLI lets a deny rule win over an
/// allow rule, so `Bash(git:*)` cannot be denied with `status` allowed: every other subcommand that
/// changes the repository, its config or a remote is listed. The delegate leaves committing and
/// publishing to the supervisor.
const BUILDER_DENIED_GIT: [&str; 24] = [
    "add",
    "am",
    "apply",
    "branch",
    "checkout",
    "cherry-pick",
    "clean",
    "clone",
    "commit",
    "config",
    "fetch",
    "merge",
    "mv",
    "pull",
    "push",
    "rebase",
    "remote",
    "reset",
    "restore",
    "revert",
    "rm",
    "stash",
    "switch",
    "tag",
];

/// What a reviewer or researcher may use: Read, Grep and Glob only. No Bash at all, because even
/// `git diff --output=<file>` writes; the supervisor puts the diff text into the review prompt.
const READONLY_ALLOWED: [&str; 3] = ["Read", "Grep", "Glob"];

/// What a reviewer or researcher may not use: every write tool and Bash.
const READONLY_DENIED: [&str; 4] = ["Edit", "Write", "NotebookEdit", "Bash"];

/// Home-level credential directories of other tools, denied next to every dot-directory named after
/// claude or codex.
const HOME_SECRET_DIRS: [&str; 2] = [".password-store", ".gnupg"];

/// Deny rules for Read, Grep and Glob on a directory tree, in the CLI's absolute-path syntax
/// (`//<abs>/**`).
fn deny_tree(dir: &Path) -> Vec<String> {
    let abs = slash(dir);
    let abs = abs.trim_start_matches('/');
    ["Read", "Grep", "Glob"]
        .iter()
        .map(|t| format!("{t}(//{abs}/**)"))
        .collect()
}

/// The directories no role may read: the profile, and under the home directory every entry whose
/// name starts with `.claude` or `.codex` (only names are listed) and the password store.
fn secret_dirs(config_dir: Option<&Path>, home: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = config_dir.map(Path::to_path_buf).into_iter().collect();
    if let Some(h) = home {
        let mut names: Vec<String> = std::fs::read_dir(h)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|n| n.starts_with(".claude") || n.starts_with(".codex"))
            .collect();
        names.extend(HOME_SECRET_DIRS.iter().map(|n| (*n).to_string()));
        names.sort();
        names.dedup();
        dirs.extend(names.into_iter().map(|n| h.join(n)));
    }
    dirs
}

/// The Claude-profile adapter.
#[derive(Debug, Clone)]
pub struct ClaudeAdapter {
    /// The `claude` program.
    pub program: PathBuf,
    /// The repository root (where `.ai-gent/` lives).
    pub root: PathBuf,
    /// The `ai-gent` binary that runs the watcher.
    pub watch_exe: PathBuf,
    /// `TMPDIR` for the delegate.
    pub tmpdir: PathBuf,
    /// The profile: a config directory passed as `CLAUDE_CONFIG_DIR` (never read or copied). `None`:
    /// the CLI's own default profile, which is the supervisor's, not a second one.
    pub config_dir: Option<PathBuf>,
    /// `--max-turns` of a run.
    pub max_turns: u32,
    /// The home directory whose agent configuration and credential directories are denied to the
    /// worker (only directory names are listed, never contents).
    pub home: Option<PathBuf>,
}

impl ClaudeAdapter {
    /// An adapter for the repository at `root`: `claude` from `AI_GENT_CLAUDE_BIN` or `PATH`, the
    /// profile from `AI_GENT_CLAUDE_CONFIG_DIR`, this binary as the watcher.
    #[must_use]
    pub fn new(root: &Path, env: &Env) -> Self {
        let program = env
            .path("AI_GENT_CLAUDE_BIN")
            .or_else(|| crate::exec::which("claude", env))
            .unwrap_or_else(|| PathBuf::from("claude"));
        Self {
            program,
            root: root.to_path_buf(),
            watch_exe: std::env::current_exe().unwrap_or_else(|_| PathBuf::from("ai-gent")),
            tmpdir: env.path("TMPDIR").unwrap_or_else(std::env::temp_dir),
            config_dir: env.path("AI_GENT_CLAUDE_CONFIG_DIR"),
            max_turns: 40,
            home: env.home_dir(),
        }
    }

    /// The environment of a probe: the real one plus the profile.
    fn probe_env(&self) -> Env {
        let mut e = Env::real();
        if let Some(d) = &self.config_dir {
            e.set("CLAUDE_CONFIG_DIR", d.as_os_str());
        }
        e
    }

    fn probe(&self, args: &[&str], secs: u64) -> Option<crate::exec::Output> {
        let mut c = Command::new(&self.program);
        c.args(args);
        crate::exec::run(c, &self.probe_env(), None, Duration::from_secs(secs)).ok()
    }

    /// The arguments of `claude -p` for a role (the brief is the single `{brief}` placeholder, which the
    /// watcher fills from `brief.txt`). Tool lists are one comma-joined argument each, so the variadic
    /// flags cannot swallow what follows.
    ///
    /// These lists are requests the CLI enforces, not a sandbox: a Claude profile has no OS sandbox,
    /// and subprocesses (build scripts, proc macros) are not covered by them. What compensates lies
    /// outside the worker: the supervisor's diff checks (FR-011), verification run outside the worker,
    /// and the worker never holding push credentials. A reviewer or researcher gets no Bash; the
    /// supervisor puts the diff text into its prompt.
    #[must_use]
    pub fn run_args(&self, role: Role) -> Vec<String> {
        let (allowed, mut denied): (&[&str], Vec<String>) = match role {
            Role::Builder => (
                &BUILDER_ALLOWED,
                BUILDER_DENIED_GIT
                    .iter()
                    .map(|g| format!("Bash(git {g}:*)"))
                    .collect(),
            ),
            Role::Reviewer | Role::Researcher => (
                &READONLY_ALLOWED,
                READONLY_DENIED.iter().map(|d| (*d).to_string()).collect(),
            ),
        };
        for d in secret_dirs(self.config_dir.as_deref(), self.home.as_deref()) {
            denied.extend(deny_tree(&d));
        }
        vec![
            "-p".into(),
            "{brief}".into(),
            "--output-format".into(),
            "json".into(),
            "--allowedTools".into(),
            allowed.join(","),
            "--disallowedTools".into(),
            denied.join(","),
            "--max-turns".into(),
            self.max_turns.to_string(),
        ]
    }
}

/// The shortest stall limit of a Claude run: the CLI prints nothing until it ends, so a shorter silence
/// is not a stall (the request's value, 600 s by default, would report every long run as stalled).
pub const MIN_STALL_SECS: u64 = 3600;

/// The stall limit of a Claude run: the request's, but at least [`MIN_STALL_SECS`].
#[must_use]
pub fn stall_limit(requested: u64) -> u64 {
    requested.max(MIN_STALL_SECS)
}

/// The result object of a run: the last JSON object of `events.jsonl` that has a `result` field.
fn result_event(dir: &Path) -> Option<Value> {
    std::fs::read_to_string(dir.join(files::EVENTS))
        .ok()?
        .lines()
        .rev()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
        .find(|v| v.is_object() && v.get("result").is_some())
}

/// The usage of a Claude result object. Claude's `input_tokens` leaves the cache reads out, so they are
/// added to the input (the `Usage` input includes its cached part).
fn usage_of(v: &Value) -> Option<Usage> {
    let u = v.get("usage")?;
    let n = |k: &str| u[k].as_u64().unwrap_or(0);
    let cached = n("cache_read_input_tokens");
    Some(Usage {
        input_tokens: n("input_tokens") + cached,
        cached_input_tokens: cached,
        output_tokens: n("output_tokens"),
        reasoning_output_tokens: 0,
    })
}

impl Adapter for ClaudeAdapter {
    fn id(&self) -> &'static str {
        "claude"
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
        let o = self
            .probe(&["-p", READY_PROMPT, "--output-format", "json"], 90)
            .ok_or_else(|| Error::Invalid("claude could not be started".into()))?;
        let answered = serde_json::from_slice::<Value>(&o.stdout).is_ok_and(|v| {
            v["is_error"] == false && v["result"].as_str().is_some_and(|r| r.contains("READY"))
        });
        if o.code == 0 && !o.timed_out && answered {
            Ok(())
        } else {
            Err(Error::NotLoggedIn("claude".into()))
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
        // Only whether the directory exists: nothing inside it is read.
        if let Some(d) = &self.config_dir {
            add("profile_dir_exists", d.is_dir(), slash(d));
        }
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
                "the delegate does not commit: mutating git commands are disallowed, the supervisor stages by explicit path and commits".into(),
                "a Claude profile has no OS sandbox: the tool lists are requests the CLI enforces and subprocesses (build scripts) are not covered; the supervisor's diff checks and verification outside the worker compensate".into(),
                "the profile's login is not probed here; `ready` does that with a one-line call".into(),
                "the CLI prints one JSON object when it ends: there is no progress before that".into(),
            ],
        }
    }

    fn run(&self, req: &RunRequest) -> Res<RunHandle> {
        if self.config_dir.is_none() {
            return Err(Error::Invalid(
                "a Claude delegate needs its own profile: set AI_GENT_CLAUDE_CONFIG_DIR".into(),
            ));
        }
        state::track_dir(&self.root, &req.track)?;
        if req.brief.trim().is_empty() {
            return Err(Error::Invalid("the brief is empty".into()));
        }
        if !req.worktree.is_dir() {
            return Err(Error::Invalid(format!(
                "worktree {} does not exist",
                req.worktree.display()
            )));
        }
        let mut env = vec![("TMPDIR".to_string(), slash(&self.tmpdir))];
        if let Some(d) = &self.config_dir {
            env.push(("CLAUDE_CONFIG_DIR".into(), slash(d)));
        }
        let l = Launch {
            program: slash(&self.program),
            args: self.run_args(req.role),
            cwd: slash(&req.worktree),
            env,
            stall_secs: stall_limit(req.stall_secs),
        };
        // The only event comes when the agent ends, so waiting for a first one would stop every run.
        let req = RunRequest {
            first_event_wait: Duration::ZERO,
            ..req.clone()
        };
        launch(&self.root, &self.watch_exe, &req, "claude", &l)
    }

    fn status(&self, track: &str) -> Res<TrackStatus> {
        track_status(&self.root, track)
    }

    fn stop(&self, track: &str) -> Res<StopOutcome> {
        stop_track(&self.root, track)
    }

    fn result(&self, track: &str) -> Res<RunResult> {
        let mut r = track_result(&self.root, track)?;
        if let Some(ev) = result_event(&state::track_dir(&self.root, track)?) {
            if r.message.is_none() {
                r.message = ev["result"]
                    .as_str()
                    .map(str::to_string)
                    .filter(|m| !m.trim().is_empty());
            }
            if r.usage.is_none() {
                r.usage = usage_of(&ev);
            }
        }
        Ok(r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adapter() -> ClaudeAdapter {
        ClaudeAdapter {
            program: "claude".into(),
            root: "/r".into(),
            watch_exe: "ai-gent".into(),
            tmpdir: "/home/u/tmp".into(),
            config_dir: Some("/home/u/.claude-second".into()),
            max_turns: 30,
            home: None,
        }
    }

    #[test]
    fn the_stall_limit_is_at_least_an_hour() {
        assert_eq!(stall_limit(600), 3600);
        assert_eq!(stall_limit(7200), 7200);
    }

    #[test]
    fn a_run_without_a_profile_is_refused() {
        let mut a = adapter();
        a.config_dir = None;
        let req = RunRequest {
            track: "t".into(),
            worktree: "/r".into(),
            brief: "b".into(),
            tasks: vec![],
            role: Role::Builder,
            stall_secs: 600,
            first_event_wait: Duration::ZERO,
        };
        let e = a.run(&req).unwrap_err().to_string();
        assert!(
            e.contains("needs its own profile: set AI_GENT_CLAUDE_CONFIG_DIR"),
            "{e}"
        );
    }

    fn lists(a: &[String]) -> (Vec<String>, Vec<String>) {
        let split = |s: &String| s.split(',').map(str::to_string).collect::<Vec<_>>();
        (split(&a[5]), split(&a[7]))
    }

    #[test]
    fn builder_args_match_the_contract() {
        let a = adapter().run_args(Role::Builder);
        assert_eq!(
            a[..5],
            ["-p", "{brief}", "--output-format", "json", "--allowedTools"]
        );
        assert_eq!(
            a[5],
            "Read,Grep,Glob,Edit,Write,Bash(cargo test:*),Bash(cargo clippy:*),Bash(cargo fmt:*),Bash(cargo build:*),Bash(cargo check:*),Bash(git status:*),Bash(git diff:*)"
        );
        assert_eq!(a[6], "--disallowedTools");
        assert_eq!(a[8..], ["--max-turns", "30"]);
        let (allowed, denied) = lists(&a);
        assert!(!allowed.iter().any(|t| t == "Bash(cargo:*)" || t == "Bash"));
        for g in [
            "commit", "push", "add", "reset", "checkout", "config", "remote",
        ] {
            assert!(denied.contains(&format!("Bash(git {g}:*)")), "{g}");
        }
        for ok in ["status", "diff", "log"] {
            assert!(!denied.contains(&format!("Bash(git {ok}:*)")), "{ok}");
        }
    }

    #[test]
    fn every_role_is_denied_the_profile_and_other_agents_directories() {
        let home = tempfile::tempdir().unwrap();
        for d in [".claude", ".claude-work", ".codex", ".config", "codes"] {
            std::fs::create_dir(home.path().join(d)).unwrap();
        }
        let mut ad = adapter();
        ad.home = Some(home.path().to_path_buf());
        for role in [Role::Builder, Role::Reviewer, Role::Researcher] {
            let (_, denied) = lists(&ad.run_args(role));
            let h = slash(home.path());
            let h = h.trim_start_matches('/');
            for tool in ["Read", "Grep", "Glob"] {
                for dir in [
                    ".claude",
                    ".claude-work",
                    ".codex",
                    ".password-store",
                    ".gnupg",
                ] {
                    let rule = format!("{tool}(//{h}/{dir}/**)");
                    assert!(denied.contains(&rule), "{rule} in {denied:?}");
                }
                assert!(denied.contains(&format!("{tool}(//home/u/.claude-second/**)")));
                assert!(
                    !denied
                        .iter()
                        .any(|d| d.contains("/codes/") || d.contains("/.config/"))
                );
            }
        }
    }

    #[test]
    fn a_reviewer_and_a_researcher_cannot_write_or_run_anything() {
        for role in [Role::Reviewer, Role::Researcher] {
            let a = adapter().run_args(role);
            assert_eq!(a[5], "Read,Grep,Glob");
            let (allowed, denied) = lists(&a);
            assert!(!allowed.iter().any(|t| t.starts_with("Bash")));
            for t in ["Edit", "Write", "NotebookEdit", "Bash"] {
                assert!(denied.iter().any(|d| d == t), "{t}");
            }
        }
    }

    #[test]
    fn usage_adds_the_cache_reads_to_the_input() {
        let v: Value = serde_json::from_str(
            r#"{"type":"result","is_error":false,"result":"x","num_turns":2,
            "usage":{"input_tokens":200,"cache_read_input_tokens":800,"output_tokens":50,"other":1}}"#,
        )
        .unwrap();
        assert_eq!(
            usage_of(&v),
            Some(Usage {
                input_tokens: 1000,
                cached_input_tokens: 800,
                output_tokens: 50,
                reasoning_output_tokens: 0
            })
        );
        assert_eq!(usage_of(&serde_json::json!({"result":"x"})), None);
    }
}
