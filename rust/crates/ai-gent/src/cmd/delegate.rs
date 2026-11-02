//! `ai-gent delegate`: hand work to another coding agent and check what comes back (spec 041).
//!
//! The adapter operations (`detect`, `ready`, `preflight`, `run`, `status`, `stop`, `result`), the
//! brief builder, the supervisor's diff checks, the supervisor's commit, the status files and the
//! delegation log. No hook or skill calls it yet (decision D7).
use ai_gent_core::delegate::adapter::{Adapter, RunRequest};
use ai_gent_core::delegate::brief::{self, BriefInput};
use ai_gent_core::delegate::checks::{self, CheckInput};
use ai_gent_core::delegate::codex::CodexAdapter;
use ai_gent_core::delegate::status::{self, Role, State, Status, Verdict, VerdictResult};
use ai_gent_core::delegate::{self, Error, commit, gitx, watch};
use ai_gent_core::env::Env;
use ai_gent_core::exit::Exit;
use ai_gent_core::output::{self, ErrorCode};
use clap::{Args, Subcommand, ValueEnum};
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Arguments of `delegate`.
#[derive(Debug, Args)]
pub struct DelegateArgs {
    /// The repository root holding `.ai-gent/` (default: the main working tree of the current directory).
    #[arg(long, global = true)]
    root: Option<PathBuf>,
    /// The delegate agent.
    #[arg(long, global = true, value_enum, default_value = "codex")]
    agent: AgentId,
    /// What to run.
    #[command(subcommand)]
    action: Action,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum AgentId {
    Codex,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum RoleArg {
    Builder,
    Reviewer,
    Researcher,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum StateArg {
    Queued,
    Running,
    Stalled,
    Done,
    Failed,
    TakenBack,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum VerdictArg {
    Pending,
    Accepted,
    Rejected,
}

#[derive(Debug, Subcommand)]
enum Action {
    /// Is the agent's CLI on PATH, and does it run?
    Detect,
    /// Can the agent start (is the user logged in)?
    Ready,
    /// Prove a run can succeed in a worktree: stdin closed, writable, the agent starts.
    Preflight {
        /// The worktree.
        #[arg(long)]
        worktree: PathBuf,
    },
    /// Start the agent in the background on a brief.
    Run {
        /// The track id (the branch with `/` as `-`).
        #[arg(long)]
        track: String,
        /// The worktree.
        #[arg(long)]
        worktree: PathBuf,
        /// A file holding the brief (`delegate brief` writes it to stdout).
        #[arg(long)]
        brief_file: PathBuf,
        /// Task ids, comma separated.
        #[arg(long, value_delimiter = ',')]
        tasks: Vec<String>,
        /// The role.
        #[arg(long, value_enum, default_value = "builder")]
        role: RoleArg,
        /// Seconds without an event after which the track counts as stalled.
        #[arg(long, default_value_t = 600)]
        stall_secs: u64,
        /// Seconds to wait for the first event before stopping the agent (0: do not wait).
        #[arg(long, default_value_t = 60)]
        first_event_wait: u64,
    },
    /// Last event time, event count, running or exited.
    Status {
        /// The track id.
        track: String,
    },
    /// Stop the agent named by the track's PID file.
    Stop {
        /// The track id.
        track: String,
    },
    /// The agent's final message, exit code and usage.
    Result {
        /// The track id.
        track: String,
    },
    /// Build a brief from the template and print it.
    Brief {
        /// The repository name.
        #[arg(long)]
        repo: String,
        /// The worktree's branch.
        #[arg(long)]
        branch: String,
        /// The spec directory the tasks come from.
        #[arg(long)]
        spec: String,
        /// Task ids, comma separated.
        #[arg(long, value_delimiter = ',', required = true)]
        tasks: Vec<String>,
        /// A file with the task text, verbatim.
        #[arg(long)]
        task_file: PathBuf,
        /// A glob the delegate may touch (repeat).
        #[arg(long = "allow", required = true)]
        allowed: Vec<String>,
        /// A file or skill to read after AGENTS.md (repeat).
        #[arg(long = "pack")]
        pack: Vec<String>,
        /// A verification command (repeat).
        #[arg(long = "verify")]
        verify: Vec<String>,
        /// TMPDIR for the delegate (default: this process's).
        #[arg(long)]
        tmpdir: Option<String>,
    },
    /// Run the supervisor's diff checks (FR-011); exit 4 when any rule refuses.
    Check {
        /// The worktree.
        #[arg(long)]
        worktree: PathBuf,
        /// The revision the delegate started from.
        #[arg(long, default_value = "HEAD")]
        base: String,
        /// A glob from the brief's `allowed_paths` (repeat).
        #[arg(long = "allow", required = true)]
        allowed: Vec<String>,
        /// Evidence for a task mark, `T010=what ran and passed` (repeat).
        #[arg(long = "evidence")]
        evidence: Vec<String>,
        /// A file with extra text to scan for markers, such as a PR description (repeat).
        #[arg(long = "text-file")]
        text_files: Vec<PathBuf>,
        /// Record the verdict in this track's status file.
        #[arg(long)]
        track: Option<String>,
    },
    /// Stage the delegate's changes by explicit path (inside `allowed_paths` only) and commit.
    Commit {
        /// The worktree.
        #[arg(long)]
        worktree: PathBuf,
        /// A glob from the brief's `allowed_paths` (repeat).
        #[arg(long = "allow", required = true)]
        allowed: Vec<String>,
        /// The commit message (a Conventional Commit, no trailer or marker).
        #[arg(long, short = 'm')]
        message: String,
    },
    /// Create or update a track's status file (for Claude's own tracks and verdicts).
    Mark {
        /// The track id.
        track: String,
        /// The worker: claude, claude-subagent, codex or opencode[:model] (required for a new track).
        #[arg(long = "as")]
        worker: Option<String>,
        /// The role (required for a new track).
        #[arg(long, value_enum)]
        role: Option<RoleArg>,
        /// Task ids, comma separated (required for a new track).
        #[arg(long, value_delimiter = ',')]
        tasks: Vec<String>,
        /// The worktree relative to the repository root (required for a new track).
        #[arg(long)]
        worktree: Option<String>,
        /// The state.
        #[arg(long, value_enum)]
        state: Option<StateArg>,
        /// The verdict.
        #[arg(long, value_enum)]
        verdict: Option<VerdictArg>,
        /// The rule id of a rejected verdict.
        #[arg(long)]
        reason: Option<String>,
    },
    /// Print the delegation log.
    Log,
    /// Internal: supervise one agent run (started by `run`).
    #[command(name = "_watch", hide = true)]
    Watch {
        /// The track directory.
        dir: PathBuf,
    },
}

fn fail(e: &Error, json: bool) -> Exit {
    let (code, exit) = match e {
        Error::Refused(_) => (ErrorCode::Refused, Exit::Refused),
        Error::Invalid(_) | Error::NoFirstEvent(_) | Error::NotLoggedIn(_) => {
            (ErrorCode::Runtime, Exit::Runtime)
        }
        _ => (ErrorCode::Runtime, Exit::Runtime),
    };
    if json {
        println!("{}", output::error_json(code, &e.to_string(), None));
    } else {
        eprintln!("error: {e}");
    }
    exit
}

fn text(v: &Value, indent: usize) -> String {
    let pad = " ".repeat(indent);
    match v {
        Value::Object(m) => m
            .iter()
            .map(|(k, v)| match v {
                Value::Object(_) | Value::Array(_)
                    if !v
                        .as_array()
                        .is_some_and(|a| a.iter().all(|x| !x.is_object() && !x.is_array())) =>
                {
                    format!("{pad}{k}:\n{}", text(v, indent + 2))
                }
                _ => format!("{pad}{k}: {}", scalar(v)),
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Value::Array(a) => a
            .iter()
            .map(|x| match x {
                Value::Object(_) => format!("{pad}-\n{}", text(x, indent + 2)),
                _ => format!("{pad}- {}", scalar(x)),
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => format!("{pad}{}", scalar(v)),
    }
}

fn scalar(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Array(a) => a.iter().map(scalar).collect::<Vec<_>>().join(", "),
        Value::Null => "-".into(),
        other => other.to_string(),
    }
}

fn emit<T: Serialize>(data: &T, json: bool) {
    let v = serde_json::to_value(data).unwrap_or(Value::Null);
    if json {
        println!("{}", output::ok_json(1, v));
    } else {
        println!("{}", text(&v, 0));
    }
}

fn role(r: RoleArg) -> Role {
    match r {
        RoleArg::Builder => Role::Builder,
        RoleArg::Reviewer => Role::Reviewer,
        RoleArg::Researcher => Role::Researcher,
    }
}

fn resolve_root(a: &DelegateArgs) -> Result<PathBuf, Error> {
    match &a.root {
        Some(r) => Ok(r.clone()),
        None => {
            gitx::repo_root(&std::env::current_dir().map_err(|e| Error::Invalid(e.to_string()))?)
        }
    }
}

fn read(path: &Path) -> Result<String, Error> {
    std::fs::read_to_string(path).map_err(|e| Error::Invalid(format!("{}: {e}", path.display())))
}

/// Run the subcommand.
pub fn run(a: &DelegateArgs, json: bool) -> Exit {
    match go(a, json) {
        Ok(e) => e,
        Err(e) => fail(&e, json),
    }
}

#[allow(clippy::too_many_lines)]
fn go(a: &DelegateArgs, json: bool) -> Result<Exit, Error> {
    if let Action::Watch { dir } = &a.action {
        watch::watch(dir)?;
        return Ok(Exit::Success);
    }
    let env = Env::real();
    match &a.action {
        Action::Brief {
            repo,
            branch,
            spec,
            tasks,
            task_file,
            allowed,
            pack,
            verify,
            tmpdir,
        } => {
            let input = BriefInput {
                repo: repo.clone(),
                branch: branch.clone(),
                worker_pack: pack.clone(),
                spec: spec.clone(),
                task_ids: tasks.clone(),
                task_text: read(task_file)?,
                allowed_paths: allowed.clone(),
                verify: verify.clone(),
                tmpdir: tmpdir
                    .clone()
                    .or_else(|| env.text("TMPDIR"))
                    .unwrap_or_else(|| std::env::temp_dir().to_string_lossy().into_owned()),
            };
            let b = brief::build(&input)?;
            if json {
                println!("{}", output::ok_json(1, json!({ "brief": b })));
            } else {
                print!("{b}");
            }
            Ok(Exit::Success)
        }
        Action::Check {
            worktree,
            base,
            allowed,
            evidence,
            text_files,
            track,
        } => {
            let root = resolve_root(a)?;
            let (changes, mut texts) = gitx::collect(worktree, base)?;
            for f in text_files {
                texts.push(read(f)?);
            }
            let mut ev = BTreeMap::new();
            for e in evidence {
                let (id, what) = e
                    .split_once('=')
                    .ok_or_else(|| Error::Invalid(format!("--evidence '{e}': want T010=text")))?;
                ev.insert(id.trim().to_string(), what.to_string());
            }
            let findings = checks::run(&CheckInput {
                changes,
                texts,
                allowed: allowed.clone(),
                evidence: ev,
            });
            if let Some(t) = track {
                let mut s = status::read(&root, t)?;
                s.verdict = findings.first().map_or(
                    Verdict {
                        result: VerdictResult::Accepted,
                        reason: None,
                    },
                    |f| Verdict {
                        result: VerdictResult::Rejected,
                        reason: Some(f.reason.to_string()),
                    },
                );
                s.updated = delegate::now_secs().max(s.started);
                status::write(&root, &s)?;
            }
            let rows: Vec<Value> = findings
                .iter()
                .map(|f| json!({ "reason": f.reason, "path": f.path, "detail": f.detail }))
                .collect();
            emit(
                &json!({ "clean": findings.is_empty(), "findings": rows }),
                json,
            );
            Ok(if findings.is_empty() {
                Exit::Success
            } else {
                Exit::Refused
            })
        }
        Action::Commit {
            worktree,
            allowed,
            message,
        } => {
            let hash = commit::commit_changes(worktree, allowed, message)?;
            emit(&json!({ "commit": hash }), json);
            Ok(Exit::Success)
        }
        Action::Mark {
            track,
            worker,
            role: r,
            tasks,
            worktree,
            state: st,
            verdict,
            reason,
        } => {
            let root = resolve_root(a)?;
            let mut s = if let Ok(s) = status::read(&root, track) {
                s
            } else {
                let need = |o: Option<&str>, n: &str| {
                    o.map(str::to_string)
                        .ok_or_else(|| Error::Invalid(format!("a new track needs {n}")))
                };
                Status::new(
                    track,
                    &need(worker.as_deref(), "--as")?,
                    role(r.ok_or_else(|| Error::Invalid("a new track needs --role".into()))?),
                    tasks,
                    &need(worktree.as_deref(), "--worktree")?,
                    State::Running,
                )
            };
            if let Some(w) = worker {
                s.agent.clone_from(w);
            }
            if let Some(r) = r {
                s.role = role(*r);
            }
            if !tasks.is_empty() {
                s.tasks.clone_from(tasks);
            }
            if let Some(w) = worktree {
                s.worktree.clone_from(w);
            }
            if let Some(x) = st {
                s.state = match x {
                    StateArg::Queued => State::Queued,
                    StateArg::Running => State::Running,
                    StateArg::Stalled => State::Stalled,
                    StateArg::Done => State::Done,
                    StateArg::Failed => State::Failed,
                    StateArg::TakenBack => State::TakenBack,
                };
            }
            if let Some(v) = verdict {
                s.verdict = Verdict {
                    result: match v {
                        VerdictArg::Pending => VerdictResult::Pending,
                        VerdictArg::Accepted => VerdictResult::Accepted,
                        VerdictArg::Rejected => VerdictResult::Rejected,
                    },
                    reason: reason.clone(),
                };
            }
            s.updated = delegate::now_secs().max(s.started);
            status::write(&root, &s)?;
            emit(&s, json);
            Ok(Exit::Success)
        }
        Action::Log => {
            let root = resolve_root(a)?;
            let lines = status::read_log(&root)?;
            emit(&json!({ "runs": lines }), json);
            Ok(Exit::Success)
        }
        _ => adapter_action(a, json, &env),
    }
}

fn adapter_action(a: &DelegateArgs, json: bool, env: &Env) -> Result<Exit, Error> {
    let root = resolve_root(a)?;
    let ad: Box<dyn Adapter> = match a.agent {
        AgentId::Codex => Box::new(CodexAdapter::new(&root, env)),
    };
    match &a.action {
        Action::Detect => {
            let d = ad.detect();
            emit(&d, json);
            Ok(if d.found {
                Exit::Success
            } else {
                Exit::NothingToDo
            })
        }
        Action::Ready => {
            ad.ready()?;
            emit(&json!({ "agent": ad.id(), "ready": true }), json);
            Ok(Exit::Success)
        }
        Action::Preflight { worktree } => {
            let p = ad.preflight(worktree);
            emit(&p, json);
            Ok(if p.ok { Exit::Success } else { Exit::Runtime })
        }
        Action::Run {
            track,
            worktree,
            brief_file,
            tasks,
            role: r,
            stall_secs,
            first_event_wait,
        } => {
            let h = ad.run(&RunRequest {
                track: track.clone(),
                worktree: std::fs::canonicalize(worktree).unwrap_or_else(|_| worktree.clone()),
                brief: read(brief_file)?,
                tasks: tasks.clone(),
                role: role(*r),
                stall_secs: *stall_secs,
                first_event_wait: Duration::from_secs(*first_event_wait),
            })?;
            emit(&h, json);
            Ok(Exit::Success)
        }
        Action::Status { track } => {
            emit(&ad.status(track)?, json);
            Ok(Exit::Success)
        }
        Action::Stop { track } => {
            emit(&ad.stop(track)?, json);
            Ok(Exit::Success)
        }
        Action::Result { track } => {
            emit(&ad.result(track)?, json);
            Ok(Exit::Success)
        }
        _ => unreachable!("handled in go"),
    }
}
