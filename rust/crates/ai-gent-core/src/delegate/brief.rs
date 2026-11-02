//! The brief builder (`contracts/brief-template.md`, FR-005): one prompt for a delegate, built from
//! the template with `allowed_paths`, the worker pack, the verification commands and the hard limits.
use super::glob;
use super::{Error, Res};

/// The worker rules every brief names first (FR-015).
pub const WORKER_RULES: &str = "shared/delegate-worker-rules.md";

const HARD_LIMITS: &str = "Hard limits: no credentials or secrets; no database or container tools; no network (you have none); no edits to skills, plugins, hooks, shared references, AGENTS.md or CLAUDE.md; no version bumps, tags or CHANGELOG edits; no trailers or markers of any kind in commits; no --no-verify; no force-push; stage by explicit path; never delete anything you did not create.";
const ENV_FAILURES: &str = "Environment failures (denied sockets, caches, missing tools) go in your final message, never in a repository file.";
const NO_COMMIT: &str = "Leave your changes in the working tree and stop. Do not commit or push (the supervisor commits).";
const FINAL: &str = "Final message (short): per task done or skipped and why, commit hashes, and \"Rules I found hard to follow\".";

/// What the brief is built from.
#[derive(Debug, Clone, Default)]
pub struct BriefInput {
    /// The repository name.
    pub repo: String,
    /// The worktree's branch.
    pub branch: String,
    /// Files to read after `AGENTS.md`: skills and references the task needs. The worker rules file is
    /// added first when missing.
    pub worker_pack: Vec<String>,
    /// The spec the tasks come from (`specs/041-agent-delegation`).
    pub spec: String,
    /// Task ids (`T010`).
    pub task_ids: Vec<String>,
    /// The task text, verbatim.
    pub task_text: String,
    /// Globs the delegate may touch.
    pub allowed_paths: Vec<String>,
    /// The task's own verification commands (targeted tests only).
    pub verify: Vec<String>,
    /// The `TMPDIR` the delegate uses.
    pub tmpdir: String,
}

/// Whether a repository path is one no delegate may edit (FR-004, the brief's hard limits): skills,
/// plugins, hooks, shared references, the agent instruction files and the changelog.
#[must_use]
pub fn protected_path(path: &str) -> bool {
    let p = glob::norm(path);
    let segs: Vec<&str> = p.split('/').collect();
    let first = segs.first().copied().unwrap_or("");
    let name = segs.last().copied().unwrap_or("");
    matches!(first, "plugins" | "shared" | "hooks" | "skills")
        || p.starts_with("opencode/guard")
        || segs.contains(&".claude-plugin")
        || matches!(name, "AGENTS.md" | "CLAUDE.md" | "CHANGELOG.md")
}

fn one_line(what: &str, v: &str) -> Res<()> {
    if v.trim().is_empty() {
        return Err(Error::Invalid(format!("brief: {what} is empty")));
    }
    if v.contains(['\n', '\r']) {
        return Err(Error::Invalid(format!("brief: {what} must be one line")));
    }
    Ok(())
}

/// Check the input against the contract.
///
/// # Errors
/// [`Error::Invalid`] for an empty required field, an `allowed_paths` entry that is absolute, climbs
/// out of the repository, covers everything, or reaches a protected path, and for a bad task id.
pub fn validate(i: &BriefInput) -> Res<()> {
    one_line("repo", &i.repo)?;
    one_line("branch", &i.branch)?;
    one_line("tmpdir", &i.tmpdir)?;
    one_line("spec", &i.spec)?;
    if i.task_text.trim().is_empty() {
        return Err(Error::Invalid("brief: the task text is empty".into()));
    }
    if i.task_ids.is_empty() {
        return Err(Error::Invalid("brief: no task ids".into()));
    }
    for t in &i.task_ids {
        if !super::status::valid_task_id(t) {
            return Err(Error::Invalid(format!("brief: '{t}' is not a task id")));
        }
    }
    for f in i.worker_pack.iter().chain(&i.verify) {
        one_line("a worker pack or verify entry", f)?;
    }
    if i.allowed_paths.is_empty() {
        return Err(Error::Invalid("brief: allowed_paths is empty".into()));
    }
    for a in &i.allowed_paths {
        one_line("an allowed path", a)?;
        let n = glob::norm(a);
        let abs = a.starts_with('/') || a.starts_with('\\') || a.contains(':');
        if abs || n.split('/').any(|s| s == "..") {
            return Err(Error::Invalid(format!(
                "brief: allowed path '{a}' must be relative and stay inside the repository"
            )));
        }
        let prefix = glob::literal_prefix(a);
        if prefix.is_empty() {
            return Err(Error::Invalid(format!(
                "brief: allowed path '{a}' covers the whole repository"
            )));
        }
        if protected_path(&prefix) {
            return Err(Error::Invalid(format!(
                "brief: allowed path '{a}' reaches a protected path (skills, plugins, hooks, shared references, AGENTS.md, CLAUDE.md, CHANGELOG.md)"
            )));
        }
    }
    Ok(())
}

