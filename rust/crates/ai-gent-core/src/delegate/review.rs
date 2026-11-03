//! The cross-agent review step (`ai-gent delegate review`, FR-010, FR-011, task T021).
//!
//! The supervisor stops running the reviewer by hand: this module decides whether a reviewer may review
//! a builder's track, builds the review prompt from the worktree, and waits for the reviewer's final
//! message. The run itself goes through the ordinary adapters in role `reviewer` as the track
//! `review-<builder-track>`, so its status file, token usage and delegation-log line are the existing ones.
//!
//! # The refusal rule (FR-010: the reviewer is never the builder)
//!
//! A reviewer is the builder when it is the same agent **and** the same profile or configuration:
//!
//! - `codex`: no profile exists, so Codex never reviews a Codex track.
//! - `claude`: the profile is the config directory (`CLAUDE_CONFIG_DIR`). A builder of the agent `claude`
//!   ran in `AI_GENT_CLAUDE_CONFIG_DIR`; `claude-subagent` ran in the supervisor's own profile
//!   (`CLAUDE_CONFIG_DIR`, else `~/.claude`). The reviewer's profile is given explicitly. The two are
//!   compared as canonical paths (the directories are never read): equal means refused, so the
//!   supervisor's own profile may review a second profile's work and the other way round, never its own.
//! - `opencode`: the configuration is the model (`opencode` alone means the default model); the same
//!   qualified model is refused, a different model is allowed.
//!
//! A builder track's own report (its `last.txt`) is never read here, so it cannot reach the prompt.
use super::adapter::{Adapter, RunResult};
use super::checks::{self, CheckInput};
use super::gitx::sensitive;
use super::status;
use super::{Error, Res, gitx, io, state};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// The reviewer agents `review` can start.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// The Codex CLI.
    Codex,
    /// A Claude Code profile.
    Claude,
    /// The `OpenCode` CLI.
    Opencode,
}

impl Kind {
    /// The agent id.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
            Self::Opencode => "opencode",
        }
    }
}

/// One side of the comparison: an agent and its profile or configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Party {
    /// The agent.
    pub kind: Kind,
    /// Claude: the canonical config directory; `OpenCode`: the qualified model; Codex: none.
    pub profile: Option<String>,
}

fn canon(p: &Path) -> String {
    let c = std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    super::slash(&c).trim_end_matches('/').to_string()
}

impl Party {
    /// A Claude reviewer or builder in the profile `dir`.
    #[must_use]
    pub fn claude(dir: &Path) -> Self {
        Self {
            kind: Kind::Claude,
            profile: Some(canon(dir)),
        }
    }

    /// An `OpenCode` party on `model` (qualified like the adapter does).
    #[must_use]
    pub fn opencode(model: &str) -> Self {
        Self {
            kind: Kind::Opencode,
            profile: Some(super::opencode::qualify(model)),
        }
    }

    /// The party a builder status file names (`status.json` `agent`).
    ///
    /// `builder_profile` is `AI_GENT_CLAUDE_CONFIG_DIR` and `supervisor_profile` the supervisor's own
    /// profile directory.
    ///
    /// # Errors
    /// When the agent is not one a review can compare.
    pub fn of_builder(
        agent: &str,
        builder_profile: Option<&Path>,
        supervisor_profile: &Path,
    ) -> Res<Self> {
        match agent {
            "codex" => Ok(Self {
                kind: Kind::Codex,
                profile: None,
            }),
            "claude" => builder_profile.map(Self::claude).ok_or_else(|| {
                Error::Invalid(
                    "the builder ran as claude: set AI_GENT_CLAUDE_CONFIG_DIR to its profile so the reviewer can be compared"
                        .into(),
                )
            }),
            "claude-subagent" => Ok(Self::claude(supervisor_profile)),
            "opencode" => Ok(Self::opencode(super::opencode::DEFAULT_MODEL)),
            other => other
                .strip_prefix("opencode:")
                .map(Self::opencode)
                .ok_or_else(|| Error::Invalid(format!("unknown builder agent '{other}'"))),
        }
    }
}

