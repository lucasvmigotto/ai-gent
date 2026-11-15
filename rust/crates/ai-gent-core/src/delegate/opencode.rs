//! The OpenCode adapter (verified in trial 3, `contracts/opencode.md`).
//!
//! `opencode run --standalone --model opencode/<model> --format json --auto "<brief>"` with stdin
//! closed and the worktree as the working directory. `--auto` approves every permission that is not
//! explicitly denied, so the run always carries an inline permission set in `OPENCODE_CONFIG_CONTENT`
//! in which every rule is a `deny` (nothing is left to ask, and nothing is edited in the user's own
//! OpenCode configuration).
//!
//! There is **no OS sandbox**: that permission set and the worktree are the containment, and a
//! permission set is a request OpenCode enforces, not a sandbox (subprocesses of an allowed build are
//! not covered). The compensations are outside the worker (`adapter.md`). Free hosted models may use
//! prompts for training (unverified), so `run` refuses one unless the caller acknowledges the privacy
//! notice, which the supervising skill gives.
//!
//! Events (`--format json`) are `step_start`, `tool_use`, `step_finish` and `text`. Usage and cost are in
//! every `step_finish.part`, so [`OpenCodeAdapter::result`] sums over the steps. The exit code alone
//! proves nothing: a run can exit 1 after it finished, so the final `step_finish` (`reason: stop`) is
//! what counts, and the supervisor still checks the diff.
#![allow(clippy::doc_markdown)] // "OpenCode" is a product name, not an identifier
use super::adapter::{
    Adapter, Check, Detect, Launch, Preflight, RunHandle, RunRequest, RunResult, StopOutcome,
    TrackStatus, launch, stop_track, track_result, track_status,
};
use super::state::{self, files};
use super::status::{self, LogLine, Role, State, Status, Usage};
use super::{Error, Res, now_secs, slash};
use crate::env::Env;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// The default model: the owner's first choice among the free ones.
pub const DEFAULT_MODEL: &str = "opencode/flint-40b-free";

/// The one-line probe of `ready`.
const READY_PROMPT: &str = "Reply with exactly: READY";

/// Where the installer puts the binary, under the home directory (it is not on `PATH`).
const INSTALL_DIR: [&str; 2] = [".opencode", "bin"];

/// Shell commands no role may run: the network, databases, containers and the password store.
const DENIED_TOOLS: [&str; 22] = [
    "curl", "wget", "ssh", "scp", "sftp", "rsync", "nc", "ncat", "netcat", "telnet", "ftp", "gh",
    "glab", "psql", "mysql", "sqlite3", "sqlcmd", "docker", "podman", "pass", "gpg", "gpg2",
];

