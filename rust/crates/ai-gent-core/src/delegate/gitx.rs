//! The few git calls the supervisor needs, run with `git -c core.quotepath=off`.
use super::diff::{self, FileChange};
use super::{Error, Res, io};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Run git in `cwd`; stdout on success.
///
/// # Errors
/// [`Error::Git`] with the first line of stderr when git exits non-zero.
pub fn git(cwd: &Path, args: &[&str]) -> Res<String> {
    let out = io(
        format!("run git {}", args.join(" ")),
        Command::new("git")
            .arg("-c")
            .arg("core.quotepath=off")
            .args(args)
            .current_dir(cwd)
            .stdin(std::process::Stdio::null())
            .output(),
    )?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(Error::Git {
            args: args.join(" "),
            message: String::from_utf8_lossy(&out.stderr)
                .lines()
                .next()
                .unwrap_or("failed")
                .to_string(),
        })
    }
}

/// The main working tree's root, also when `cwd` is a linked worktree: where `.ai-gent/` lives.
///
/// # Errors
/// When `cwd` is not in a git repository.
pub fn repo_root(cwd: &Path) -> Res<PathBuf> {
    let common = git(
        cwd,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )?;
    let common = PathBuf::from(common.trim());
    common
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| Error::Invalid("the git common dir has no parent".into()))
}

/// The common git directory of a worktree (absolute).
///
/// # Errors
/// When `worktree` is not in a git repository.
pub fn common_dir(worktree: &Path) -> Res<PathBuf> {
    Ok(PathBuf::from(
        git(
            worktree,
            &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        )?
        .trim(),
    ))
}

/// The changes in `worktree` since `base` (staged, unstaged and untracked files) and the commit
/// messages in `base..HEAD`.
///
/// # Errors
/// When a git call fails.
pub fn collect(worktree: &Path, base: &str) -> Res<(Vec<FileChange>, Vec<String>)> {
    let text = git(
        worktree,
        &[
            "diff",
            "--no-color",
            "--no-ext-diff",
            "-U0",
            "--find-renames",
            base,
            "--",
        ],
    )?;
    let mut changes = diff::parse(&text);
    let untracked = git(
        worktree,
        &["ls-files", "--others", "--exclude-standard", "-z"],
    )?;
    for rel in untracked.split('\0').filter(|s| !s.is_empty()) {
        let bytes = std::fs::read(worktree.join(rel)).unwrap_or_default();
        let content = String::from_utf8(bytes).unwrap_or_default();
        changes.push(FileChange::added_file(rel, &content));
    }
    let range = format!("{base}..HEAD");
    let log = git(worktree, &["log", "--format=%B%x00", &range])?;
    let msgs = log
        .split('\0')
        .map(|m| m.trim().to_string())
        .filter(|m| !m.is_empty())
        .collect();
    Ok((changes, msgs))
}
