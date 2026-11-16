//! How the board talks to a platform: every request is `api::api_call` with `write: false`, so the
//! GraphQL read check (`graphql::is_read_body`, no query string in the URL) and the host rules decide, and
//! a board read can never send a change (FR-020). Documents are constants; values travel as variables.
use crate::repo::api::{Call, Data, api_base, api_call_with_headers};
use crate::repo::entry::{check_entry, default_entry, pass_has};
use crate::repo::py::quote;
use crate::repo::{Ctx, Failed, R};
use crate::tokens::pyjson::{J, dumps, obj};
use std::collections::BTreeMap;

fn rate_limited(name: &str, hint: Option<&str>) -> Failed {
    Failed::err(format!(
        "{name} rate limit reached ({}); run it again after that (no retry here)",
        hint.unwrap_or("no reset time given")
    ))
}

/// A GraphQL answer: the `data` object, and whether the only errors were `NOT_FOUND`.
#[derive(Debug)]
pub struct Gql {
    /// The response's `data` (null when absent).
    pub data: J,
    /// Every error was `NOT_FOUND`.
    pub not_found: bool,
}

/// One platform's request path.
#[derive(Debug, Clone)]
pub struct Net {
    /// `github` or `gitlab`.
    pub platform: &'static str,
    /// Another pass entry.
    pub entry: Option<String>,
    /// Page cap.
    pub max_pages: usize,
}

fn err_type(e: &J) -> &str {
    e.get("type").and_then(J::as_str).unwrap_or("")
}

impl Net {
    /// A path for a platform.
    #[must_use]
    pub fn new(platform: &'static str, entry: Option<&str>, max_pages: usize) -> Self {
        Self {
            platform,
            entry: entry.map(str::to_string),
            max_pages: max_pages.max(1),
        }
    }

    /// The token entry must exist before anything is sent: GitHub refuses anonymous GraphQL, and a
    /// missing entry is a message with the `pass insert` line, not a request.
    ///
    /// # Errors
    /// A missing or invalid entry.
    pub fn check_token(&self, ctx: &Ctx) -> R<()> {
        let entry = match &self.entry {
            Some(e) => check_entry(ctx, e)?,
            None => default_entry(ctx, self.platform),
        };
        if pass_has(ctx, &entry) {
            Ok(())
        } else {
            Err(Failed::err(format!(
                "{entry} isn't in pass; add it in your own terminal: pass insert -m {entry} (secrets:pass)"
            )))
        }
    }

    fn call(
        &self,
        ctx: &Ctx,
        method: &str,
        path: &str,
        body: Option<&[u8]>,
        query: Vec<(String, String)>,
    ) -> R<(Data, Vec<(String, String)>)> {
        api_call_with_headers(
            ctx,
            &Call {
                platform: self.platform,
                method,
                path,
                body,
                query,
                entry: self.entry.as_deref(),
                write: false,
                paginate: false,
                max_pages: 1,
            },
        )
        .map_err(|f| self.http_error(f))
    }

    fn http_error(&self, f: Failed) -> Failed {
        let name = if self.platform == "github" {
            "GitHub"
        } else {
            "GitLab"
        };
        let low = f.msg.to_lowercase();
        match f.http_status {
            Some(429) => rate_limited(name, f.hint.as_deref()),
            Some(403) if low.contains("rate limit") => rate_limited(name, f.hint.as_deref()),
            Some(401) => Failed::err(format!(
                "{name} refused the token (HTTP 401); check it with secrets:pass{}",
                if self.platform == "github" {
                    "; boards need a classic token with read:project"
                } else {
                    ""
                }
            )),
            Some(404) => Failed::err(format!("{name}: not found ({})", f.msg)),
            _ => f,
        }
    }

    /// The absolute GraphQL endpoint (GitLab's `/api/graphql` sits beside `/api/v4`, not under it).
    fn graphql_path(&self, ctx: &Ctx) -> R<String> {
        if self.platform == "github" {
            return Ok("/graphql".to_string());
        }
        let (base, _) = api_base(ctx, "gitlab", &BTreeMap::new())?;
        let origin = base.trim_end_matches('/');
        let origin = origin.strip_suffix("/api/v4").unwrap_or(origin);
        Ok(format!("{origin}/api/graphql"))
    }

    /// Send one named query document with variables. Any GraphQL error is a failure: scope and
    /// rate-limit errors get their own messages (FR-023), and partial `data` with errors never passes.
    ///
    /// # Errors
    /// Transport and HTTP failures, and a 200 answer whose `errors` array is not empty.
    pub fn gql(&self, ctx: &Ctx, doc: (&str, &str), vars: &[(&str, J)]) -> R<Gql> {
        let g = self.gql_lookup(ctx, doc, vars)?;
        if g.not_found {
            return Err(Failed::err(format!(
                "{}: not found, or not visible to this token",
                doc.0
            )));
        }
        Ok(g)
    }

