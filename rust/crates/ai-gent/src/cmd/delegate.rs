//! `ai-gent delegate`: hand work to another coding agent and check what comes back (spec 041).
//!
//! The adapter operations (`detect`, `ready`, `preflight`, `run`, `status`, `stop`, `result`), the
//! scheduler (`plan`), the source check (`verify-sources`), the
//! brief builder, the supervisor's diff checks, the supervisor's commit, the status files and the
//! delegation log. No hook or skill calls it yet (decision D7).
use ai_gent_core::delegate::adapter::{Adapter, RunRequest};
use ai_gent_core::delegate::brief::{self, BriefInput};
use ai_gent_core::delegate::checks::{self, CheckInput};
use ai_gent_core::delegate::claude::ClaudeAdapter;
use ai_gent_core::delegate::codex::CodexAdapter;
use ai_gent_core::delegate::opencode::OpenCodeAdapter;
use ai_gent_core::delegate::review::{self, Kind, Party};
use ai_gent_core::delegate::schedule::{self, Pool};
use ai_gent_core::delegate::sources;
use ai_gent_core::delegate::status::{self, Role, State, Status, Verdict, VerdictResult};
use ai_gent_core::delegate::{self, Error, commit, gitx, watch};
use ai_gent_core::env::Env;
use ai_gent_core::exit::Exit;
use ai_gent_core::output::{self, ErrorCode};
use ai_gent_core::wt;
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
    Claude,
    Opencode,
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
        /// `OpenCode` only: the model, as for `run`.
        #[arg(long)]
        model: Option<String>,
        /// `OpenCode` only: the privacy notice was given, as for `run`; without it the check for a free
        /// model fails.
        #[arg(long)]
        privacy_ack: bool,
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
        /// `OpenCode` only: the model (`flint-40b-free` or `opencode/flint-40b-free`; default
        /// `opencode/flint-40b-free`).
        #[arg(long)]
        model: Option<String>,
        /// `OpenCode` only: the privacy notice for free hosted models was given (they may use prompts
        /// to improve the provider's models). Without it a free model run is refused.
        #[arg(long)]
        privacy_ack: bool,
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
    /// Plan a task list: tracks, who runs each, and which start now (JSON in, JSON out).
    Plan {
        /// A JSON file with the task list (`[{id, kind, paths, needs, depends}]`); `-` reads stdin.
        #[arg(long)]
        tasks_file: PathBuf,
        /// A delegate agent the project allows, in preference order (repeat; default `codex`).
        #[arg(long = "allow-agent")]
        allow_agent: Vec<String>,
        /// The disk floor for this call, in GB (default `disk_floor_gb`, else max(20, 10%)).
        #[arg(long)]
        min_free_gb: Option<f64>,
    },
    /// Re-fetch the sources a claim cites and compare its checkable terms; exit 4 unless confirmed.
    VerifySources {
        /// The claim text.
        #[arg(
            long,
            conflicts_with = "claim_file",
            required_unless_present = "claim_file"
        )]
        claim: Option<String>,
        /// A file holding the claim text.
        #[arg(long)]
        claim_file: Option<PathBuf>,
        /// A cited source: an http(s) URL (fetched with curl) or a local path (repeat).
        #[arg(long = "source", required = true)]
        sources: Vec<String>,
    },
    /// Review a builder track's worktree with a different agent, read-only (FR-010); exit 4 when the
    /// reviewer would be the builder.
    Review {
        /// The builder's track id (its `status.json` names the builder's agent).
        #[arg(long)]
        builder_track: String,
        /// The reviewer agent.
        #[arg(long, value_enum)]
        reviewer: AgentId,
        /// The worktree (default: the builder track's).
        #[arg(long)]
        worktree: Option<PathBuf>,
        /// A spec file to put in the prompt (repeat).
        #[arg(long = "spec")]
        specs: Vec<PathBuf>,
        /// Where to write the review (default `.ai-gent/delegate/review-<track>/review.md`).
        #[arg(long)]
        out: Option<PathBuf>,
        /// The revision the work started from (default: the merge base with dev, main or master).
        #[arg(long)]
        base: Option<String>,
        /// A glob from the brief's `allowed_paths` (repeat); required, the FR-011 check runs before the
        /// review and its findings go into the prompt.
        #[arg(long = "allow")]
        allowed: Vec<String>,
        /// Review without the FR-011 check (only when no `--allow` is given; the prompt says so).
        #[arg(long)]
        no_check: bool,
        /// Evidence for a task mark the FR-011 check looks for, `T010=what ran and passed` (repeat).
        #[arg(long = "evidence")]
        evidence: Vec<String>,
        /// Claude reviewer: its config directory (default `AI_GENT_CLAUDE_CONFIG_DIR`).
        #[arg(long)]
        reviewer_profile: Option<PathBuf>,
        /// `OpenCode` reviewer: its model (default the adapter's).
        #[arg(long)]
        reviewer_model: Option<String>,
        /// `OpenCode` reviewer: the privacy notice for free hosted models was given.
        #[arg(long)]
        privacy_ack: bool,
        /// Seconds to wait for the reviewer before stopping it.
        #[arg(long, default_value_t = 1800)]
        timeout_secs: u64,
        /// Print the prompt and the refusal decision; launch nothing, write nothing.
        #[arg(long)]
        dry_run: bool,
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
        Action::Plan {
            tasks_file,
            allow_agent,
            min_free_gb,
        } => {
            let root = resolve_root(a)?;
            let raw = if tasks_file.as_os_str() == "-" {
                let mut b = String::new();
                std::io::Read::read_to_string(&mut std::io::stdin(), &mut b)
                    .map_err(|e| Error::Invalid(format!("stdin: {e}")))?;
                b
            } else {
                read(tasks_file)?
            };
            let tasks: Vec<schedule::Task> = serde_json::from_str(&raw)
                .map_err(|e| Error::Invalid(format!("task list: {e}")))?;
            let allowed = if allow_agent.is_empty() {
                vec!["codex".to_string()]
            } else {
                allow_agent.clone()
            };
            let (pool, view) = plan_pool(&root, *min_free_gb);
            let p = schedule::plan(&tasks, &allowed, &pool)?;
            let mut v = serde_json::to_value(&p).unwrap_or(Value::Null);
            v["pool"] = view;
            emit(&v, json);
            Ok(Exit::Success)
        }
        Action::VerifySources {
            claim,
            claim_file,
            sources,
        } => {
            let text = match (claim, claim_file) {
                (Some(c), _) => c.clone(),
                (None, Some(f)) => read(f)?,
                (None, None) => return Err(Error::Invalid("give --claim or --claim-file".into())),
            };
            let r = sources::verify(&text, sources)?;
            emit(&r, json);
            Ok(if r.outcome == sources::Outcome::Confirmed {
                Exit::Success
            } else {
                Exit::Refused
            })
        }
        Action::Review { .. } => review_action(a, json, &env),
        Action::Log => {
            let root = resolve_root(a)?;
            let lines = status::read_log(&root)?;
            emit(&json!({ "runs": lines }), json);
            Ok(Exit::Success)
        }
        _ => adapter_action(a, json, &env),
    }
}

