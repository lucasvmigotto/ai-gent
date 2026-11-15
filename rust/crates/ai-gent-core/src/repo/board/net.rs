//! How the board talks to a platform: every request is `api::api_call` with `write: false`, so the
//! GraphQL read check (`graphql::is_read_body`, no query string in the URL) and the host rules decide, and
//! a board read can never send a change (FR-020). Documents are constants; values travel as variables.
use crate::repo::api::{Call, Data, api_base, api_call};
use crate::repo::entry::{check_entry, default_entry, pass_has};
use crate::repo::py::quote;
use crate::repo::{Ctx, Failed, R};
use crate::tokens::pyjson::{J, dumps, obj};
use std::collections::BTreeMap;

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
    ) -> R<Data> {
        api_call(
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
            Some(429) => Failed::err(format!(
                "{name} rate limit reached; wait for the limit to reset and run it again (no retry here)"
            )),
            Some(403) if low.contains("rate limit") => Failed::err(format!(
                "{name} rate limit reached; wait for the limit to reset and run it again (no retry here)"
            )),
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

    /// Send one named query document with variables.
    ///
    /// # Errors
    /// Transport and HTTP failures, and a 200 answer whose `errors` array reports anything but
    /// `NOT_FOUND` (scope and rate-limit errors get their own messages, FR-023).
    pub fn gql(&self, ctx: &Ctx, doc: (&str, &str), vars: &[(&str, J)]) -> R<Gql> {
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
        let Data::Json(data) = self.call(ctx, "POST", &path, Some(body.as_bytes()), Vec::new())?
        else {
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
            return Err(Failed::err(format!(
                "{name} rate limit reached; wait for the limit to reset and run it again (no retry here)"
            )));
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

    /// One REST GET (GitLab), JSON back.
    ///
    /// # Errors
    /// Transport and HTTP failures.
    pub fn get(&self, ctx: &Ctx, path: &str, query: Vec<(String, String)>) -> R<J> {
        match self.call(ctx, "GET", path, None, query)? {
            Data::Json(j) => Ok(j),
            Data::Nothing => Ok(J::Null),
            Data::Text(_) => Err(Failed::err(format!("GET {path}: the answer was not JSON"))),
        }
    }

    /// A paged REST list by `page` and `per_page=100`: the rows, and whether the page cap cut it. The
    /// pages are asked for one by one so the cap is exact and no response header is needed.
    ///
    /// # Errors
    /// Transport and HTTP failures, or an answer that is not an array.
    pub fn get_pages(&self, ctx: &Ctx, path: &str, query: &[(&str, &str)]) -> R<(Vec<J>, bool)> {
        let mut rows = Vec::new();
        for page in 1..=self.max_pages {
            let mut q: Vec<(String, String)> = query
                .iter()
                .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
                .collect();
            q.push(("per_page".into(), "100".into()));
            q.push(("page".into(), page.to_string()));
            let J::Arr(batch) = self.get(ctx, path, q)? else {
                return Err(Failed::err(format!("GET {path}: expected a list")));
            };
            let full = batch.len() >= 100;
            rows.extend(batch);
            if !full {
                return Ok((rows, false));
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

/// A project path or numeric id as one URL path segment (`group%2Fproject`).
#[must_use]
pub fn project_segment(project: &str) -> String {
    quote(project)
}