/// Git subcommands that change the repository, its config or a remote. The delegate leaves the
/// working tree and stops; the supervisor stages and commits.
const DENIED_GIT: [&str; 26] = [
    "add",
    "am",
    "apply",
    "branch -D",
    "branch -d",
    "checkout",
    "cherry-pick",
    "clean",
    "clone",
    "commit",
    "config",
    "fetch",
    "ls-remote",
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

/// Directories under the home directory that hold credentials or agent configuration.
const SECRET_DIRS: [&str; 12] = [
    ".ssh",
    ".gnupg",
    ".password-store",
    ".aws",
    ".azure",
    ".kube",
    ".docker",
    ".config/ai-gent",
    ".config/opencode",
    ".config/gh",
    ".local/share/opencode",
    ".local/share/ai-gent",
];

/// Entries of the home directory whose name starts with one of these are other agents' profiles
/// (only the names are listed, never the contents).
const AGENT_DIR_PREFIXES: [&str; 11] = [
    ".claude",
    ".codex",
    ".opencode",
    ".cursor",
    ".gemini",
    ".agents",
    ".amp",
    ".kiro",
    ".windsurf",
    ".antigravity",
    ".copilot",
];

/// File names that look like credentials, denied for reading and editing anywhere.
const CREDENTIAL_FILES: [&str; 12] = [
    "*/.env",
    "*/.env.*",
    "*credentials*",
    "*id_rsa*",
    "*id_ed25519*",
    "*.pem",
    "*.p12",
    "*.key",
    "*/.netrc",
    "*/connections.env",
    "*/connections.toml",
    "*/.npmrc",
];

/// Whether a file's base name looks like a credential file: the one list the OpenCode deny rules and
/// the review prompt share (`.env*`, `.netrc`, `.npmrc`, `.pypirc`, `.git-credentials`, SSH keys,
/// `*.pem`/`*.key`/`*.p12`/`*.pfx`, `credentials*`, `secrets*`, `connections.env|toml`). Case is ignored.
#[must_use]
pub fn is_credential_name(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.starts_with(".env")
        || [
            "credentials",
            "secrets",
            "id_rsa",
            "id_ed25519",
            "id_ecdsa",
            "id_dsa",
        ]
        .iter()
        .any(|f| n.contains(f))
        || [".pem", ".key", ".p12", ".pfx"]
            .iter()
            .any(|e| n.ends_with(e))
        || matches!(
            n.as_str(),
            ".netrc" | "_netrc" | ".npmrc" | ".pypirc" | "connections.env" | "connections.toml"
        )
}

/// The same names as fragments, for shell commands that read a file (`cat .env`): a pattern
/// `<reader> *<fragment>*`.
const CREDENTIAL_FRAGMENTS: [&str; 10] = [
    ".env",
    "credentials",
    "id_rsa",
    "id_ed25519",
    ".pem",
    ".p12",
    ".key",
    ".netrc",
    "connections.",
    ".npmrc",
];

/// Variables the child must not inherit, because each can name a directory other than its worktree:
/// `INIT_CWD` and `OLDPWD` (the supervisor's directories, which a Node or Bun tool may prefer to the
/// real working directory) and git's own location variables (`GIT_DIR`, `GIT_WORK_TREE`,
/// `GIT_COMMON_DIR`, `GIT_INDEX_FILE`, `GIT_PREFIX`). `PWD` is not removed but set to the worktree.
/// `opencode run` has no directory option (v2.2.5, `opencode run --help`), so the working directory
/// and these variables are all the adapter can set.
pub const ISOLATION_ENV_REMOVE: [&str; 7] = [
    "INIT_CWD",
    "OLDPWD",
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_COMMON_DIR",
    "GIT_INDEX_FILE",
    "GIT_PREFIX",
];

/// Commands that print, copy or encode a file's content.
const FILE_READERS: [&str; 17] = [
    "cat", "less", "more", "head", "tail", "tac", "cp", "base64", "xxd", "strings", "od",
    "hexdump", "grep", "sed", "awk", "tar", "zip",
];

/// The forms a command name takes in a shell line: first word, a path to it, or after other words
/// (`env VAR=x git ...`, `sudo git ...`).
fn command_forms(cmd: &str) -> [String; 3] {
    [cmd.to_string(), format!("*/{cmd}"), format!("* {cmd}")]
}

/// The model as passed to `--model`: `flint-40b-free` becomes `opencode/flint-40b-free`.
#[must_use]
pub fn qualify(model: &str) -> String {
    if model.contains('/') {
        model.to_string()
    } else {
        format!("opencode/{model}")
    }
}

/// Whether the model is a free hosted one (its name ends in `free`), which may use prompts and
/// completions to improve the provider's models (unverified).
///
/// A `#variant` suffix (`provider/model#variant`, `contracts/opencode.md`) is dropped first, so
/// `flint-40b-free#fast` is still free. A `:tag` form (`model:free`, as some gateways write it) is not
/// in the contract; it is treated as free when any `:`-separated part ends in `free`, the cautious
/// reading. Case is ignored.
#[must_use]
pub fn is_free_model(model: &str) -> bool {
    let base = model.split('#').next().unwrap_or(model);
    base.rsplit('/')
        .next()
        .is_some_and(|m| m.split(':').any(|p| p.to_lowercase().ends_with("free")))
}

fn rule(action: &str, resource: &str) -> Value {
    json!({ "action": action, "resource": resource, "effect": "deny" })
}

/// The per-run permission set, an ordered `permissions` array of `{action, resource, effect}` rules in
/// which every effect is `deny`: `--auto` approves whatever is not denied, so a rule that would ask
/// must be a deny, and a set of denies only has no order to get wrong.
///
/// Every role is denied git writes (push, commit, reset, `branch -D`, ...), network and database
/// clients, containers, `pass`/gpg, web search and fetch, and reading or editing credential and
/// agent-configuration directories. A reviewer or researcher is also denied every edit and every shell
/// command. These are patterns OpenCode matches, not a sandbox.
#[must_use]
pub fn permission_rules(role: Role, home: Option<&Path>) -> Vec<Value> {
    let mut rules = Vec::new();
    if !matches!(role, Role::Builder) {
        rules.push(rule("edit", "*"));
        rules.push(rule("shell", "*"));
    }
    // Git: `git push*`, `/usr/bin/git push*`, `env VAR=x git push*`.
    let git_args = DENIED_GIT
        .iter()
        .map(|g| format!("{g}*"))
        .chain(["-C", "-c", "--git-dir", "--work-tree"].map(|g| format!("{g}*")))
        .chain(["commit*--no-verify*".to_string()]);
    for a in git_args {
        for f in command_forms("git") {
            rules.push(rule("shell", &format!("{f} {a}")));
        }
    }
    for t in DENIED_TOOLS {
        for p in [format!("{t}*"), format!("*/{t} *"), format!("* {t} *")] {
            rules.push(rule("shell", &p));
        }
    }
    // Reading a credential-named file through a shell reader. A list cannot cover an interpreter or
    // a program of the worker's own; the file rules below cover OpenCode's own tools.
    for c in FILE_READERS {
        for f in command_forms(c) {
            for n in CREDENTIAL_FRAGMENTS {
                rules.push(rule("shell", &format!("{f} *{n}*")));
            }
        }
    }
    for a in ["webfetch", "websearch"] {
        rules.push(rule(a, "*"));
    }
    let mut dirs: Vec<String> = SECRET_DIRS.iter().map(|d| (*d).to_string()).collect();
    if let Some(h) = home {
        let mut names: Vec<String> = std::fs::read_dir(h)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|n| AGENT_DIR_PREFIXES.iter().any(|p| n.starts_with(p)))
            .collect();
        names.sort();
        dirs.extend(names);
    }
    dirs.sort();
    dirs.dedup();
    for d in &dirs {
        let mut roots = vec![format!("~/{d}")];
        if let Some(h) = home {
            roots.push(format!("{}/{d}", slash(h)));
        }
        for r in roots {
            // The entry itself too: a home entry can be a file (`.claude.json`), not a directory.
            for a in ["read", "edit"] {
                rules.push(rule(a, &r));
                rules.push(rule(a, &format!("{r}/*")));
                rules.push(rule(a, &format!("{r}/**")));
            }
        }
        rules.push(rule("shell", &format!("*{d}*")));
    }
    for f in CREDENTIAL_FILES {
        for a in ["read", "edit"] {
            rules.push(rule(a, f));
        }
    }
    rules
}

