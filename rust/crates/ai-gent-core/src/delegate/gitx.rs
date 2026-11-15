//! The few git calls the supervisor needs, run with `git -c core.quotepath=off`.
use super::diff::{self, FileChange};
use super::{Error, Res, io};
use crate::env::Env;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

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

/// How long one snapshot of the main checkout may take before it counts as failed.
pub const SNAPSHOT_TIMEOUT: Duration = Duration::from_secs(30);

/// The key holding the main checkout's `HEAD` in a snapshot (a commit or checkout there changes it).
const HEAD_KEY: &str = "<HEAD>";

/// One git call for a snapshot: the supervisor's environment minus git's location variables (git must
/// read `main`, not the directory a leftover `GIT_DIR` names), no optional locks, a timeout.
fn snapshot_git(
    program: &str,
    main: &Path,
    args: &[&str],
    timeout: Duration,
) -> Res<crate::exec::Output> {
    let mut env = Env::real();
    for k in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_COMMON_DIR",
        "GIT_INDEX_FILE",
        "GIT_PREFIX",
    ] {
        env.remove(k);
    }
    env.set("GIT_OPTIONAL_LOCKS", "0");
    let mut c = Command::new(program);
    c.args(args).current_dir(main);
    let out = io(
        format!("run git {}", args.join(" ")),
        crate::exec::run(c, &env, None, timeout),
    )?;
    if out.timed_out {
        return Err(Error::Invalid(format!(
            "git {} timed out after {} s",
            args.join(" "),
            timeout.as_secs()
        )));
    }
    if out.code != 0 {
        return Err(Error::Git {
            args: args.join(" "),
            message: String::from_utf8_lossy(&out.stderr)
                .lines()
                .next()
                .unwrap_or("failed")
                .to_string(),
        });
    }
    Ok(out)
}

fn canon(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

/// The state of the main checkout `main`: path to a fingerprint (status code, size, mtime) as `git
/// status` reports it (untracked files one by one), plus its `HEAD`. Left out: `.ai-gent/`, anything
/// under `skip` (canonicalised first) or under another linked worktree of the repository, and
/// directories (a submodule or nested repository shows as one). Ignored files are never seen. Read-only:
/// no index refresh, and each git call is bounded by [`SNAPSHOT_TIMEOUT`].
///
/// # Errors
/// When `git status` cannot run in `main`, fails, or times out.
pub fn snapshot(main: &Path, skip: &[PathBuf]) -> Res<std::collections::BTreeMap<String, String>> {
    snapshot_using("git", main, skip, SNAPSHOT_TIMEOUT)
}

/// [`snapshot`] with the git program and the timeout chosen (for tests).
///
/// # Errors
/// As [`snapshot`].
pub fn snapshot_using(
    program: &str,
    main: &Path,
    skip: &[PathBuf],
    timeout: Duration,
) -> Res<std::collections::BTreeMap<String, String>> {
    let out = snapshot_git(
        program,
        main,
        &["status", "--porcelain", "-z", "--untracked-files=all"],
        timeout,
    )?;
    let cmain = canon(main);
    let mut skips: Vec<PathBuf> = skip.iter().map(|s| canon(s)).collect();
    if let Ok(w) = snapshot_git(program, main, &["worktree", "list", "--porcelain"], timeout) {
        let text = String::from_utf8_lossy(&w.stdout).into_owned();
        // The first entry is the main working tree itself.
        skips.extend(
            text.lines()
                .filter_map(|l| l.strip_prefix("worktree "))
                .skip(1)
                .map(|p| canon(Path::new(p))),
        );
    }
    let head = snapshot_git(program, main, &["rev-parse", "HEAD"], timeout).map_or_else(
        |_| "unborn".to_string(),
        |o| String::from_utf8_lossy(&o.stdout).trim().to_string(),
    );
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let mut parts = text.split('\0').filter(|p| !p.is_empty());
    let mut map = std::collections::BTreeMap::new();
    map.insert(HEAD_KEY.to_string(), head);
    while let Some(e) = parts.next() {
        let (code, path) = e.split_at(3.min(e.len()));
        if code.starts_with(['R', 'C']) {
            parts.next();
        }
        let abs = cmain.join(path);
        if path.starts_with(".ai-gent/")
            || path.ends_with('/')
            || abs.is_dir()
            || skips.iter().any(|s| abs.starts_with(s))
        {
            continue;
        }
        let print = std::fs::metadata(&abs).map_or_else(
            |_| format!("{} gone", code.trim()),
            |m| {
                let t = m
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_nanos());
                format!("{} {} {t}", code.trim(), m.len())
            },
        );
        map.insert(path.to_string(), print);
    }
    Ok(map)
}