    /// Like `gql`, for the one lookup that may come back `NOT_FOUND` (an owner tried as a user, then as
    /// an organization): the answer says so in `not_found`, and the caller must treat any non-null target
    /// next to it as a failure.
    ///
    /// # Errors
    /// As `gql`, except an all-`NOT_FOUND` answer is returned.
    pub fn gql_lookup(&self, ctx: &Ctx, doc: (&str, &str), vars: &[(&str, J)]) -> R<Gql> {
        let body = dumps(
            &obj(vec![
                ("query", J::s(doc.1)),
                ("variables", obj(vars.to_vec())),
                ("operationName", J::s(doc.0)),
            ]),
            false,
            None,
        );
        let path = self.graphql_path(ctx)?;
        let (data, headers) = self.call(ctx, "POST", &path, Some(body.as_bytes()), Vec::new())?;
        let Data::Json(data) = data else {
            return Err(Failed::err(format!("{}: the answer was not JSON", doc.0)));
        };
        let errors: Vec<&J> = match data.get("errors") {
            Some(J::Arr(a)) => a.iter().collect(),
            _ => Vec::new(),
        };
        let name = if self.platform == "github" {
            "GitHub"
        } else {
            "GitLab"
        };
        if errors.iter().any(|e| err_type(e) == "INSUFFICIENT_SCOPES") {
            return Err(Failed::err(
                "GitHub boards need a classic token with `read:project` (read) or `project` (write); this token lacks the scope (spec 045 D4)",
            ));
        }
        if errors.iter().any(|e| err_type(e) == "RATE_LIMITED") {
            let hint = header(&headers, "retry-after")
                .map(|v| format!("retry after {v}s"))
                .or_else(|| {
                    header(&headers, "x-ratelimit-reset")
                        .map(|v| format!("the limit resets at epoch {v}"))
                });
            return Err(rate_limited(name, hint.as_deref()));
        }
        let not_found = !errors.is_empty() && errors.iter().all(|e| err_type(e) == "NOT_FOUND");
        if !errors.is_empty() && !not_found {
            let msg = errors
                .iter()
                .filter_map(|e| e.get("message").and_then(J::as_str))
                .take(3)
                .collect::<Vec<_>>()
                .join("; ");
            return Err(Failed::err(format!(
                "{name} GraphQL error in {}: {msg}",
                doc.0
            )));
        }
        Ok(Gql {
            data: data.get("data").cloned().unwrap_or(J::Null),
            not_found,
        })
    }

    /// One REST GET (GitLab), JSON back, with the continuation headers of the response.
    ///
    /// # Errors
    /// Transport and HTTP failures.
    pub fn get_with_headers(
        &self,
        ctx: &Ctx,
        path: &str,
        query: Vec<(String, String)>,
    ) -> R<(J, Vec<(String, String)>)> {
        match self.call(ctx, "GET", path, None, query)? {
            (Data::Json(j), h) => Ok((j, h)),
            (Data::Nothing, h) => Ok((J::Null, h)),
            (Data::Text(_), _) => Err(Failed::err(format!("GET {path}: the answer was not JSON"))),
        }
    }

    /// One REST GET (GitLab), JSON back.
    ///
    /// # Errors
    /// Transport and HTTP failures.
    pub fn get(&self, ctx: &Ctx, path: &str, query: Vec<(String, String)>) -> R<J> {
        self.get_with_headers(ctx, path, query).map(|(j, _)| j)
    }

    /// A paged REST list: the rows, and whether the page cap cut it. The next page is the one the
    /// response says exists (`x-next-page`, else a `Link` with `rel="next"`), so a full last page is not
    /// "truncated" and a short page that has a next one is followed.
    ///
    /// # Errors
    /// Transport and HTTP failures, or an answer that is not an array.
    pub fn get_pages(
        &self,
        ctx: &Ctx,
        path: &str,
        query: &[(String, String)],
    ) -> R<(Vec<J>, bool)> {
        let mut rows = Vec::new();
        let mut page = 1usize;
        for n in 1..=self.max_pages {
            let mut q: Vec<(String, String)> = query.to_vec();
            q.push(("per_page".into(), "100".into()));
            q.push(("page".into(), page.to_string()));
            let (j, h) = self.get_with_headers(ctx, path, q)?;
            let J::Arr(batch) = j else {
                return Err(Failed::err(format!("GET {path}: expected a list")));
            };
            rows.extend(batch);
            let next = header(&h, "x-next-page")
                .filter(|v| !v.is_empty())
                .and_then(|v| v.parse::<usize>().ok())
                .or_else(|| {
                    header(&h, "link")
                        .filter(|l| l.contains("rel=\"next\""))
                        .map(|_| page + 1)
                });
            match next {
                None => return Ok((rows, false)),
                Some(p) => {
                    page = p;
                    if n == self.max_pages {
                        return Ok((rows, true));
                    }
                }
            }
        }
        Ok((rows, true))
    }

    /// The web origin of the platform's API host, for URLs the API does not give.
    ///
    /// # Errors
    /// When the host cannot be determined.
    pub fn web_origin(&self, ctx: &Ctx) -> R<String> {
        let (base, _) = api_base(ctx, self.platform, &BTreeMap::new())?;
        let base = base.trim_end_matches('/');
        Ok(base.strip_suffix("/api/v4").unwrap_or(base).to_string())
    }
}

fn header<'a>(h: &'a [(String, String)], name: &str) -> Option<&'a str> {
    h.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
}

/// A project path or numeric id as one URL path segment (`group%2Fproject`).
#[must_use]
pub fn project_segment(project: &str) -> String {
    quote(project)
}