/// `None` when `reviewer` may review `builder`, else the one-line reason for the refusal (FR-010).
#[must_use]
pub fn refusal(builder: &Party, reviewer: &Party) -> Option<String> {
    if builder != reviewer {
        return None;
    }
    Some(format!(
        "the reviewer must not be the builder: both are {}{} (FR-010); pick another agent{}",
        builder.kind.id(),
        builder
            .profile
            .as_ref()
            .map_or(String::new(), |p| format!(" with profile/config {p}")),
        match builder.kind {
            Kind::Codex => "",
            Kind::Claude => " or another profile (--reviewer-profile)",
            Kind::Opencode => " or another model (--reviewer-model)",
        }
    ))
}

/// The review track for a builder track.
///
/// # Errors
/// When the result is not a valid track id.
pub fn review_track(builder: &str) -> Res<String> {
    let t = format!("review-{builder}");
    if state::valid_track(&t) {
        Ok(t)
    } else {
        Err(Error::Invalid(format!("invalid review track id '{t}'")))
    }
}

/// The revision a worktree's work started from: the merge base with the first of `dev`, `main`,
/// `master` that exists, else `HEAD`.
#[must_use]
pub fn default_base(worktree: &Path) -> String {
    for cand in ["dev", "main", "master"] {
        if let Ok(h) = gitx::git(worktree, &["merge-base", "HEAD", cand]) {
            let h = h.trim();
            if !h.is_empty() {
                return h.to_string();
            }
        }
    }
    "HEAD".to_string()
}

/// Largest untracked file put into the diff text, and the cap on each embedded block.
const MAX_FILE: u64 = 64 * 1024;
const MAX_BLOCK: usize = 400 * 1024;

fn clip_block(mut s: String, what: &str) -> String {
    if s.len() > MAX_BLOCK {
        let mut n = MAX_BLOCK;
        while !s.is_char_boundary(n) {
            n -= 1;
        }
        s.truncate(n);
        let _ = write!(s, "\n[{what} truncated at {MAX_BLOCK} bytes]\n");
    }
    s
}

/// The paths a `diff --git a/<old> b/<new>` header names (quotes dropped).
fn header_paths(header: &str) -> Vec<String> {
    let rest = header.trim_start_matches("diff --git ").replace('"', "");
    match rest.rfind(" b/") {
        Some(i) => vec![
            rest[..i].trim_start_matches("a/").to_string(),
            rest[i + 3..].to_string(),
        ],
        None => vec![rest],
    }
}

/// Drop the sections of a tracked diff whose file is sensitive; each is replaced by a "left out" line.
fn filter_tracked(diff: &str) -> String {
    let mut out = String::with_capacity(diff.len());
    let mut skipping = false;
    for line in diff.split_inclusive('\n') {
        if line.starts_with("diff --git ") {
            let paths = header_paths(line.trim_end());
            skipping = paths.iter().any(|p| sensitive(p));
            if skipping {
                let _ = writeln!(
                    out,
                    "[tracked {}: left out, sensitive name]",
                    paths.last().map_or("", String::as_str)
                );
            }
        }
        if !skipping {
            out.push_str(line);
        }
    }
    out
}

/// The diff text of `worktree` against `base`: tracked changes (staged or not, commits since `base`
/// included) and untracked files as new files. Files that look like credentials, tracked or not, are
/// left out and said so; an untracked symlink is never followed and is listed as skipped.
///
/// # Errors
/// When a git call fails.
pub fn diff_text(worktree: &Path, base: &str) -> Res<String> {
    let tracked = gitx::git(
        worktree,
        &[
            "diff",
            "--no-color",
            "--no-ext-diff",
            "--find-renames",
            base,
            "--",
        ],
    )?;
    let mut out = filter_tracked(&tracked);
    let untracked = gitx::git(
        worktree,
        &["ls-files", "--others", "--exclude-standard", "-z"],
    )?;
    for rel in untracked.split('\0').filter(|s| !s.is_empty()) {
        if rel.starts_with(".ai-gent/") {
            continue;
        }
        if sensitive(rel) {
            let _ = write!(out, "\n[untracked {rel}: left out, sensitive name]\n");
            continue;
        }
        let p = worktree.join(rel);
        let meta = std::fs::symlink_metadata(&p).ok();
        if meta.as_ref().is_some_and(|m| m.file_type().is_symlink()) {
            let _ = write!(out, "\n[untracked {rel}: skipped, a symlink]\n");
            continue;
        }
        let big = meta.is_none_or(|m| !m.is_file() || m.len() > MAX_FILE);
        let text = if big {
            None
        } else {
            std::fs::read(&p)
                .ok()
                .and_then(|b| String::from_utf8(b).ok())
        };
        match text {
            Some(t) => {
                let _ = write!(out, "\ndiff --git a/{rel} b/{rel}\nnew file\n+++ b/{rel}\n");
                for l in t.lines() {
                    out.push('+');
                    out.push_str(l);
                    out.push('\n');
                }
            }
            None => {
                let _ = write!(
                    out,
                    "\n[untracked {rel}: not shown (binary or over {MAX_FILE} bytes)]\n"
                );
            }
        }
    }
    Ok(clip_block(out, "diff"))
}