/// The text of `OPENCODE_CONFIG_CONTENT` for a role.
#[must_use]
pub fn config_content(role: Role, home: Option<&Path>) -> String {
    json!({
        "$schema": "https://opencode.ai/config.json",
        "permissions": permission_rules(role, home),
    })
    .to_string()
}

/// What the events of a run add up to.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Summary {
    /// Usage summed over every `step_finish`; `None` when there was none.
    pub usage: Option<Usage>,
    /// Cost summed over every `step_finish` (0 for free models).
    pub cost: f64,
    /// The final `step_finish` has `reason: stop`.
    pub finished: bool,
    /// The text of the last step (`text` events after the last `step_start`).
    pub text: Option<String>,
}

/// Read the events of a run (one JSON object per line; other lines are ignored).
#[must_use]
pub fn summarize(events: &str) -> Summary {
    let mut s = Summary::default();
    let mut total = Usage::default();
    let mut steps = 0_u32;
    let mut text = String::new();
    for v in events
        .lines()
        .filter_map(|l| serde_json::from_str::<Value>(l).ok())
    {
        match v["type"].as_str() {
            Some("step_start") => text.clear(),
            Some("text") => {
                if let Some(t) = v["part"]["text"].as_str() {
                    text.push_str(t);
                }
            }
            Some("step_finish") => {
                steps += 1;
                let p = &v["part"];
                let t = &p["tokens"];
                let n = |x: &Value| x.as_u64().unwrap_or(0);
                let (read, write) = (n(&t["cache"]["read"]), n(&t["cache"]["write"]));
                total.add(&Usage {
                    input_tokens: n(&t["input"]) + read + write,
                    cached_input_tokens: read,
                    output_tokens: n(&t["output"]),
                    reasoning_output_tokens: n(&t["reasoning"]),
                });
                s.cost += p["cost"].as_f64().unwrap_or(0.0);
                s.finished = p["reason"] == "stop";
            }
            _ => {}
        }
    }
    s.usage = (steps > 0).then_some(total);
    s.text = Some(text.trim().to_string()).filter(|t| !t.is_empty());
    s
}

