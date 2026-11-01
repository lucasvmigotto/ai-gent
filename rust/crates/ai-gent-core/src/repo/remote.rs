//! Remote parsing and repository facts (`parse_remote`, `detect_repo`, `git_config` of `reporun.py`).
use super::Ctx;
use super::py::{unquote, urlparse};
use crate::exec;
use std::process::Command;
use std::time::Duration;

/// Facts about the repository's remote, in the order `reporun.py` sets them.
#[derive(Debug, Clone, Default)]
pub struct Remote {
    /// Ordered `(key, value)` pairs: `url`, `host`, `platform`, then owner/org/project/repo...
    pub facts: Vec<(String, String)>,
}

impl Remote {
    /// A fact by key.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&str> {
        self.facts
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// A fact, or the empty string.
    #[must_use]
    pub fn str(&self, key: &str) -> &str {
        self.get(key).unwrap_or("")
    }

    /// Set a fact (in place when it exists).
    pub fn set(&mut self, key: &str, value: impl Into<String>) {
        let value = value.into();
        if let Some(slot) = self.facts.iter_mut().find(|(k, _)| k == key) {
            slot.1 = value;
        } else {
            self.facts.push((key.to_string(), value));
        }
    }
}

/// Run `git` and return trimmed stdout, or the empty string on any failure.
#[must_use]
pub fn git(ctx: &Ctx, args: &[&str]) -> String {
    let mut c = Command::new("git");
    c.args(args);
    match exec::run(c, &ctx.env, None, Duration::from_secs(10)) {
        Ok(o) if o.code == 0 => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => String::new(),
    }
}

/// `git config --get KEY`.
#[must_use]
pub fn git_config(ctx: &Ctx, key: &str) -> String {
    git(ctx, &["config", "--get", key])
}

/// `^[\w.+-]+@([^:/]+):(.*)$` (scp-like `git@host:path`) on a URL without `://`.
fn scp_like(url: &str) -> Option<(&str, &str)> {
    if url.contains("://") {
        return None;
    }
    let at = url.find('@')?;
    let user = &url[..at];
    if user.is_empty()
        || !user
            .chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '_' | '.' | '+' | '-'))
    {
        return None;
    }
    let rest = &url[at + 1..];
    let colon = rest.find([':', '/'])?;
    if rest.as_bytes()[colon] != b':' || colon == 0 {
        return None;
    }
    Some((&rest[..colon], &rest[colon + 1..]))
}

fn pad3(parts: &[String], n: usize) -> Vec<String> {
    let mut v: Vec<String> = parts.iter().take(n).cloned().collect();
    while v.len() < n {
        v.push(String::new());
    }
    v
}

/// Platform, host and path parts from a remote URL (https, ssh or scp-like).
#[must_use]
pub fn parse_remote(url: &str, forced: &str) -> Remote {
    let url = url.trim();
    let (host, path) = if let Some((h, p)) = scp_like(url) {
        (h.to_string(), p.to_string())
    } else {
        let u = urlparse(url);
        (u.hostname, u.path)
    };
    let host = host.to_lowercase();
    let path = path.trim_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let mut parts: Vec<String> = path
        .split('/')
        .filter(|p| !p.is_empty())
        .map(unquote)
        .collect();
    let mut info = Remote::default();
    info.set("url", url);
    info.set("host", host.clone());
    info.set("platform", forced);

    if host == "dev.azure.com" || host == "ssh.dev.azure.com" || host.ends_with(".visualstudio.com")
    {
        info.set("platform", "azure-devops");
        if host == "ssh.dev.azure.com" {
            if parts.first().is_some_and(|p| p == "v3") {
                parts.remove(0);
            }
            let p = pad3(&parts, 3);
            info.set("org", p[0].clone());
            info.set("project", p[1].clone());
            info.set("repo", p[2].clone());
        } else {
            if host == "dev.azure.com" {
                let org = parts.first().cloned().unwrap_or_default();
                info.set("org", org);
                if !parts.is_empty() {
                    parts.remove(0);
                }
            } else {
                info.set("org", host.split('.').next().unwrap_or(""));
            }
            if parts.first().is_some_and(|p| p == "DefaultCollection") {
                parts.remove(0);
            }
            if let Some(i) = parts.iter().position(|p| p == "_git") {
                let repo = parts.get(i + 1).cloned().unwrap_or_default();
                info.set("repo", repo.clone());
                info.set("project", if i > 0 { parts[i - 1].clone() } else { repo });
            }
        }
        info.set("host", "dev.azure.com");
        return info;
    }
    if info.str("platform").is_empty() {
        let p = if host == "github.com" || host.ends_with(".ghe.com") {
            "github"
        } else if host == "bitbucket.org" {
            "bitbucket"
        } else if host.contains("gitlab") {
            "gitlab"
        } else if host == "codeberg.org" || host.contains("forgejo") || host.contains("gitea") {
            "forgejo"
        } else {
            ""
        };
        info.set("platform", p);
    }
    match info.str("platform") {
        "gitlab" => {
            let n = parts.len();
            info.set("namespace", parts[..n.saturating_sub(1)].join("/"));
            info.set("repo", parts.last().cloned().unwrap_or_default());
            info.set("project_path", parts.join("/"));
        }
        "bitbucket" => {
            let p = pad3(&parts, 2);
            info.set("workspace", p[0].clone());
            info.set("repo", p[1].clone());
        }
        _ if !parts.is_empty() => {
            let p = pad3(&parts, 2);
            info.set("owner", p[0].clone());
            info.set("repo", p[1].clone());
        }
        _ => {}
    }
    info
}

