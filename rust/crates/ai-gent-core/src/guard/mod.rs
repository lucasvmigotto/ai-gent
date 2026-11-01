//! The guard evaluator (spec 033): one implementation of the five guards.
//!
//! A faithful port of `plugins/{git,db,secrets,repo,devcontainer}/hooks/guard.sh`. It runs in
//! shadow mode beside them: the shell guards still decide.
pub mod branch;
pub mod event;
pub mod reasons;
pub mod rules;
pub mod segment;
pub mod shadow;

pub use event::{Decision, Event, GuardId, Level};
pub use segment::Shell;

/// Largest command evaluated; longer ones are not judged (the caller asks).
pub const MAX_COMMAND_BYTES: usize = 256 * 1024;

/// What the evaluator needs from its surroundings.
pub struct Ctx<'a> {
    /// Current branch of a directory (`None` when unknown).
    pub branch_of: &'a dyn Fn(&str) -> Option<String>,
}

/// Evaluate one guard.
#[must_use]
pub fn evaluate(guard: GuardId, ev: &Event, shell: Shell, ctx: &Ctx) -> Decision {
    if !ev.is_shell() {
        // Only the db guard looks at other tools (credentials files as targets).
        return if guard == GuardId::Db {
            rules::db::creds_in_tool(ev)
        } else {
            Decision::none()
        };
    }
    let Some(cmd) = ev.command() else {
        return Decision::none();
    };
    let v = segment::view(shell, cmd, guard == GuardId::Containers);
    match guard {
        GuardId::Git => rules::git::evaluate(&v, &ev.cwd, ctx),
        GuardId::Db => rules::db::evaluate(&v),
        GuardId::Secrets => rules::secrets::evaluate(&v),
        GuardId::Repo => rules::repo::evaluate(&v),
        GuardId::Containers => rules::containers::evaluate(&v),
    }
}

/// Evaluate every guard: deny beats ask beats none; within a level the first of git, db, db
/// credentials, secrets, repo, containers.
#[must_use]
pub fn evaluate_all(ev: &Event, shell: Shell, ctx: &Ctx) -> Decision {
    let mut found: Vec<Decision> = Vec::new();
    if !ev.is_shell() {
        found.push(rules::db::creds_in_tool(ev));
    } else if let Some(cmd) = ev.command() {
        let plain = segment::view(shell, cmd, false);
        let ctr = segment::view(shell, cmd, true);
        found.push(rules::git::evaluate(&plain, &ev.cwd, ctx));
        found.push(rules::db::clients(&plain));
        found.push(rules::db::creds_in_command(&plain));
        found.push(rules::secrets::evaluate(&plain));
        found.push(rules::repo::evaluate(&plain));
        found.push(rules::containers::evaluate(&ctr));
    }
    for level in [Level::Deny, Level::Ask] {
        if let Some(d) = found.iter().find(|d| d.level == level) {
            return d.clone();
        }
    }
    Decision::none()
}

/// Evaluate an opencode permission event (`action`: shell, read or edit).
#[must_use]
pub fn evaluate_opencode(
    action: &str,
    resources: &[String],
    cwd: &str,
    shell: Shell,
    ctx: &Ctx,
) -> Decision {
    let mut found: Vec<Decision> = Vec::new();
    for r in resources {
        match action {
            "shell" => {
                let ev = Event {
                    tool_name: "Bash".into(),
                    cwd: cwd.into(),
                    tool_input: serde_json::json!({ "command": r }),
                };
                found.push(evaluate_all(&ev, shell, ctx));
            }
            "read" | "edit" => {
                let ev = Event {
                    tool_name: "Read".into(),
                    cwd: cwd.into(),
                    tool_input: serde_json::json!({ "file_path": r }),
                };
                found.push(rules::db::creds_in_tool(&ev));
            }
            _ => {}
        }
    }
    for level in [Level::Deny, Level::Ask] {
        if let Some(d) = found.iter().find(|d| d.level == level) {
            return d.clone();
        }
    }
    Decision::none()
}