/// The OpenCode adapter.
#[derive(Debug, Clone)]
pub struct OpenCodeAdapter {
    /// The `opencode` program.
    pub program: PathBuf,
    /// The repository root (where `.ai-gent/` lives).
    pub root: PathBuf,
    /// The `ai-gent` binary that runs the watcher.
    pub watch_exe: PathBuf,
    /// `TMPDIR` for the delegate.
    pub tmpdir: PathBuf,
    /// The model, `provider/model`.
    pub model: String,
    /// The caller acknowledged the privacy notice for free hosted models (`--privacy-ack`).
    pub privacy_ack: bool,
    /// The home directory: where the installer puts the binary, and whose credential and agent
    /// directories are denied to the worker (only names are listed).
    pub home: Option<PathBuf>,
}

impl OpenCodeAdapter {
    /// An adapter for the repository at `root`: `opencode` from `AI_GENT_OPENCODE_BIN`, else `PATH`,
    /// else `~/.opencode/bin/opencode`; this binary as the watcher. `PATH` is never assumed to hold it.
    #[must_use]
    pub fn new(root: &Path, env: &Env) -> Self {
        let home = env.home_dir();
        let program = env
            .path("AI_GENT_OPENCODE_BIN")
            .map(|p| absolutize(p, std::env::current_dir().ok().as_deref()))
            .or_else(|| crate::exec::which("opencode", env))
            .or_else(|| install_path(home.as_deref()))
            .unwrap_or_else(|| PathBuf::from("opencode"));
        Self {
            program,
            root: root.to_path_buf(),
            watch_exe: std::env::current_exe().unwrap_or_else(|_| PathBuf::from("ai-gent")),
            tmpdir: env.path("TMPDIR").unwrap_or_else(std::env::temp_dir),
            model: DEFAULT_MODEL.to_string(),
            privacy_ack: false,
            home,
        }
    }

    /// Use another model (`flint-40b-free` and `opencode/flint-40b-free` both work).
    #[must_use]
    pub fn with_model(mut self, model: &str) -> Self {
        self.model = qualify(model);
        self
    }

    /// Record the caller's acknowledgement of the privacy notice.
    #[must_use]
    pub fn with_privacy_ack(mut self, ack: bool) -> Self {
        self.privacy_ack = ack;
        self
    }

    fn found(&self) -> Option<PathBuf> {
        if self.program.components().count() > 1 {
            self.program.is_file().then(|| self.program.clone())
        } else {
            crate::exec::which(&self.program.to_string_lossy(), &Env::real()).or_else(|| {
                (self.program == Path::new("opencode"))
                    .then(|| install_path(self.home.as_deref()))
                    .flatten()
            })
        }
    }

    /// The environment of a probe or run: the permission set, inline (the user's own configuration is
    /// never edited).
    fn probe_env(&self, role: Role) -> Env {
        let mut e = Env::real();
        e.set(
            "OPENCODE_CONFIG_CONTENT",
            config_content(role, self.home.as_deref()),
        );
        e
    }

    fn probe(&self, args: &[&str], role: Role, secs: u64) -> Option<crate::exec::Output> {
        let mut c = Command::new(self.found().unwrap_or_else(|| self.program.clone()));
        c.args(args).current_dir(&self.tmpdir);
        crate::exec::run(c, &self.probe_env(role), None, Duration::from_secs(secs)).ok()
    }

    /// The arguments of `opencode run` (the brief is the single `{brief}` placeholder, which the
    /// watcher fills from `brief.txt`).
    #[must_use]
    pub fn run_args(&self) -> Vec<String> {
        [
            "run",
            "--standalone",
            "--model",
            &self.model,
            "--format",
            "json",
            "--auto",
            "{brief}",
        ]
        .map(String::from)
        .to_vec()
    }