/// The remote of this repository (`AI_GENT_REPO_REMOTE`, default `origin`), parsed.
#[must_use]
pub fn detect_repo(ctx: &Ctx) -> Remote {
    let name = ctx
        .env
        .text("AI_GENT_REPO_REMOTE")
        .unwrap_or_else(|| "origin".to_string());
    let remote = git(ctx, &["remote", "get-url", &name]);
    if remote.is_empty() {
        let mut r = Remote::default();
        r.set("platform", "");
        r.set("url", "");
        return r;
    }
    parse_remote(&remote, &git_config(ctx, "ai-gent.platform"))
}

/// Jira's site URL: git config, `JIRA_URL`, or the entry's `url:` note.
#[must_use]
pub fn jira_url(ctx: &Ctx, notes: Option<&std::collections::BTreeMap<String, String>>) -> String {
    let v = git_config(ctx, "ai-gent.jira.url");
    let v = if v.is_empty() {
        ctx.env.text("JIRA_URL").unwrap_or_default()
    } else {
        v
    };
    let v = if v.is_empty() {
        notes
            .and_then(|n| n.get("url"))
            .cloned()
            .unwrap_or_default()
    } else {
        v
    };
    v.trim_end_matches('/').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platforms() {
        let cases: [(&str, &str, &[(&str, &str)]); 12] = [
            (
                "git@github.com:o/r.git",
                "github",
                &[("owner", "o"), ("repo", "r")],
            ),
            (
                "https://github.com/o/r",
                "github",
                &[("owner", "o"), ("repo", "r")],
            ),
            (
                "https://acme.ghe.com/o/r.git",
                "github",
                &[("owner", "o"), ("repo", "r")],
            ),
            (
                "https://org@dev.azure.com/org/My%20Proj/_git/app",
                "azure-devops",
                &[("org", "org"), ("project", "My Proj"), ("repo", "app")],
            ),
            (
                "git@ssh.dev.azure.com:v3/org/proj/app",
                "azure-devops",
                &[("org", "org"), ("project", "proj"), ("repo", "app")],
            ),
            (
                "https://org.visualstudio.com/DefaultCollection/proj/_git/app",
                "azure-devops",
                &[("org", "org"), ("project", "proj"), ("repo", "app")],
            ),
            (
                "https://dev.azure.com/org/_git/app",
                "azure-devops",
                &[("org", "org"), ("project", "app"), ("repo", "app")],
            ),
            (
                "git@gitlab.com:grp/sub/proj.git",
                "gitlab",
                &[
                    ("namespace", "grp/sub"),
                    ("repo", "proj"),
                    ("project_path", "grp/sub/proj"),
                ],
            ),
            (
                "git@bitbucket.org:ws/app.git",
                "bitbucket",
                &[("workspace", "ws"), ("repo", "app")],
            ),
            (
                "https://codeberg.org/o/r.git",
                "forgejo",
                &[("owner", "o"), ("repo", "r")],
            ),
            (
                "git@gitea.example.com:o/r.git",
                "forgejo",
                &[("owner", "o"), ("repo", "r")],
            ),
            (
                "https://git.example.com/a/b",
                "",
                &[("owner", "a"), ("repo", "b")],
            ),
        ];
        for (url, platform, facts) in cases {
            let info = parse_remote(url, "");
            assert_eq!(info.str("platform"), platform, "{url}");
            for (k, v) in facts {
                assert_eq!(info.str(k), *v, "{url} {k}");
            }
        }
        assert_eq!(
            parse_remote("https://code.example.com/a/b", "gitlab").str("project_path"),
            "a/b"
        );
    }
}