/// The FR-011 findings of `delegate check` as one line each, for the prompt.
///
/// # Errors
/// When a git call fails.
pub fn check_lines(
    worktree: &Path,
    base: &str,
    allowed: &[String],
    evidence: &std::collections::BTreeMap<String, String>,
) -> Res<Vec<String>> {
    let (changes, texts) = gitx::collect(worktree, base)?;
    let findings = checks::run(&CheckInput {
        changes,
        texts,
        allowed: allowed.to_vec(),
        evidence: evidence.clone(),
    });
    Ok(findings
        .iter()
        .map(|f| {
            format!(
                "{}{}: {}",
                f.reason,
                f.path.as_ref().map_or(String::new(), |p| format!(" ({p})")),
                f.detail
            )
        })
        .collect())
}

/// Read a spec file for the prompt.
///
/// # Errors
/// When it is a symlink, resolves outside `root` or to a sensitive path, or cannot be read as text.
pub fn read_spec(root: &Path, path: &Path) -> Res<String> {
    let refuse = |why: &str| Error::Refused(format!("{}: {why}", path.display()));
    let sens = "a credential or agent profile path is never put in a review prompt";
    if sensitive(&super::slash(path)) {
        return Err(refuse(sens));
    }
    let meta = io(
        format!("stat {}", path.display()),
        std::fs::symlink_metadata(path),
    )?;
    if meta.file_type().is_symlink() {
        return Err(refuse("a symlink is never followed for a spec file"));
    }
    let real = io(
        format!("resolve {}", path.display()),
        std::fs::canonicalize(path),
    )?;
    let root_real = io(
        format!("resolve {}", root.display()),
        std::fs::canonicalize(root),
    )?;
    let Ok(inside) = real.strip_prefix(&root_real) else {
        return Err(refuse("resolves outside the repository root"));
    };
    if sensitive(&super::slash(inside)) {
        return Err(refuse(sens));
    }
    let t = io(
        format!("read {}", real.display()),
        std::fs::read_to_string(&real),
    )?;
    Ok(clip_block(t, "file"))
}

/// What the review prompt is made of.
#[derive(Debug, Clone)]
pub struct PromptInput {
    /// The builder's track id.
    pub builder_track: String,
    /// The base revision the diff is against.
    pub base: String,
    /// The diff text.
    pub diff: String,
    /// Named spec files: `(path, contents)`.
    pub specs: Vec<(String, String)>,
    /// `delegate check` findings, or `None` when the check was not run.
    pub findings: Option<Vec<String>>,
}

/// The standing review instructions (supervisor.md step 4).
const INSTRUCTIONS: &str = "\
You are the reviewer of a change another agent made. You did not write it.
- Read-only: edit nothing, create no file, run nothing that writes. You have no network.
- Do not read credentials, tokens, keys, `.env` files or any agent profile directory.
- The change is the diff below. Check it against the spec files below and against the files in the
  working directory. Report only what you can show.
- Rank the findings by severity (high, medium, low). Each finding carries file:line and a concrete
  failure scenario (input or state, then the wrong result). No scenario, no finding.
- Recount every number you cite (counts, line numbers, sizes) from the files before you write it.
- List what you could not check.
- No report from the builder is provided and none is wanted: judge the diff and the spec, not claims.";

/// The whole prompt is one command-line argument for the agent; Linux caps one argument near 128 KiB
/// (`E2BIG`), so the prompt stays well under that.
pub const MAX_PROMPT: usize = 100 * 1024;
/// Room kept per block for its "[truncated: N bytes left out]" note and the fences around it.
const NOTE_RESERVE: usize = 256;
/// The least room the spec and diff blocks need together for a review to make sense.
const MIN_CONTEXT: usize = 4 * 1024;