    /// The refusal of a free hosted model without the privacy acknowledgement.
    fn privacy_gate(&self) -> Res<()> {
        if is_free_model(&self.model) && !self.privacy_ack {
            return Err(Error::Refused(format!(
                "{} is a free hosted model, and free hosted models may use prompts and completions to improve their provider's models (unchecked); \
                 refused without --privacy-ack. The supervising skill gives the notice first: public content only, never private repositories, \
                 credentials, customer data or .ai-gent/",
                self.model
            )));
        }
        Ok(())
    }

    fn summary(&self, track: &str) -> Res<Summary> {
        let dir = state::track_dir(&self.root, track)?;
        Ok(summarize(
            &std::fs::read_to_string(dir.join(files::EVENTS)).unwrap_or_default(),
        ))
    }

    /// The cost of a run, summed over its steps (0 for free models).
    ///
    /// # Errors
    /// When the track id is invalid.
    pub fn cost(&self, track: &str) -> Res<f64> {
        Ok(self.summary(track)?.cost)
    }
}

/// A relative path with a directory part, made absolute against `cwd` (the supervisor's), because the
/// watcher starts the agent from the worktree. A bare name stays: it is looked up on `PATH`.
fn absolutize(p: PathBuf, cwd: Option<&Path>) -> PathBuf {
    match cwd {
        Some(c) if p.is_relative() && p.components().count() > 1 => c.join(p),
        _ => p,
    }
}

/// `~/.opencode/bin/opencode`, when it is a file.
fn install_path(home: Option<&Path>) -> Option<PathBuf> {
    let mut p = home?.to_path_buf();
    p.extend(INSTALL_DIR);
    p.push(format!("opencode{}", std::env::consts::EXE_SUFFIX));
    p.is_file().then_some(p)
}