const GB: f64 = 1e9;

/// Tracks holding a slot: a status file in `queued`, `running` or `stalled`. Unreadable files are
/// counted apart and do not hold one.
fn live_tracks(root: &Path) -> (usize, usize) {
    let dir = root.join(".ai-gent").join("delegate");
    let (mut live, mut bad) = (0, 0);
    let Ok(rd) = std::fs::read_dir(&dir) else {
        return (0, 0);
    };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if !e.path().join("status.json").is_file() {
            continue;
        }
        match status::read(root, &name) {
            Ok(s) if matches!(s.state, State::Queued | State::Running | State::Stalled) => {
                live += 1;
            }
            Ok(_) => {}
            Err(_) => bad += 1,
        }
    }
    (live, bad)
}

/// The pool from `.ai-gent/parallel.yaml` (cap `max_volumes`, else `max_parallel`, default 3) and the
/// container store's free space, the numbers `wt.py add` checks. Without a container engine the disk
/// is not checked (as `wt.py` does) and only the cap binds.
fn plan_pool(root: &Path, flag: Option<f64>) -> (Pool, Value) {
    let rs = root.to_string_lossy().into_owned();
    let s = wt::settings(&rs);
    let (live, unreadable) = live_tracks(root);
    let cap = usize::try_from(s.volume_cap().max(0)).unwrap_or(0);
    let est = s.volume_estimate_gb() * GB;
    #[allow(clippy::cast_precision_loss)]
    let usage = wt::engine::engine().and_then(|e| wt::disk::store_usage(&e).ok());
    #[allow(clippy::cast_precision_loss)]
    let (free, floor, checked) = usage.map_or((f64::INFINITY, 0.0, false), |(total, free, _)| {
        (free as f64, wt::disk::floor(&s, total, flag), true)
    });
    let pool = Pool {
        cap,
        live,
        free,
        floor,
        estimate: est,
    };
    let view = json!({
        "cap": cap,
        "live": live,
        "unreadable_status_files": unreadable,
        "disk_checked": checked,
        "free_gb": if checked { json!((free / GB * 10.0).round() / 10.0) } else { Value::Null },
        "floor_gb": if checked { json!((floor / GB * 10.0).round() / 10.0) } else { Value::Null },
        "estimate_gb": est / GB,
    });
    (pool, view)
}