/// The brief text.
///
/// # Errors
/// What [`validate`] refuses.
pub fn build(i: &BriefInput) -> Res<String> {
    validate(i)?;
    let mut pack: Vec<&str> = vec![WORKER_RULES];
    for p in &i.worker_pack {
        if p != WORKER_RULES {
            pack.push(p);
        }
    }
    let verify = if i.verify.is_empty() {
        "the task's own targeted tests".to_string()
    } else {
        i.verify.join("; ")
    };
    Ok(format!(
        "You are a contributor to {repo} (this directory is a git worktree on branch {branch}).\n\
Read first, in order: AGENTS.md; {pack}.\n\
Task: {spec} {ids}: {text}\n\
allowed_paths: {allowed}; touching anything else fails review automatically.\n\
Verify with: {verify}.\n\
{HARD_LIMITS}\n\
Environment: TMPDIR={tmp}; build output outside the worktree.\n\
{ENV_FAILURES}\n\
{NO_COMMIT}\n\
{FINAL}\n",
        repo = i.repo,
        branch = i.branch,
        pack = pack.join(", "),
        spec = i.spec,
        ids = i.task_ids.join(", "),
        text = i.task_text.trim_end(),
        allowed = i.allowed_paths.join(", "),
        tmp = i.tmpdir,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn sample() -> BriefInput {
        BriefInput {
            repo: "ai-gent".into(),
            branch: "feat/db-spike".into(),
            worker_pack: vec!["git:workflow".into()],
            spec: "specs/038-db-runner".into(),
            task_ids: vec!["T003".into(), "T013".into()],
            task_text: "Add the _exec process bridge and the --runner flag.\nKeep the default path unchanged.".into(),
            allowed_paths: vec!["rust/spikes/dbrun/**".into(), "specs/038-db-runner/tasks.md".into()],
            verify: vec!["cargo test -p dbrun-spike".into()],
            tmpdir: "/home/u/tmp".into(),
        }
    }

    #[test]
    fn golden_brief() {
        let got = build(&sample()).unwrap();
        crate::delegate::tests_support::golden("brief/trial2.txt", &got);
    }

    #[test]
    fn golden_minimal_brief() {
        let i = BriefInput {
            repo: "demo".into(),
            branch: "feat/min".into(),
            spec: "specs/001-demo".into(),
            task_ids: vec!["T001".into()],
            task_text: "Add one function.".into(),
            allowed_paths: vec!["src/".into()],
            tmpdir: "/tmp/w".into(),
            ..BriefInput::default()
        };
        crate::delegate::tests_support::golden("brief/minimal.txt", &build(&i).unwrap());
    }

    #[test]
    fn worker_rules_come_first_once() {
        let mut i = sample();
        i.worker_pack = vec!["git:workflow".into(), WORKER_RULES.into()];
        let b = build(&i).unwrap();
        assert!(b.contains("AGENTS.md; shared/delegate-worker-rules.md, git:workflow."));
        assert_eq!(b.matches(WORKER_RULES).count(), 1);
    }

    #[test]
    fn limits_and_no_commit_are_always_present() {
        let b = build(&sample()).unwrap();
        for need in [
            "no credentials or secrets",
            "no database or container tools",
            "no network (you have none)",
            "no trailers or markers of any kind in commits",
            "no --no-verify",
            "stage by explicit path",
            "Do not commit or push (the supervisor commits).",
            "Rules I found hard to follow",
        ] {
            assert!(b.contains(need), "{need}");
        }
    }

    #[test]
    fn refuses_bad_scopes() {
        for bad in [
            "**",
            "*.md",
            "/etc/x",
            "../x/**",
            "plugins/agents/**",
            "shared/",
            "AGENTS.md",
            "docs/CLAUDE.md",
            "opencode/guard/**",
            "C:/x",
        ] {
            let mut i = sample();
            i.allowed_paths = vec![bad.into()];
            assert!(build(&i).is_err(), "{bad}");
        }
        let mut i = sample();
        i.allowed_paths.clear();
        assert!(build(&i).is_err());
        let mut i = sample();
        i.task_ids = vec!["x".into()];
        assert!(build(&i).is_err());
        let mut i = sample();
        i.branch = "a\nb".into();
        assert!(build(&i).is_err());
    }

    #[test]
    fn default_verify_text() {
        let mut i = sample();
        i.verify.clear();
        assert!(
            build(&i)
                .unwrap()
                .contains("Verify with: the task's own targeted tests.")
        );
    }
}