impl Adapter for OpenCodeAdapter {
    fn id(&self) -> &'static str {
        "opencode"
    }

    fn detect(&self) -> Detect {
        let found = self.found();
        let version = found
            .as_ref()
            .and_then(|_| self.probe(&["--version"], Role::Reviewer, 10))
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

    /// The probe goes through the run shape (`--standalone`, `--auto`, the read-only permission set,
    /// stdin closed). Its content is fixed public text, so it needs no privacy acknowledgement.
    fn ready(&self) -> Res<()> {
        let args = [
            "run",
            "--standalone",
            "--model",
            &self.model,
            "--format",
            "json",
            "--auto",
            READY_PROMPT,
        ];
        let o = self.probe(&args, Role::Reviewer, 90).ok_or_else(|| {
            Error::Invalid("opencode is not available: it could not be started".into())
        })?;
        // The exit code alone proves nothing (a finished run can exit 1): the events decide.
        let events = String::from_utf8_lossy(&o.stdout);
        let s = summarize(&events);
        if !o.timed_out && s.finished && s.text.is_some_and(|t| t.contains("READY")) {
            Ok(())
        } else {
            Err(Error::NotLoggedIn("opencode".into()))
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
        add(
            "privacy_acknowledged",
            !is_free_model(&self.model) || self.privacy_ack,
            if is_free_model(&self.model) {
                format!(
                    "{} is a free hosted model: --privacy-ack needed",
                    self.model
                )
            } else {
                format!("{} is not a free model", self.model)
            },
        );
        // Same spawn path as `run`: stdin is /dev/null, so an agent that waits on stdin hangs here.
        let started = self.probe(&["--version"], Role::Reviewer, 10);
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
                "the delegate does not commit: git writes are denied in the permission set, the supervisor stages by explicit path and commits".into(),
                "OpenCode has no OS sandbox: the per-run permission set (every rule a deny, passed inline in OPENCODE_CONFIG_CONTENT) and the worktree are the containment, and subprocesses are not covered; the supervisor's diff checks and verification outside the worker compensate".into(),
                "the worker inherits the supervisor's environment: the supervisor must not hold credentials there".into(),
                "the login is not probed here; `ready` does that with a one-line call".into(),
            ],
        }
    }

    fn run(&self, req: &RunRequest) -> Res<RunHandle> {
        self.privacy_gate()?;
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
        let l = Launch {
            program: slash(&self.found().unwrap_or_else(|| self.program.clone())),
            args: self.run_args(),
            cwd: slash(&req.worktree),
            env: vec![
                ("TMPDIR".into(), slash(&self.tmpdir)),
                (
                    "OPENCODE_CONFIG_CONTENT".into(),
                    config_content(req.role, self.home.as_deref()),
                ),
            ],
            stall_secs: req.stall_secs,
            env_remove: ISOLATION_ENV_REMOVE.iter().map(|s| (*s).into()).collect(),
            guard_root: super::gitx::repo_root(&req.worktree)
                .ok()
                .map(|r| slash(&r)),
        };
        // The status records the model (`opencode:<model>`): the review rule compares models (FR-010).
        let agent = format!("opencode:{}", qualify(&self.model));
        launch(&self.root, &self.watch_exe, req, &agent, &l)
    }

    /// `result` runs first, so a finished run's persisted state is the corrected one here too.
    fn status(&self, track: &str) -> Res<TrackStatus> {
        let _ = self.result(track);
        track_status(&self.root, track)
    }

    fn stop(&self, track: &str) -> Res<StopOutcome> {
        stop_track(&self.root, track)
    }

    /// Judged by the final `step_finish` (`reason: stop`), not the exit code alone: exit 1 after a
    /// finished run is `done` (with a `reason` saying so), and exit 0 without a final stop is `failed`.
    /// The watcher cannot read OpenCode's events, so when this verdict differs from the persisted state it
    /// is written back (`status.json`, the track's delegation log line, and the usage the watcher left
    /// empty); `status` and `log` then agree with `result`. Whether the diff is right is the
    /// supervisor's check.
    fn result(&self, track: &str) -> Res<RunResult> {
        let mut r = track_result(&self.root, track)?;
        let s = self.summary(track)?;
        if r.message.is_none() {
            r.message.clone_from(&s.text);
        }
        if r.usage.is_none() {
            r.usage = s.usage;
        }
        let escaped = r
            .reason
            .as_deref()
            .is_some_and(|x| x.starts_with("wrote-outside-worktree"));
        if r.finished && r.state != State::TakenBack && !escaped {
            match r.exit_code {
                Some(0) if !s.finished => {
                    r.state = State::Failed;
                    r.is_error = true;
                    r.reason = Some("no-final-step-finish".into());
                }
                Some(c) if c != 0 && s.finished => {
                    r.state = State::Done;
                    r.is_error = false;
                    r.reason = Some(format!("exit-code-{c}-but-finished"));
                }
                _ => {}
            }
            self.reconcile(track, &r);
        }
        Ok(r)
    }
}

impl OpenCodeAdapter {
    /// Write the verdict of `result` back to `status.json` and the track's delegation log line when
    /// they differ (best effort: a failed write leaves the files as the watcher wrote them).
    fn reconcile(&self, track: &str, r: &RunResult) {
        let Ok(mut st) = status::read(&self.root, track) else {
            return;
        };
        if st.state == r.state && (st.tokens.is_some() || r.usage.is_none()) {
            return;
        }
        st.state = r.state;
        st.tokens = r.usage.or(st.tokens);
        st.updated = now_secs().max(st.started);
        if status::write(&self.root, &st).is_ok() {
            let _ = correct_log(&self.root, &st);
        }
    }
}

