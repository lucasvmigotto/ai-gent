//! The supervisor's commit (FR-007, FR-008): a Codex delegate cannot write `.git`, so it leaves its
//! changes in the working tree and the supervisor stages them by explicit path, only inside
//! `allowed_paths`, and commits with the project's message rules and no marker.
use super::checks::is_marker_line;
use super::gitx::git;
use super::glob;
use super::{Error, Res};
use std::path::Path;

const TYPES: [&str; 11] = [
    "feat", "fix", "docs", "style", "refactor", "perf", "test", "build", "ci", "chore", "revert",
];

/// Whether the first line is a Conventional Commit header (`type(scope)!: subject`).
#[must_use]
pub fn conventional(first_line: &str) -> bool {
    let Some((head, subject)) = first_line.split_once(": ") else {
        return false;
    };
    let head = head.strip_suffix('!').unwrap_or(head);
    let ty = match head.split_once('(') {
        Some((t, rest)) => {
            if !rest.ends_with(')') || rest.len() < 2 {
                return false;
            }
            t
        }
        None => head,
    };
    TYPES.contains(&ty) && !subject.trim().is_empty()
}

/// Why `message` may not be used, if it may not.
#[must_use]
pub fn message_problem(message: &str) -> Option<String> {
    let first = message.lines().next().unwrap_or("");
    if !conventional(first) {
        return Some("the first line must be a Conventional Commit header".into());
    }
    message
        .lines()
        .find(|l| is_marker_line(l))
        .map(|_| "the message carries a trailer or marker".into())
}

/// Paths with changes in `worktree` (modified, added, deleted and untracked), relative to it.
///
/// # Errors
/// When git fails.
pub fn changed_paths(worktree: &Path) -> Res<Vec<String>> {
    let out = git(
        worktree,
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
    )?;
    let mut paths = Vec::new();
    let mut it = out.split('\0').filter(|s| !s.is_empty());
    while let Some(entry) = it.next() {
        let (xy, path) = entry.split_at(entry.len().min(3));
        paths.push(path.to_string());
        if xy.starts_with('R') || xy.starts_with('C') {
            // a rename entry is followed by the old path
            if let Some(old) = it.next() {
                paths.push(old.to_string());
            }
        }
    }
    paths.sort();
    paths.dedup();
    Ok(paths)
}

/// Stage the working tree's changes by explicit path and commit them; returns the commit hash.
///
/// Refuses, before touching the index, when any changed path is outside `allowed` or when the
/// message is not a marker-free Conventional Commit. Hooks run (no `--no-verify`).
///
/// # Errors
/// [`Error::Refused`] for the refusals above or when there is nothing to commit; git errors.
pub fn commit_changes(worktree: &Path, allowed: &[String], message: &str) -> Res<String> {
    if let Some(p) = message_problem(message) {
        return Err(Error::Refused(p));
    }
    let paths = changed_paths(worktree)?;
    if paths.is_empty() {
        return Err(Error::Refused("nothing to commit".into()));
    }
    let outside: Vec<&String> = paths
        .iter()
        .filter(|p| !allowed.iter().any(|g| glob::matches(g, p)))
        .collect();
    if !outside.is_empty() {
        return Err(Error::Refused(format!(
            "path-outside-allowed: {}",
            outside
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }
    let mut add: Vec<&str> = vec!["add", "--"];
    add.extend(paths.iter().map(String::as_str));
    git(worktree, &add)?;
    let mut commit: Vec<&str> = vec!["commit", "-m", message, "--"];
    commit.extend(paths.iter().map(String::as_str));
    git(worktree, &commit)?;
    Ok(git(worktree, &["rev-parse", "--short", "HEAD"])?
        .trim()
        .to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn sh(dir: &Path, args: &[&str]) {
        let s = Command::new("git")
            .args([
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .current_dir(dir)
            .status()
            .unwrap();
        assert!(s.success(), "{args:?}");
    }

    fn repo() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        sh(d.path(), &["init", "-q", "-b", "main"]);
        sh(d.path(), &["config", "user.name", "t"]);
        sh(d.path(), &["config", "user.email", "t@t"]);
        sh(d.path(), &["config", "commit.gpgsign", "false"]);
        std::fs::write(d.path().join("seed.txt"), "seed\n").unwrap();
        sh(d.path(), &["add", "seed.txt"]);
        sh(d.path(), &["commit", "-q", "-m", "chore: seed"]);
        d
    }

    #[test]
    fn message_rules() {
        assert!(conventional("chore(delegate): add x"));
        assert!(conventional("feat!: add x"));
        assert!(!conventional("added x"));
        assert!(!conventional("wip: x"));
        assert!(!conventional("feat(): x"));
        assert!(message_problem("fix: a\n\nCo-Authored-By: B <b@b>").is_some());
        assert!(message_problem("fix: a\n\nbody text").is_none());
    }

    #[test]
    fn commits_only_inside_allowed_paths_and_by_explicit_path() {
        let d = repo();
        std::fs::create_dir_all(d.path().join("src")).unwrap();
        std::fs::write(d.path().join("src/a.rs"), "a\n").unwrap();
        std::fs::write(d.path().join("seed.txt"), "seed2\n").unwrap();
        let allowed = vec!["src/**".to_string(), "seed.txt".to_string()];
        let h = commit_changes(d.path(), &allowed, "feat(x): add a").unwrap();
        assert!(!h.is_empty());
        assert!(changed_paths(d.path()).unwrap().is_empty());
        let body = git(d.path(), &["log", "-1", "--format=%B"]).unwrap();
        assert_eq!(body.trim(), "feat(x): add a");
    }

    #[test]
    fn refuses_outside_paths_without_staging_anything() {
        let d = repo();
        std::fs::write(d.path().join("src.rs"), "a\n").unwrap();
        std::fs::write(d.path().join("other.txt"), "b\n").unwrap();
        let e = commit_changes(d.path(), &["src.rs".to_string()], "feat: x").unwrap_err();
        assert!(
            e.to_string().contains("path-outside-allowed: other.txt"),
            "{e}"
        );
        assert!(
            git(d.path(), &["diff", "--cached", "--name-only"])
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn refuses_markers_and_empty() {
        let d = repo();
        assert!(commit_changes(d.path(), &["x".to_string()], "feat: x").is_err());
        std::fs::write(d.path().join("x"), "1\n").unwrap();
        let e = commit_changes(
            d.path(),
            &["x".to_string()],
            "feat: x\n\nGenerated with Foo",
        )
        .unwrap_err();
        assert!(e.to_string().contains("marker"));
    }

    #[test]
    fn deletions_and_staged_leftovers() {
        let d = repo();
        std::fs::remove_file(d.path().join("seed.txt")).unwrap();
        // something staged outside the paths must not ride along
        std::fs::write(d.path().join("keep.txt"), "k\n").unwrap();
        sh(d.path(), &["add", "keep.txt"]);
        let e = commit_changes(d.path(), &["seed.txt".to_string()], "fix: drop seed");
        assert!(e.is_err(), "keep.txt is outside allowed_paths");
        sh(d.path(), &["reset", "-q", "keep.txt"]);
        std::fs::remove_file(d.path().join("keep.txt")).unwrap();
        commit_changes(d.path(), &["seed.txt".to_string()], "fix: drop seed").unwrap();
        assert!(!d.path().join("seed.txt").exists());
    }
}