/// `delegate review`: decide, build the prompt from the worktree, run the reviewer (role `reviewer`,
/// track `review-<builder-track>`) and write its final message.
#[allow(clippy::too_many_lines)]
fn review_action(a: &DelegateArgs, json: bool, env: &Env) -> Result<Exit, Error> {
    let Action::Review {
        builder_track,
        reviewer,
        worktree,
        specs,
        out,
        base,
        allowed,
        no_check,
        evidence,
        reviewer_profile,
        reviewer_model,
        privacy_ack,
        timeout_secs,
        dry_run,
    } = &a.action
    else {
        unreachable!("handled in go");
    };
    if allowed.is_empty() && !*no_check {
        // FR-011: a change is checked against the task's limits before anyone reviews it.
        let msg = "--allow is required: the FR-011 check runs before the review (pass the brief's allowed_paths with --allow, or --no-check to review without it)";
        if json {
            println!("{}", output::error_json(ErrorCode::Usage, msg, None));
        } else {
            eprintln!("error: {msg}");
        }
        return Ok(Exit::Usage);
    }
    let root = resolve_root(a)?;
    let bst = review::builder_status(&root, builder_track)?;
    let supervisor = env
        .path("CLAUDE_CONFIG_DIR")
        .unwrap_or_else(|| env.home_dir().unwrap_or_default().join(".claude"));
    let builder = Party::of_builder(
        &bst.agent,
        env.path("AI_GENT_CLAUDE_CONFIG_DIR").as_deref(),
        &supervisor,
    )?;
    let claude_dir = reviewer_profile
        .clone()
        .or_else(|| env.path("AI_GENT_CLAUDE_CONFIG_DIR"));
    let model = reviewer_model.clone();
    let (party, ad): (Party, Box<dyn Adapter>) = match reviewer {
        AgentId::Codex => (
            Party {
                kind: Kind::Codex,
                profile: None,
            },
            Box::new(CodexAdapter::new(&root, env)),
        ),
        AgentId::Claude => {
            let dir = claude_dir.ok_or_else(|| {
                Error::Invalid(
                    "a Claude reviewer needs its profile: pass --reviewer-profile or set AI_GENT_CLAUDE_CONFIG_DIR"
                        .into(),
                )
            })?;
            let mut c = ClaudeAdapter::new(&root, env);
            c.config_dir = Some(dir.clone());
            (Party::claude(&dir), Box::new(c))
        }
        AgentId::Opencode => {
            let mut oc = OpenCodeAdapter::new(&root, env).with_privacy_ack(*privacy_ack);
            if let Some(m) = &model {
                oc = oc.with_model(m);
            }
            (Party::opencode(&oc.model), Box::new(oc))
        }
    };
    let refused = review::refusal(&builder, &party);
    let track = review::review_track(builder_track)?;

    let wt = worktree.clone().unwrap_or_else(|| root.join(&bst.worktree));
    let base = base.clone().unwrap_or_else(|| review::default_base(&wt));
    let mut spec_texts = Vec::new();
    for s in specs {
        spec_texts.push((delegate::slash(s), review::read_spec(&root, s)?));
    }
    let findings = if allowed.is_empty() {
        None
    } else {
        let mut ev = BTreeMap::new();
        for e in evidence {
            let (id, what) = e
                .split_once('=')
                .ok_or_else(|| Error::Invalid(format!("--evidence '{e}': want T010=text")))?;
            ev.insert(id.trim().to_string(), what.to_string());
        }
        Some(review::check_lines(&wt, &base, allowed, &ev)?)
    };
    let blocking = findings.clone().filter(|f| !f.is_empty());
    let prompt = review::build_prompt(&review::PromptInput {
        builder_track: builder_track.clone(),
        base: base.clone(),
        diff: review::diff_text(&wt, &base)?,
        specs: spec_texts,
        findings,
    })?;
    let out_path = match out {
        Some(o) => o.clone(),
        None => review::default_out(&root, builder_track)?,
    };
    if *dry_run {
        let ok = refused.is_none() && blocking.is_none();
        emit(
            &json!({
                "dry_run": true,
                "allowed": ok,
                "launched": false,
                "refusal": refused,
                "findings": blocking.clone().unwrap_or_default(),
                "builder": bst.agent,
                "reviewer": reviewer_id(*reviewer),
                "track": track,
                "out": delegate::slash(&out_path),
                "prompt": prompt,
            }),
            json,
        );
        return Ok(if ok { Exit::Success } else { Exit::Refused });
    }
    if let Some(why) = refused {
        return Err(Error::Refused(why));
    }
    if let Some(f) = blocking {
        // FR-011: the change breaks the task's limits; it is not reviewed, it is sent back.
        emit(
            &json!({ "clean": false, "findings": f, "launched": false }),
            json,
        );
        return Ok(Exit::Refused);
    }
    ad.run(&RunRequest {
        track: track.clone(),
        worktree: std::fs::canonicalize(&wt).unwrap_or(wt),
        brief: prompt,
        tasks: bst.tasks.clone(),
        role: Role::Reviewer,
        stall_secs: 600,
        first_event_wait: Duration::from_secs(60),
    })?;
    let r = review::await_result(ad.as_ref(), &track, Duration::from_secs(*timeout_secs))?;
    // Success is the adapter's verdict (`result`), not the exit code: OpenCode may end with exit 1
    // after a final stop and the adapter then reports `done`.
    let text = r
        .message
        .clone()
        .filter(|_| r.state == State::Done && !r.is_error);
    let Some(text) = text else {
        return Err(Error::Invalid(format!(
            "the reviewer ended without a review (state {:?}, exit {:?}{}); see .ai-gent/delegate/{track}/",
            r.state,
            r.exit_code,
            r.reason
                .as_ref()
                .map_or(String::new(), |x| format!(", {x}"))
        )));
    };
    if let Some(dir) = out_path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| Error::Invalid(format!("{}: {e}", dir.display())))?;
    }
    std::fs::write(&out_path, &text)
        .map_err(|e| Error::Invalid(format!("{}: {e}", out_path.display())))?;
    emit(
        &json!({
            "track": track,
            "builder": bst.agent,
            "reviewer": reviewer_id(*reviewer),
            "out": delegate::slash(&out_path),
            "usage": r.usage,
            "wall_secs": r.wall_secs,
        }),
        json,
    );
    Ok(Exit::Success)
}

const fn reviewer_id(a: AgentId) -> &'static str {
    match a {
        AgentId::Codex => "codex",
        AgentId::Claude => "claude",
        AgentId::Opencode => "opencode",
    }
}

fn adapter_action(a: &DelegateArgs, json: bool, env: &Env) -> Result<Exit, Error> {
    let root = resolve_root(a)?;
    let ad: Box<dyn Adapter> = match a.agent {
        AgentId::Codex => Box::new(CodexAdapter::new(&root, env)),
        AgentId::Claude => Box::new(ClaudeAdapter::new(&root, env)),
        AgentId::Opencode => {
            let mut oc = OpenCodeAdapter::new(&root, env);
            if let Action::Run {
                model, privacy_ack, ..
            }
            | Action::Preflight {
                model, privacy_ack, ..
            } = &a.action
            {
                if let Some(m) = model {
                    oc = oc.with_model(m);
                }
                oc = oc.with_privacy_ack(*privacy_ack);
            }
            Box::new(oc)
        }
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
        Action::Preflight { worktree, .. } => {
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
            ..
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