/// `s` cut to at most `max` bytes (on a char boundary) with a note of how much was left out.
fn cut(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut n = max.saturating_sub(NOTE_RESERVE).min(s.len());
    while !s.is_char_boundary(n) {
        n -= 1;
    }
    format!("{}\n[truncated: {} bytes left out]\n", &s[..n], s.len() - n)
}

/// The review prompt. It holds the diff text because a reviewer of the Claude profile has no shell.
/// Spec and diff blocks are cut so the whole prompt stays within [`MAX_PROMPT`] (the specs take at most
/// half of the room, the diff the rest), each cut saying how many bytes it left out.
///
/// # Errors
/// When the instructions and findings leave less than a few KiB for the spec and diff blocks.
pub fn build_prompt(i: &PromptInput) -> Res<String> {
    let mut p = format!(
        "{INSTRUCTIONS}\n\nTrack under review: {} (diff against {})\n",
        i.builder_track, i.base
    );
    p.push_str("\n## Supervisor check findings (FR-011)\n");
    match &i.findings {
        None => p.push_str("not run (no allowed paths were given; --no-check was passed, the check was skipped on request)\n"),
        Some(f) if f.is_empty() => p.push_str("none\n"),
        Some(f) => {
            for l in f {
                let _ = writeln!(p, "- {l}");
            }
        }
    }
    let blocks = i.specs.len() + 1;
    let fixed = p.len()
        + i.specs.iter().map(|(n, _)| n.len() + 32).sum::<usize>()
        + 32
        + NOTE_RESERVE * blocks;
    let room = MAX_PROMPT.saturating_sub(fixed);
    if room < MIN_CONTEXT {
        return Err(Error::Invalid(format!(
            "the review prompt cannot fit in {MAX_PROMPT} bytes: the instructions and findings take {fixed}; shorten the findings or give fewer --spec files"
        )));
    }
    let spec_total: usize = i.specs.iter().map(|(_, t)| t.len()).sum();
    let mut spec_left = spec_total.min(room / 2);
    let diff_room = room - spec_left;
    for (path, text) in &i.specs {
        let part = cut(text, spec_left.min(text.len()) + NOTE_RESERVE);
        spec_left = spec_left.saturating_sub(text.len());
        let _ = write!(p, "\n## Spec file {path}\n```\n{part}\n```\n");
    }
    p.push_str("\n## Diff\n```diff\n");
    p.push_str(&cut(&i.diff, diff_room + NOTE_RESERVE));
    if !p.ends_with('\n') {
        p.push('\n');
    }
    p.push_str("```\n");
    if p.len() > MAX_PROMPT {
        return Err(Error::Invalid(format!(
            "the review prompt is {} bytes, over the {MAX_PROMPT} limit",
            p.len()
        )));
    }
    Ok(p)
}

