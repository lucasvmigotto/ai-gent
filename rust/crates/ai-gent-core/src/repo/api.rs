//! The REST path: base URLs, auth headers, placeholders, write detection, pagination.
use super::entry::{check_entry, default_entry, pass_has, read_entry};
use super::platforms::{has_read_posts, is_read_post};
use super::py::{quote, urlencode, urlparse};
use super::remote::{Remote, detect_repo, git_config, jira_url};
use super::{Ctx, Failed, R, note};
use crate::passstore::{self, Secret, redact};
use crate::tokens::pyjson::{J, dumps, parse};
use std::collections::BTreeMap;

/// Base URL and the repository facts used to fill `{placeholders}`.
///
/// # Errors
/// When the platform's host or organization cannot be determined.
pub fn api_base(
    ctx: &Ctx,
    platform: &str,
    notes: &BTreeMap<String, String>,
) -> R<(String, Vec<(String, String)>)> {
    // Tests only: a local stub server. A credential is never sent to a non-loopback override.
    let override_url = ctx.env.text("REPORUN_TEST_BASE").filter(|u| {
        let h = urlparse(u).hostname;
        matches!(h.as_str(), "127.0.0.1" | "localhost" | "::1")
    });
    let repo: Remote = detect_repo(ctx);
    let mut facts: Vec<(String, String)> = repo.facts.clone();
    if platform == "jira" {
        let url = jira_url(ctx, Some(notes));
        if url.is_empty() && override_url.is_none() {
            return Err(Failed::err(
                "no Jira URL: git config --global ai-gent.jira.url https://<site>.atlassian.net (or a `url:` line in the token entry)",
            ));
        }
        return Ok((override_url.unwrap_or(url), facts));
    }
    if !repo.str("platform").is_empty() && repo.str("platform") != platform {
        facts.clear();
    }
    if let Some(o) = override_url {
        return Ok((o, facts));
    }
    let host = if facts.is_empty() {
        String::new()
    } else {
        repo.str("host").to_string()
    };
    let base = match platform {
        "github" => {
            if host.is_empty() || host == "github.com" {
                "https://api.github.com".to_string()
            } else if host.ends_with(".ghe.com") {
                format!("https://api.{host}")
            } else {
                format!("https://{host}/api/v3")
            }
        }
        "gitlab" => format!(
            "https://{}/api/v4",
            if host.is_empty() { "gitlab.com" } else { &host }
        ),
        "forgejo" => {
            if host.is_empty() {
                return Err(Failed::err(
                    "no Forgejo host: the remote isn't on one; set git config ai-gent.platform forgejo with a remote on the Forgejo or Gitea instance",
                ));
            }
            format!("https://{host}/api/v1")
        }
        "azure-devops" => {
            let from_facts = facts
                .iter()
                .find(|(k, _)| k == "org")
                .map(|(_, v)| v.clone())
                .unwrap_or_default();
            let org = [
                from_facts,
                notes.get("org").cloned().unwrap_or_default(),
                git_config(ctx, "ai-gent.azure-devops.org"),
            ]
            .into_iter()
            .find(|s| !s.is_empty())
            .unwrap_or_default();
            if org.is_empty() {
                return Err(Failed::err(
                    "no Azure DevOps organization: not in the remote; set git config ai-gent.azure-devops.org <org>",
                ));
            }
            format!("https://dev.azure.com/{org}")
        }
        _ => "https://api.bitbucket.org/2.0".to_string(),
    };
    Ok((base, facts))
}

/// The `Authorization` header value for a platform. Registers any derived form for redaction.
#[must_use]
pub fn auth_header(platform: &str, secret: &Secret, notes: &BTreeMap<String, String>) -> String {
    let user = ["email", "username", "user"]
        .iter()
        .find_map(|k| notes.get(*k).filter(|v| !v.is_empty()));
    let secret = secret.expose();
    let basic = |s: String| {
        let enc = crate::b64::encode(s.as_bytes());
        passstore::redact::register_derived(&enc);
        format!("Basic {enc}")
    };
    if platform == "azure-devops" {
        return basic(format!(":{secret}"));
    }
    if platform == "forgejo" {
        return format!("token {secret}");
    }
    if let (Some(u), "bitbucket" | "jira") = (user, platform) {
        return basic(format!("{u}:{secret}"));
    }
    format!("Bearer {secret}")
}

