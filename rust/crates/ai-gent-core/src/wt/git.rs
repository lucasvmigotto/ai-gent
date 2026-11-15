//! git as a subprocess: the same commands as `wt.py`.
use super::{DIR, Failed, Res, tail};
use std::path::Path;
use std::process::Command;

/// Turn a path into the `/` form git prints; drop a Windows verbatim prefix.
#[must_use]
pub fn norm(p: &str) -> String {
    let p = p.strip_prefix(r"\\?\").unwrap_or(p);
    p.replace('\\', "/")
}

/// Run git. A failure is an error unless `ok`, in which case it is the empty string.
pub fn git(args: &[&str], cwd: Option<&str>, ok: bool) -> Res<String> {
    let mut c = Command::new("git");
    c.args(args);
    if let Some(d) = cwd {
        c.current_dir(d);
    }
    let out = c.output().map_err(|e| {
        Failed(format!(
            "git {}: {e}",
            args.iter().take(3).copied().collect::<Vec<_>>().join(" ")
        ))
    })?;
    if !out.status.success() {
        if ok {
            return Ok(String::new());
        }
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(Failed(format!(
            "git {}: {}",
            args.iter().take(3).copied().collect::<Vec<_>>().join(" "),
            tail(err.trim(), 300)
        )));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// The main checkout, also when run from inside a worktree.
pub fn root() -> Res<String> {
    root_in(None)
}

/// The main checkout as seen from `cwd`.
pub fn root_in(cwd: Option<&str>) -> Res<String> {
    let common = norm(&git(
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        cwd,
        false,
    )?);
    Ok(common
        .rsplit_once('/')
        .map_or(String::new(), |(a, _)| a.to_string()))
}

/// The checkout you are in: a worktree reads its own specs.
pub fn here() -> Res<String> {
    Ok(norm(&git(&["rev-parse", "--show-toplevel"], None, false)?))
}

/// The chain's dev branch (`dev` or `develop`).
pub fn chain_dev(root: &str) -> Res<String> {
    let out = git(
        &[
            "for-each-ref",
            "--format=%(refname:short)",
            "refs/heads",
            "refs/remotes",
        ],
        Some(root),
        false,
    )?;
    let names: Vec<&str> = out.split_whitespace().collect();
    let found: Vec<&str> = ["dev", "develop"]
        .into_iter()
        .filter(|b| names.contains(b) || names.contains(&format!("origin/{b}").as_str()))
        .collect();
    if found.len() == 1 {
        return Ok(found[0].to_string());
    }
    Err(Failed(format!(
        "the chain's dev branch is {}; pass --base, or ask the user which one the flow uses",
        if found.is_empty() {
            "missing"
        } else {
            "ambiguous (dev and develop both exist)"
        }
    )))
}

/// Keep `.worktrees/` and the per-worktree compose override out of git, locally.
pub fn exclude(root: &str) -> Res<()> {
    let common = norm(&git(
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
        Some(root),
        false,
    )?);
    let path = Path::new(&common).join("info").join("exclude");
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).map_err(|e| Failed(format!("{}: {e}", d.display())))?;
    }
    let text = std::fs::read(&path)
        .map(|b| String::from_utf8_lossy(&b).to_string())
        .unwrap_or_default();
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let wanted = [format!("/{DIR}/"), "compose.worktree.yaml".to_string()];
    let added: Vec<String> = wanted.into_iter().filter(|x| !lines.contains(x)).collect();
    if !added.is_empty() {
        lines.push("# agents:parallel".into());
        lines.extend(added);
        std::fs::write(&path, lines.join("\n") + "\n")
            .map_err(|e| Failed(format!("{}: {e}", path.display())))?;
    }
    Ok(())
}

/// A worktree under `.worktrees/`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Worktree {
    /// Absolute path as git prints it, `/` separated.
    pub path: String,
    /// The branch, `None` when detached.
    pub branch: Option<String>,
}

fn under(path: &str, dir: &str) -> bool {
    let (p, d) = (path.to_string(), format!("{dir}/"));
    if cfg!(windows) {
        p.to_lowercase().starts_with(&d.to_lowercase())
    } else {
        p.starts_with(&d)
    }
}

/// The worktrees under `<root>/.worktrees/`, from `git worktree list --porcelain`.
pub fn worktrees(root: &str) -> Res<Vec<Worktree>> {
    let text = git(&["worktree", "list", "--porcelain"], Some(root), false)?;
    let dir = format!("{root}/{DIR}");
    let mut out = Vec::new();
    let mut cur = Worktree::default();
    for line in text.lines().chain(std::iter::once("")) {
        if line.is_empty() {
            if under(&cur.path, &dir) {
                out.push(cur.clone());
            }
            cur = Worktree::default();
            continue;
        }
        let (key, value) = line.split_once(' ').unwrap_or((line, ""));
        match key {
            "worktree" => cur.path = norm(value),
            "branch" => {
                cur.branch = Some(
                    value
                        .strip_prefix("refs/heads/")
                        .unwrap_or(value)
                        .to_string(),
                )
            }
            _ => {}
        }
    }
    Ok(out)
}

/// `path` relative to `root`.
#[must_use]
pub fn rel<'a>(path: &'a str, root: &str) -> &'a str {
    path.strip_prefix(root)
        .map_or(path, |p| p.trim_start_matches('/'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn norm_handles_windows_forms() {
        assert_eq!(norm(r"\\?\C:\Users\me\repo"), "C:/Users/me/repo");
        assert_eq!(norm("C:/Users/me"), "C:/Users/me");
    }

    #[test]
    fn rel_strips_the_root() {
        assert_eq!(rel("/r/shop/.worktrees/a", "/r/shop"), ".worktrees/a");
    }

    #[test]
    fn under_matches_a_directory_prefix() {
        assert!(under("/r/.worktrees/a", "/r/.worktrees"));
        assert!(!under("/r/.worktreesx/a", "/r/.worktrees"));
        assert!(!under("/r", "/r/.worktrees"));
    }
}