/// Wait for the reviewer to end. On timeout the run is stopped (by PID file) and the call fails.
///
/// # Errors
/// When the status file is unreadable, or the review timed out.
pub fn await_result(ad: &dyn Adapter, track: &str, timeout: Duration) -> Res<RunResult> {
    let t = Instant::now();
    loop {
        let r = ad.result(track)?;
        if r.finished {
            return Ok(r);
        }
        if t.elapsed() >= timeout {
            let _ = ad.stop(track);
            return Err(Error::Invalid(format!(
                "the review did not end within {}s; stopped",
                timeout.as_secs()
            )));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// The builder's status (agent, tasks, worktree) read from `status.json`.
///
/// # Errors
/// When the builder track has no readable status file.
pub fn builder_status(root: &Path, track: &str) -> Res<status::Status> {
    status::read(root, track)
}

/// Where the review is written by default.
///
/// # Errors
/// When the builder track id is invalid.
pub fn default_out(root: &Path, builder: &str) -> Res<PathBuf> {
    Ok(state::track_dir(root, &review_track(builder)?)?.join("review.md"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(p: &str) -> Party {
        Party::claude(Path::new(p))
    }

    #[test]
    fn same_agent_same_config_is_refused() {
        let codex = Party::of_builder("codex", None, Path::new("/s")).unwrap();
        assert!(refusal(&codex, &codex).is_some());
        assert!(refusal(&d("/a"), &d("/a")).unwrap().contains("FR-010"));
        let m = Party::opencode("flint-40b-free");
        assert!(refusal(&m, &Party::opencode("opencode/flint-40b-free")).is_some());
    }

    #[test]
    fn opencode_builder_model_is_compared() {
        let b = Party::of_builder("opencode:opencode/x", None, Path::new("/s")).unwrap();
        assert!(refusal(&b, &Party::opencode("x")).is_some());
        assert!(refusal(&b, &Party::opencode("y")).is_none());
        let old = Party::of_builder("opencode", None, Path::new("/s")).unwrap();
        assert!(
            refusal(
                &old,
                &Party::opencode(super::super::opencode::DEFAULT_MODEL)
            )
            .is_some()
        );
    }

    #[test]
    fn a_different_agent_or_config_is_allowed() {
        let codex = Party::of_builder("codex", None, Path::new("/s")).unwrap();
        assert!(refusal(&codex, &d("/a")).is_none());
        assert!(refusal(&d("/a"), &d("/b")).is_none());
        assert!(refusal(&Party::opencode("a"), &Party::opencode("b")).is_none());
        let sub = Party::of_builder("claude-subagent", None, Path::new("/s")).unwrap();
        assert!(refusal(&sub, &d("/s")).is_some());
        assert!(refusal(&sub, &d("/other")).is_none());
    }

    #[test]
    fn the_prompt_carries_diff_spec_and_findings() {
        let p = build_prompt(&PromptInput {
            builder_track: "feat-x".into(),
            base: "abc".into(),
            diff: "+added line".into(),
            specs: vec![("spec.md".into(), "FR-001 text".into())],
            findings: Some(vec!["paths (a.rs): outside".into()]),
        })
        .unwrap();
        for s in [
            "+added line",
            "FR-001 text",
            "paths (a.rs)",
            "file:line",
            "Recount every number",
        ] {
            assert!(p.contains(s), "{s}");
        }
    }

    #[test]
    fn sensitive_paths_are_recognised() {
        for s in [
            ".env",
            "a/.env.local",
            "x/.claude-b/y",
            "k.pem",
            "~/.ssh/id",
            ".netrc",
            "home/.netrc",
            ".npmrc",
            "a/.pypirc",
            ".git-credentials",
            "id_rsa",
            "d/id_ed25519",
            "id_ecdsa",
            "id_dsa",
            "a.key",
            "a.p12",
            "a.pfx",
            "credentials.json",
            "secrets.yaml",
            "x/secrets",
        ] {
            assert!(sensitive(s), "{s}");
        }
        assert!(!sensitive("src/main.rs"));
    }

    fn input(diff: String, specs: Vec<(String, String)>) -> PromptInput {
        PromptInput {
            builder_track: "t".into(),
            base: "b".into(),
            diff,
            specs,
            findings: Some(vec![]),
        }
    }

    #[test]
    fn a_huge_diff_is_cut_to_fit_with_a_note() {
        let diff = "+x".repeat(512 * 1024);
        let spec = "s".repeat(300 * 1024);
        let p = build_prompt(&input(diff, vec![("spec.md".into(), spec)])).unwrap();
        assert!(p.len() <= MAX_PROMPT, "{}", p.len());
        assert!(
            p.matches("[truncated: ").count() == 2,
            "diff and spec are both cut"
        );
        assert!(p.contains("bytes left out]"));
    }

    #[test]
    fn a_small_prompt_is_not_cut_and_a_prompt_that_cannot_fit_is_refused() {
        let p = build_prompt(&input("+a\n".into(), vec![])).unwrap();
        assert!(!p.contains("truncated"));
        let mut i = input("+a\n".into(), vec![]);
        i.findings = Some(vec!["f".repeat(MAX_PROMPT)]);
        let e = build_prompt(&i).unwrap_err().to_string();
        assert!(e.contains("cannot fit"), "{e}");
    }

    #[test]
    fn a_sensitive_tracked_section_is_left_out() {
        let diff = "diff --git a/src/a.rs b/src/a.rs\n+ok\ndiff --git a/.env b/.env\n+SECRET=1\ndiff --git a/k.pem b/x/k.pem\n+KEY\ndiff --git a/b.rs b/b.rs\n+fine\n";
        let f = filter_tracked(diff);
        assert!(f.contains("+ok") && f.contains("+fine"));
        assert!(!f.contains("SECRET") && !f.contains("+KEY"));
        assert!(f.contains("[tracked .env: left out"));
    }

    #[test]
    fn track_ids() {
        assert_eq!(review_track("feat-x").unwrap(), "review-feat-x");
        assert!(review_track("Bad").is_err());
    }
}