/// Fill `{owner}`-style placeholders from the facts (percent-encoded: GitLab wants `group%2Fproject`).
///
/// # Errors
/// When a placeholder is unknown for this repository.
pub fn fill(path: &str, facts: &[(String, String)]) -> R<String> {
    let mut out = String::new();
    let mut rest = path;
    while let Some(i) = rest.find('{') {
        out.push_str(&rest[..i]);
        let tail = &rest[i + 1..];
        let n = tail
            .chars()
            .take_while(|c| c.is_ascii_lowercase() || *c == '_')
            .count();
        if n > 0 && tail[n..].starts_with('}') {
            let key = &tail[..n];
            match facts.iter().find(|(k, v)| k == key && !v.is_empty()) {
                Some((_, v)) => out.push_str(&quote(v)),
                None => {
                    return Err(Failed::err(format!(
                        "{{{key}}} isn't known for this repository; write it out"
                    )));
                }
            }
            rest = &tail[n + 1..];
        } else {
            out.push('{');
            rest = tail;
        }
    }
    out.push_str(rest);
    Ok(out)
}

/// Whether an API request changes something.
#[must_use]
pub fn api_is_write(platform: &str, method: &str, path: &str, body: Option<&[u8]>) -> bool {
    if method == "GET" || method == "HEAD" {
        return false;
    }
    if method == "POST" && has_read_posts(platform) && is_read_post(platform, path) {
        return matches!(platform, "github" | "gitlab")
            && body.is_some_and(|b| b.windows(8).any(|w| w == b"mutation"));
    }
    true
}

fn next_link(platform: &str, link: &str, data: &J) -> String {
    // <url>; rel="next"
    for part in link.split(',') {
        if let Some(rest) = part.trim().strip_prefix('<') {
            if let Some((url, attrs)) = rest.split_once('>') {
                let a: String = attrs.split_whitespace().collect::<Vec<_>>().join("");
                if a.contains("rel=\"next\"") {
                    return url.to_string();
                }
            }
        }
    }
    if platform == "bitbucket" {
        if let Some(J::Str(n)) = data.get("next") {
            if !n.is_empty() {
                return n.clone();
            }
        }
    }
    String::new()
}

/// Options of one API call.
#[derive(Debug, Clone)]
pub struct Call<'a> {
    /// Platform name.
    pub platform: &'a str,
    /// HTTP method.
    pub method: &'a str,
    /// Path relative to the base, or an absolute URL on the platform's host.
    pub path: &'a str,
    /// Request body.
    pub body: Option<&'a [u8]>,
    /// Query pairs.
    pub query: Vec<(String, String)>,
    /// Another entry under the prefix.
    pub entry: Option<&'a str>,
    /// The user asked for this specific change.
    pub write: bool,
    /// Follow next-page links.
    pub paginate: bool,
    /// Page limit.
    pub max_pages: usize,
}

/// The result of an API call: JSON when it parses, else the text; `None` for an empty body.
#[derive(Debug, Clone)]
pub enum Data {
    /// Empty body.
    Nothing,
    /// A parsed JSON document.
    Json(J),
    /// Text that is not JSON.
    Text(String),
}

fn is_workitem_patch(platform: &str, method: &str, url: &str) -> bool {
    if platform != "azure-devops" {
        return false;
    }
    let lower = url.to_lowercase();
    let decoded = super::py::unquote(url).to_lowercase();
    (method == "PATCH" && lower.contains("/wit/workitems"))
        || (method == "POST" && decoded.contains("/wit/workitems/$"))
}