/// Replace the state and usage of the track's latest line in the delegation log (the watcher wrote it
/// before `result` could judge). A line appended by another run between the read and the rename would
/// be lost; the window is a few milliseconds.
fn correct_log(root: &Path, st: &Status) -> Res<()> {
    let path = state::log_path(root);
    let text = std::fs::read_to_string(&path).map_err(|e| Error::Invalid(e.to_string()))?;
    let mut lines: Vec<String> = text.lines().map(String::from).collect();
    let parse = |l: &String| serde_json::from_str::<LogLine>(l).ok();
    let Some(i) = lines
        .iter()
        .rposition(|l| parse(l).is_some_and(|x| x.track == st.track))
    else {
        return Ok(());
    };
    if let Some(mut line) = parse(&lines[i]) {
        line.result = st.state;
        line.tokens = st.tokens;
        lines[i] = serde_json::to_string(&line).map_err(|e| Error::Invalid(e.to_string()))?;
    }
    let mut out = lines.join("\n");
    out.push('\n');
    status::atomic_write(&path, out.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_denied_credential_pattern_is_a_credential_name() {
        for pat in CREDENTIAL_FILES {
            let name = pat.trim_start_matches("*/").replace('*', "");
            let name = if name.ends_with('.') {
                format!("{name}x")
            } else {
                name
            };
            assert!(is_credential_name(&name), "{pat} -> {name}");
        }
        for n in [
            ".netrc",
            ".npmrc",
            ".pypirc",
            ".git-credentials",
            "id_ecdsa",
            "id_dsa",
            "a.pfx",
            "secrets.yml",
            "Credentials.json",
        ] {
            assert!(is_credential_name(n), "{n}");
        }
        assert!(!is_credential_name("main.rs") && !is_credential_name("environment.md"));
    }

    fn adapter() -> OpenCodeAdapter {
        OpenCodeAdapter {
            program: "opencode".into(),
            root: "/r".into(),
            watch_exe: "ai-gent".into(),
            tmpdir: "/home/u/tmp".into(),
            model: DEFAULT_MODEL.into(),
            privacy_ack: false,
            home: None,
        }
    }

    #[test]
    fn run_args_match_the_contract() {
        assert_eq!(
            adapter().run_args(),
            [
                "run",
                "--standalone",
                "--model",
                "opencode/flint-40b-free",
                "--format",
                "json",
                "--auto",
                "{brief}"
            ]
        );
    }

    #[test]
    fn models_are_qualified_and_free_ones_recognized() {
        assert_eq!(qualify("beam-free"), "opencode/beam-free");
        assert_eq!(qualify("opencode/beam-free"), "opencode/beam-free");
        assert!(is_free_model("opencode/flint-40b-free"));
        assert!(!is_free_model("anthropic/claude-sonnet"));
        // A variant or tag suffix, and capitals, do not hide a free model.
        assert!(is_free_model("opencode/flint-40b-free#fast"));
        assert!(is_free_model("opencode/Flint-40B-FREE"));
        assert!(is_free_model("opencode/FLINT-FREE#Fast"));
        assert!(is_free_model("acme/some-model:free"));
        assert!(!is_free_model("acme/paid-pro#free-ish"));
        assert!(!is_free_model("acme/paid-pro#fast"));
        assert_eq!(qualify("beam-free#fast"), "opencode/beam-free#fast");
    }

    #[test]
    fn git_and_reader_denies_cover_paths_env_and_wrappers() {
        let rules = permission_rules(Role::Builder, None);
        let has = |p: &str| {
            rules
                .iter()
                .any(|r| r["action"] == "shell" && r["resource"] == p)
        };
        for p in [
            "git push*",
            "*/git push*",
            "* git push*",
            "* git commit*--no-verify*",
            "*/git -C*",
            "* git branch -D*",
            "cat *.env*",
            "*/cat *.env*",
            "* cat *.env*",
            "base64 *credentials*",
            "* xxd *.pem*",
            "head *id_rsa*",
            "strings *.key*",
            "less *.netrc*",
            "cp *id_ed25519*",
            "tail *connections.*",
        ] {
            assert!(has(p), "{p}");
        }
        assert!(
            rules
                .iter()
                .any(|r| r["action"] == "read" && r["resource"] == "*.key"),
            "key files are denied for OpenCode's own tools too"
        );
        // The set stays well inside the size of one environment variable.
        assert!(config_content(Role::Reviewer, Some(Path::new("/home/u"))).len() < 100_000);
    }

    #[test]
    fn home_entries_that_are_files_are_denied_themselves() {
        let home = tempfile::tempdir().unwrap();
        std::fs::write(home.path().join(".claude.json"), "{}").unwrap();
        std::fs::create_dir(home.path().join(".codex-work")).unwrap();
        let rules = permission_rules(Role::Builder, Some(home.path()));
        let h = slash(home.path());
        for entry in [".claude.json", ".codex-work", ".ssh"] {
            for a in ["read", "edit"] {
                for r in [
                    format!("{h}/{entry}"),
                    format!("{h}/{entry}/*"),
                    format!("{h}/{entry}/**"),
                    format!("~/{entry}"),
                ] {
                    assert!(
                        rules.iter().any(|x| x["action"] == a && x["resource"] == r),
                        "{a} {r}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_relative_binary_override_is_made_absolute_once() {
        let cwd = Path::new("/work/sup");
        assert_eq!(
            absolutize("bin/opencode".into(), Some(cwd)),
            Path::new("/work/sup/bin/opencode")
        );
        assert_eq!(
            absolutize("./opencode".into(), Some(cwd)),
            Path::new("/work/sup/./opencode")
        );
        assert_eq!(
            absolutize("/abs/opencode".into(), Some(cwd)),
            Path::new("/abs/opencode")
        );
        assert_eq!(
            absolutize("opencode".into(), Some(cwd)),
            Path::new("opencode")
        );
    }

    #[test]
    fn every_rule_is_a_deny_and_the_denies_cover_the_contract() {
        for role in [Role::Builder, Role::Reviewer, Role::Researcher] {
            let rules = permission_rules(role, Some(Path::new("/home/u")));
            assert!(rules.iter().all(|r| r["effect"] == "deny"), "{role:?}");
            let has =
                |a: &str, p: &str| rules.iter().any(|r| r["action"] == a && r["resource"] == p);
            for p in [
                "git push*",
                "git commit*--no-verify*",
                "git reset*",
                "git branch -D*",
                "curl*",
                "*.ssh*",
            ] {
                assert!(has("shell", p), "{p}");
            }
            assert!(has("webfetch", "*") && has("websearch", "*"));
            assert!(has("read", "~/.ssh/*") && has("read", "/home/u/.config/opencode/**"));
            assert!(has("edit", "*/.env"));
        }
    }

    #[test]
    fn only_read_only_roles_lose_edit_and_shell() {
        let blanket = |role| {
            let r = permission_rules(role, None);
            let has = |a: &str| r.iter().any(|x| x["action"] == a && x["resource"] == "*");
            (has("edit"), has("shell"))
        };
        assert_eq!(blanket(Role::Builder), (false, false));
        assert_eq!(blanket(Role::Reviewer), (true, true));
        assert_eq!(blanket(Role::Researcher), (true, true));
    }

    #[test]
    fn other_agents_directories_are_listed_by_name() {
        let home = tempfile::tempdir().unwrap();
        for d in [".claude-work", ".codex", ".config", "codes"] {
            std::fs::create_dir(home.path().join(d)).unwrap();
        }
        let rules = permission_rules(Role::Builder, Some(home.path()));
        let any = |needle: &str| {
            rules
                .iter()
                .any(|r| r["resource"].as_str().unwrap().contains(needle))
        };
        assert!(any("/.claude-work/*") && any("/.codex/**"));
        assert!(!any("/codes/") && !any("/.config/*"));
    }

    #[test]
    fn summary_sums_every_step_and_judges_by_the_final_stop() {
        let ev = r#"{"type":"step_start","part":{}}
{"type":"text","part":{"text":"working"}}
{"type":"step_finish","part":{"reason":"tool-calls","cost":0.5,"tokens":{"input":100,"output":10,"reasoning":1,"cache":{"read":50,"write":5}}}}
not json
{"type":"step_start","part":{}}
{"type":"text","part":{"text":"Done: T1"}}
{"type":"step_finish","part":{"reason":"stop","cost":0.25,"tokens":{"input":200,"output":20,"reasoning":2,"cache":{"read":150,"write":0}}}}"#;
        let s = summarize(ev);
        assert_eq!(
            s.usage,
            Some(Usage {
                input_tokens: 505,
                cached_input_tokens: 200,
                output_tokens: 30,
                reasoning_output_tokens: 3
            })
        );
        assert!((s.cost - 0.75).abs() < 1e-9);
        assert!(s.finished);
        assert_eq!(s.text.as_deref(), Some("Done: T1"));
        let cut = ev.lines().take(3).collect::<Vec<_>>().join("\n");
        assert!(!summarize(&cut).finished);
        assert_eq!(summarize("").usage, None);
    }

    #[test]
    fn a_free_model_is_refused_without_the_acknowledgement() {
        let e = adapter().privacy_gate().unwrap_err().to_string();
        assert!(
            e.contains("--privacy-ack") && e.contains("free hosted model"),
            "{e}"
        );
        assert!(adapter().with_privacy_ack(true).privacy_gate().is_ok());
        assert!(adapter().with_model("acme/paid-pro").privacy_gate().is_ok());
    }
}
