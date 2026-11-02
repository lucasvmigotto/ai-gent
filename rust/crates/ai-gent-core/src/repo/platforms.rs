//! The platform table and the read/write vocabularies (data copied from `reporun.py` lines 62-90).

/// A platform: its CLI, default token entry and the environment variables the CLI reads.
#[derive(Debug, Clone, Copy)]
pub struct Platform {
    /// Name used on the command line.
    pub name: &'static str,
    /// CLI program, when there is one.
    pub cli: Option<&'static str>,
    /// Default entry under the prefix.
    pub entry: &'static str,
    /// Variables that carry the token to the CLI.
    pub env: &'static [&'static str],
}

/// The platforms, in `reporun.py` order.
pub const PLATFORMS: [Platform; 6] = [
    Platform {
        name: "github",
        cli: Some("gh"),
        entry: "github/pat",
        env: &["GH_TOKEN"],
    },
    Platform {
        name: "azure-devops",
        cli: Some("az"),
        entry: "azure-devops/pat",
        env: &["AZURE_DEVOPS_EXT_PAT"],
    },
    Platform {
        name: "gitlab",
        cli: Some("glab"),
        entry: "gitlab/pat",
        env: &["GITLAB_TOKEN"],
    },
    Platform {
        name: "bitbucket",
        cli: None,
        entry: "bitbucket/token",
        env: &[],
    },
    Platform {
        name: "forgejo",
        cli: None,
        entry: "forgejo/pat",
        env: &[],
    },
    Platform {
        name: "jira",
        cli: Some("jira"),
        entry: "jira/token",
        env: &["JIRA_API_TOKEN"],
    },
];

/// The platform by name.
#[must_use]
pub fn platform(name: &str) -> Option<&'static Platform> {
    PLATFORMS.iter().find(|p| p.name == name)
}

/// CLI words that only read. Anything else is a write and needs `--write`.
pub const READ_VERBS: [&str; 22] = [
    "list",
    "ls",
    "view",
    "show",
    "status",
    "checks",
    "diff",
    "search",
    "query",
    "get",
    "watch",
    "download",
    "issues",
    "comments",
    "me",
    "serverinfo",
    "help",
    "--help",
    "version",
    "--version",
    "field-list",
    "item-list",
];

/// Whether a word is a read verb.
#[must_use]
pub fn is_read_verb(w: &str) -> bool {
    READ_VERBS.contains(&w)
}

/// Subcommands that print a token, run arbitrary code or log auth headers.
#[must_use]
pub fn blocked_cli(cli: &str) -> &'static [&'static str] {
    match cli {
        "gh" => &[
            "auth",
            "alias",
            "extension",
            "ext",
            "secret",
            "codespace",
            "config",
        ],
        "glab" => &["auth", "alias", "config", "variable", "token"],
        "az" => &["login", "logout", "account", "extension", "config"],
        "jira" => &["init"],
        _ => &[],
    }
}

/// Flags that can log the auth header.
pub const BLOCKED_FLAGS: [&str; 2] = ["--debug", "--show-token"];

/// The path of a request target (`urlparse(path).path`).
#[must_use]
pub fn url_path(target: &str) -> String {
    super::py::urlparse(target).path
}

/// POSTs that only read (query languages over POST): the path matches the platform's search pattern.
#[must_use]
pub fn is_read_post(platform: &str, path: &str) -> bool {
    let p = url_path(path);
    match platform {
        "github" => p == "/graphql",
        "gitlab" => p == "/api/graphql" || p == "/graphql",
        "azure-devops" => p.find("/_apis/wit/wiql").is_some_and(|i| {
            let tail = &p[i + "/_apis/wit/wiql".len()..];
            tail.is_empty() || tail.starts_with('/')
        }),
        "jira" => jira_search(&p),
        _ => false,
    }
}

fn jira_search(p: &str) -> bool {
    if let Some(r) = p.strip_prefix("/rest/api/") {
        let d = r.chars().take_while(char::is_ascii_digit).count();
        if d > 0 {
            let tail = &r[d..];
            return tail == "/search" || tail == "/search/jql";
        }
    }
    if let Some(r) = p.strip_prefix("/rest/agile/") {
        let v = r
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .count();
        if v > 0 {
            if let Some(tail) = r[v..].strip_prefix('/') {
                return tail.ends_with("/search");
            }
        }
    }
    false
}

/// Whether a platform has a read-POST pattern at all.
#[must_use]
pub fn has_read_posts(platform: &str) -> bool {
    !matches!(platform, "bitbucket" | "forgejo")
}