/// One REST request (or a paginated series) with the token from pass.
///
/// # Errors
/// Refusals (a write without `--write`, a foreign host), a missing entry, transport errors and HTTP
/// errors (status 400 and above, with the redacted body).
pub fn api_call(ctx: &Ctx, c: &Call<'_>) -> R<Data> {
    let method = c.method.to_uppercase();
    if !["GET", "HEAD", "POST", "PUT", "PATCH", "DELETE"].contains(&method.as_str()) {
        return Err(Failed::usage(format!("method {method} isn't supported")));
    }
    if api_is_write(c.platform, &method, c.path, c.body) && !c.write {
        return Err(Failed::refused(format!(
            "{method} {} changes something on {}; only when the user asked for this specific change, re-run with --write",
            c.path, c.platform
        )));
    }
    let entry = match c.entry {
        Some(e) => check_entry(ctx, e)?,
        None => default_entry(ctx, c.platform),
    };
    let mut secret: Option<Secret> = None;
    let mut notes = BTreeMap::new();
    if pass_has(ctx, &entry) {
        let (s, n) = read_entry(ctx, &entry)?;
        secret = Some(s);
        notes = n;
    } else if c.platform != "github" || c.entry.is_some() {
        return Err(Failed::err(format!(
            "{entry} isn't in pass; add it in your own terminal: pass insert -m {entry}"
        )));
    }
    let (base, facts) = api_base(ctx, c.platform, &notes)?;
    let path = fill(c.path, &facts)?;
    let mut url = if path.starts_with("http://") || path.starts_with("https://") {
        let (pn, bn) = (urlparse(&path), urlparse(&base));
        if pn.netloc != bn.netloc || pn.scheme != bn.scheme {
            return Err(Failed::refused(format!(
                "{} isn't {}'s API host; the token goes nowhere else",
                pn.netloc, c.platform
            )));
        }
        path.clone()
    } else {
        format!(
            "{}/{}",
            base.trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    };
    let mut query = c.query.clone();
    if c.platform == "azure-devops"
        && !url.contains("api-version")
        && !query.iter().any(|(k, _)| k == "api-version")
    {
        query.push(("api-version".into(), "7.1".into()));
    }
    if !query.is_empty() {
        url.push(if url.contains('?') { '&' } else { '?' });
        url.push_str(&urlencode(&query));
    }

    let mut headers: Vec<(String, String)> = vec![
        ("Accept".into(), "application/json".into()),
        ("User-Agent".into(), "ai-gent-repo".into()),
    ];
    if c.platform == "github" {
        headers[0].1 = "application/vnd.github+json".into();
        headers.push(("X-GitHub-Api-Version".into(), "2022-11-28".into()));
    }
    if let Some(s) = &secret {
        headers.push(("Authorization".into(), auth_header(c.platform, s, &notes)));
    }
    if c.body.is_some() {
        let ct = if is_workitem_patch(c.platform, &method, &url) {
            "application/json-patch+json"
        } else {
            "application/json"
        };
        headers.push(("Content-Type".into(), ct.into()));
    }
    note(&format!(
        "{method} {} \u{b7} token from {}",
        redact(&url),
        if secret.is_some() {
            format!("pass ({entry})")
        } else {
            "none (public)".to_string()
        }
    ));

    let mut pages = 0;
    let mut results: Vec<J> = Vec::new();
    let mut current = url;
    while !current.is_empty() && pages < c.max_pages {
        let resp = super::http::request(ctx, &method, &current, &headers, c.body)?;
        pages += 1;
        let text = redact(&String::from_utf8_lossy(&resp.body));
        if resp.status >= 400 {
            let cut: String = text.chars().take(4000).collect();
            let mut f = Failed::err(format!("HTTP {}: {cut}", resp.status));
            f.http_status = Some(resp.status);
            return Err(f);
        }
        let data = if text.trim().is_empty() {
            None
        } else {
            Some(parse(&text).unwrap_or(J::Str(text.clone())))
        };
        if !c.paginate {
            return Ok(match data {
                None => Data::Nothing,
                Some(J::Str(s)) => Data::Text(s),
                Some(j) => Data::Json(j),
            });
        }
        let data = data.unwrap_or(J::Null);
        let nxt = next_link(c.platform, resp.header("link").unwrap_or(""), &data);
        results.push(data);
        if !nxt.is_empty() && urlparse(&nxt).netloc != urlparse(&current).netloc {
            break;
        }
        current = nxt;
    }
    let mut merged: Vec<J> = Vec::new();
    for page in results {
        match page {
            J::Arr(a) => merged.extend(a),
            J::Obj(_) if matches!(page.get("values"), Some(J::Arr(_))) => {
                if let Some(J::Arr(a)) = page.get("values") {
                    merged.extend(a.clone());
                }
            }
            J::Obj(_) if matches!(page.get("value"), Some(J::Arr(_))) => {
                if let Some(J::Arr(a)) = page.get("value") {
                    merged.extend(a.clone());
                }
            }
            other => merged.push(other),
        }
    }
    if !current.is_empty() && pages >= c.max_pages {
        note(&format!("stopped after {pages} pages (--max-pages)"));
    }
    Ok(Data::Json(J::Arr(merged)))
}

/// Pretty-print API data the way `cmd_api` does.
#[must_use]
pub fn render(data: &Data) -> Option<String> {
    match data {
        Data::Nothing => None,
        Data::Text(t) => Some(t.clone()),
        Data::Json(j) => Some(dumps(j, false, Some(2))),
    }
}