/// The paths whose state differs between `before` and `after`: new or changed entries, entries that
/// disappeared (a file deleted, a change reverted or stashed) and `HEAD` when it moved.
#[must_use]
pub fn changed_since(
    before: &std::collections::BTreeMap<String, String>,
    after: &std::collections::BTreeMap<String, String>,
) -> Vec<String> {
    let mut keys: Vec<&String> = after
        .iter()
        .filter(|(k, v)| before.get(*k) != Some(*v))
        .map(|(k, _)| k)
        .chain(before.keys().filter(|k| !after.contains_key(*k)))
        .collect();
    keys.sort();
    keys.dedup();
    keys.into_iter()
        .map(|k| {
            if k == HEAD_KEY {
                "HEAD (a commit or checkout in the main checkout)".to_string()
            } else {
                k.clone()
            }
        })
        .collect()
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

/// Whether a path names something that holds credentials or an agent profile; its content is never put
/// in a prompt or a finding. The file names come from [`super::opencode::is_credential_name`].
#[must_use]
pub fn sensitive(path: &str) -> bool {
    let comps: Vec<&str> = path.split(['/', '\\']).collect();
    let name = comps.last().copied().unwrap_or("");
    comps.iter().any(|c| {
        c.starts_with(".claude")
            || c.starts_with(".codex")
            || matches!(*c, ".ssh" | ".gnupg" | ".password-store" | ".aws")
    }) || super::opencode::is_credential_name(name)
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
    // A tracked credential-named file keeps its path (the path rules still see it) but none of its
    // lines, so no finding can quote a value from it.
    for c in &mut changes {
        if sensitive(&c.path) || c.old_path.as_deref().is_some_and(sensitive) {
            c.added.clear();
            c.removed.clear();
        }
    }
    let untracked = git(
        worktree,
        &["ls-files", "--others", "--exclude-standard", "-z"],
    )?;
    for rel in untracked.split('\0').filter(|s| !s.is_empty()) {
        // The path stays in the change list (the path rules still see it); the content of a symlink
        // or a credential-named file is never read, so no finding can print it.
        let p = worktree.join(rel);
        let plain =
            !sensitive(rel) && std::fs::symlink_metadata(&p).is_ok_and(|m| m.file_type().is_file());
        let content = if plain {
            String::from_utf8(std::fs::read(&p).unwrap_or_default()).unwrap_or_default()
        } else {
            String::new()
        };
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

#[cfg(test)]
mod snapshot_tests {
    use super::*;

    fn g(dir: &Path, args: &[&str]) {
        let s = Command::new("git")
            .args(args)
            .current_dir(dir)
            .status()
            .unwrap();
        assert!(s.success(), "{args:?}");
    }

    /// A repository with `a.txt` committed and a `.gitignore` for `*.log`.
    fn repo() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        g(d.path(), &["init", "-q", "-b", "main"]);
        g(d.path(), &["config", "user.email", "t@example.invalid"]);
        g(d.path(), &["config", "user.name", "t"]);
        std::fs::write(d.path().join("a.txt"), "a\n").unwrap();
        std::fs::write(d.path().join(".gitignore"), "*.log\n").unwrap();
        g(d.path(), &["add", "."]);
        g(d.path(), &["commit", "-q", "-m", "init"]);
        d
    }

    fn changed(d: &Path, f: impl FnOnce()) -> Vec<String> {
        let before = snapshot(d, &[]).unwrap();
        f();
        changed_since(&before, &snapshot(d, &[]).unwrap())
    }

    #[test]
    fn a_new_file_is_reported_and_an_ignored_one_is_not() {
        let d = repo();
        let c = changed(d.path(), || {
            std::fs::write(d.path().join("new.txt"), "x").unwrap();
            std::fs::write(d.path().join("noise.log"), "x").unwrap();
        });
        assert_eq!(c, ["new.txt"]);
    }

    #[test]
    fn a_deleted_untracked_file_is_reported() {
        let d = repo();
        std::fs::write(d.path().join("scratch.txt"), "x").unwrap();
        let c = changed(d.path(), || {
            std::fs::remove_file(d.path().join("scratch.txt")).unwrap();
        });
        assert_eq!(c, ["scratch.txt"]);
    }

    #[test]
    fn a_restored_dirty_file_and_a_stash_are_reported() {
        let d = repo();
        std::fs::write(d.path().join("a.txt"), "dirty\n").unwrap();
        let c = changed(d.path(), || g(d.path(), &["checkout", "--", "a.txt"]));
        assert_eq!(c, ["a.txt"]);
        std::fs::write(d.path().join("a.txt"), "dirty again\n").unwrap();
        let c = changed(d.path(), || g(d.path(), &["stash", "push", "-q"]));
        assert_eq!(c, ["a.txt"]);
    }

    #[test]
    fn a_commit_or_checkout_in_the_main_checkout_is_reported() {
        let d = repo();
        let c = changed(d.path(), || {
            g(d.path(), &["commit", "-q", "--allow-empty", "-m", "more"]);
        });
        assert_eq!(c.len(), 1);
        assert!(c[0].starts_with("HEAD"), "{c:?}");
        let c = changed(d.path(), || g(d.path(), &["checkout", "-q", "-b", "other"]));
        assert!(c.is_empty(), "same commit, branch name only: {c:?}");
    }

    #[test]
    fn skipped_paths_are_canonicalised_and_linked_worktrees_are_left_out() {
        let d = repo();
        let wt = d.path().join("wt");
        g(
            d.path(),
            &["worktree", "add", "-q", "-b", "w", wt.to_str().unwrap()],
        );
        let other = tempfile::tempdir().unwrap();
        let far = other.path().join("far");
        g(
            d.path(),
            &["worktree", "add", "-q", "-b", "f", far.to_str().unwrap()],
        );
        // A skip given through a symlink and with `..` still names the worktree.
        #[cfg(unix)]
        {
            let link = other.path().join("link");
            std::os::unix::fs::symlink(d.path(), &link).unwrap();
            let skip = link.join("wt/../wt");
            let before = snapshot(d.path(), std::slice::from_ref(&skip)).unwrap();
            std::fs::write(wt.join("x.txt"), "x").unwrap();
            std::fs::write(far.join("y.txt"), "y").unwrap();
            let after = snapshot(d.path(), &[skip]).unwrap();
            assert!(changed_since(&before, &after).is_empty());
            // Without the skip, `wt` is another linked worktree anyway.
            assert!(changed_since(&before, &snapshot(d.path(), &[]).unwrap()).is_empty());
        }
    }

    #[test]
    fn a_failing_or_hanging_git_is_an_error() {
        let plain = tempfile::tempdir().unwrap();
        assert!(snapshot(plain.path(), &[]).is_err());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let d = repo();
            let slow = d.path().join("slow-git");
            std::fs::write(&slow, "#!/bin/sh\nexec sleep 5\n").unwrap();
            std::fs::set_permissions(&slow, std::fs::Permissions::from_mode(0o755)).unwrap();
            let t = std::time::Instant::now();
            let e = snapshot_using(
                slow.to_str().unwrap(),
                d.path(),
                &[],
                Duration::from_millis(200),
            )
            .unwrap_err();
            assert!(e.to_string().contains("timed out"), "{e}");
            assert!(t.elapsed() < Duration::from_secs(4));
        }
    }
}
