//! GitHub Projects v2 reads over GraphQL (contract §3). The owner is tried as a user, then as an
//! organization. Every request is a named query sent through `net::Net`.
use super::docs;
use super::net::{Gql, Net};
use super::{Board, BoardAdapter, BoardRef, Field, Item, Opt, Scope};
use crate::repo::remote::detect_repo;
use crate::repo::{Ctx, Failed, R};
use crate::tokens::pyjson::J;

/// The GitHub board adapter.
pub struct GitHub {
    net: Net,
}

impl GitHub {
    /// With the entry and page cap of the command.
    #[must_use]
    pub fn new(entry: Option<&str>, max_pages: usize) -> Self {
        Self {
            net: Net::new("github", entry, max_pages),
        }
    }
}

fn s(v: Option<&J>) -> String {
    v.and_then(J::as_str).unwrap_or("").to_string()
}

fn board_of(node: &J, owner: &str, number: i64) -> Board {
    Board {
        platform: "github".into(),
        id: s(node.get("id")),
        key: format!("github:{owner}/{number}"),
        title: s(node.get("title")),
        url: s(node.get("url")),
        closed: matches!(node.get("closed"), Some(J::Bool(true))),
        mode: "field".into(),
        scope: Vec::new(),
    }
}

/// The member `root` of the answer's `data`, `None` when it is null.
fn root<'a>(g: &'a Gql, name: &str) -> Option<&'a J> {
    g.data.get(name).filter(|j| !matches!(j, J::Null))
}

fn partial(op: &str) -> Failed {
    Failed::err(format!(
        "{op}: GitHub returned data together with NOT_FOUND errors; refusing the partial answer"
    ))
}

fn has_next(page: &J) -> Option<String> {
    let info = page.get("pageInfo")?;
    if matches!(info.get("hasNextPage"), Some(J::Bool(true))) {
        info.get("endCursor")
            .and_then(J::as_str)
            .map(str::to_string)
    } else {
        None
    }
}

impl GitHub {
    fn owner_boards(&self, ctx: &Ctx, owner: &str) -> R<(Vec<Board>, bool)> {
        for (doc, kind) in [
            (docs::BOARDS_OF_USER, "user"),
            (docs::BOARDS_OF_ORG, "organization"),
        ] {
            let mut out = Vec::new();
            let mut after = J::Null;
            let mut found = false;
            let mut truncated = false;
            for page_no in 1..=self.net.max_pages {
                let g = self.net.gql_lookup(
                    ctx,
                    doc,
                    &[("login", J::s(owner)), ("after", after.clone())],
                )?;
                let target = root(&g, kind).and_then(|r| r.get("projectsV2"));
                if g.not_found && target.is_some() {
                    return Err(partial(doc.0));
                }
                let Some(page) = target else {
                    break;
                };
                found = true;
                if let Some(J::Arr(nodes)) = page.get("nodes") {
                    out.extend(nodes.iter().map(|n| {
                        board_of(n, owner, n.get("number").and_then(J::as_i64).unwrap_or(0))
                    }));
                }
                if let Some(c) = has_next(page) {
                    after = J::s(c);
                    truncated = page_no == self.net.max_pages;
                } else {
                    break;
                }
            }
            if found {
                return Ok((out, truncated));
            }
        }
        Err(Failed::err(format!(
            "GitHub owner '{owner}' was not found as a user or as an organization"
        )))
    }

    /// The board node and its field cell, from the user or the organization.
    fn board_node(&self, ctx: &Ctx, b: &BoardRef) -> R<(J, String)> {
        for (doc, kind) in [
            (docs::BOARD_OF_USER, "user"),
            (docs::BOARD_OF_ORG, "organization"),
        ] {
            let g = self.net.gql_lookup(
                ctx,
                doc,
                &[
                    ("login", J::s(&b.owner)),
                    ("number", J::Int(b.number)),
                    ("field", J::s(&b.field)),
                ],
            )?;
            let target = root(&g, kind)
                .and_then(|r| r.get("projectV2"))
                .filter(|p| !matches!(p, J::Null));
            if g.not_found && target.is_some() {
                return Err(partial(doc.0));
            }
            if let Some(p) = target {
                return Ok((p.clone(), kind.to_string()));
            }
        }
        Err(Failed::err(format!(
            "board {} was not found (looked up under '{}' as a user and as an organization)",
            b.key(),
            b.owner
        )))
    }

    fn field_names(&self, ctx: &Ctx, project: &str) -> R<Vec<String>> {
        let g = self
            .net
            .gql(ctx, docs::BOARD_FIELDS, &[("project", J::s(project))])?;
        let mut names = Vec::new();
        if let Some(J::Arr(nodes)) = g
            .data
            .get("node")
            .and_then(|n| n.get("fields"))
            .and_then(|f| f.get("nodes"))
        {
            for n in nodes {
                if let Some(name) = n.get("name").and_then(J::as_str) {
                    names.push(name.to_string());
                }
            }
        }
        Ok(names)
    }
}

impl BoardAdapter for GitHub {
    fn list(&self, ctx: &Ctx, scope: &Scope) -> R<(Vec<Board>, bool)> {
        self.net.check_token(ctx)?;
        let owner = if let Some(o) = scope.owner.clone().filter(|o| !o.is_empty()) {
            o
        } else {
            let r = detect_repo(ctx);
            if r.str("platform") == "github" {
                r.str("owner").to_string()
            } else {
                String::new()
            }
        };
        if owner.is_empty() {
            return Err(Failed::usage(
                "no owner: pass --owner <login> \u{2014} a Projects v2 board belongs to a user or an organization, never to a repository",
            ));
        }
        self.owner_boards(ctx, &owner)
    }

    fn fields(&self, ctx: &Ctx, b: &BoardRef) -> R<(Board, Field)> {
        self.net.check_token(ctx)?;
        let (p, _) = self.board_node(ctx, b)?;
        let board = board_of(&p, &b.owner, b.number);
        let cell = p.get("field").filter(|f| !matches!(f, J::Null));
        let options = cell.and_then(|f| match f.get("options") {
            Some(J::Arr(a)) => Some(a),
            _ => None,
        });
        let Some((cell, options)) = cell.zip(options) else {
            let names = self.field_names(ctx, &board.id)?;
            let list = if names.is_empty() {
                "(none)".to_string()
            } else {
                names.join(", ")
            };
            let what = if cell.is_some() {
                "isn't a single-select field"
            } else {
                "doesn't exist"
            };
            return Err(Failed::err(format!(
                "field '{}' {what} on {}; fields: {list}",
                b.field, board.key
            )));
        };
        let opts = options
            .iter()
            .filter(|o| o.get("name").is_some_and(J::truthy))
            .map(|o| Opt::new(s(o.get("id")), s(o.get("name"))))
            .collect();
        Ok((
            board,
            Field {
                id: s(cell.get("id")),
                name: s(cell.get("name")),
                kind: "single_select",
                options: opts,
            },
        ))
    }

    fn items(
        &self,
        ctx: &Ctx,
        resolved: &Board,
        b: &BoardRef,
        field: &Field,
    ) -> R<(Vec<Item>, bool)> {
        let mut out = Vec::new();
        let mut after = J::Null;
        let mut truncated = false;
        for page_no in 1..=self.net.max_pages {
            let g = self.net.gql(
                ctx,
                docs::BOARD_ITEMS,
                &[
                    ("project", J::s(&resolved.id)),
                    ("field", J::s(&b.field)),
                    ("after", after.clone()),
                ],
            )?;
            let Some(page) = g.data.get("node").and_then(|n| n.get("items")) else {
                return Err(Failed::err(format!("board {} was not found", resolved.key)));
            };
            if let Some(J::Arr(nodes)) = page.get("nodes") {
                for n in nodes {
                    out.push(item_of(n, field));
                }
            }
            match has_next(page) {
                Some(c) => {
                    after = J::s(c);
                    if page_no == self.net.max_pages {
                        truncated = true;
                    }
                }
                None => break,
            }
        }
        Ok((out, truncated))
    }
}

fn item_of(n: &J, field: &Field) -> Item {
    let content = n.get("content").filter(|c| !matches!(c, J::Null));
    let (kind, number, url, title, open) = match content {
        Some(c) => {
            let kind = match c.get("__typename").and_then(J::as_str) {
                Some("Issue") => "issue",
                Some("PullRequest") => "pull_request",
                _ => "draft",
            };
            (
                kind,
                c.get("number").and_then(J::as_i64),
                s(c.get("url")),
                s(c.get("title")),
                kind == "draft" || c.get("state").and_then(J::as_str) == Some("OPEN"),
            )
        }
        None => (
            "draft",
            None,
            String::new(),
            "(content not visible to this token)".to_string(),
            true,
        ),
    };
    let value = n
        .get("fieldValueByName")
        .and_then(|v| v.get("name"))
        .and_then(J::as_str)
        .map(str::to_string);
    let state = value
        .as_deref()
        .and_then(|v| field.option(v))
        .and_then(|o| o.state)
        .map(str::to_string);
    Item {
        id: s(n.get("id")),
        kind,
        number,
        url,
        title,
        open,
        value,
        state,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::pyjson::parse;

    fn field() -> Field {
        let mut f = Field {
            id: "F".into(),
            name: "Status".into(),
            kind: "single_select",
            options: vec![Opt::new("1", "Todo"), Opt::new("2", "Done")],
        };
        f.options[1].state = Some("done");
        f
    }

    #[test]
    fn an_item_maps_its_content_and_column() {
        let n = parse(r#"{"id":"I1","content":{"__typename":"PullRequest","number":7,"url":"u","title":"T","state":"MERGED"},"fieldValueByName":{"name":"Done","optionId":"2"}}"#).unwrap();
        let i = item_of(&n, &field());
        assert_eq!((i.kind, i.number, i.open), ("pull_request", Some(7), false));
        assert_eq!(i.state.as_deref(), Some("done"));
        assert_eq!(i.value.as_deref(), Some("Done"));
    }

    #[test]
    fn a_draft_and_a_hidden_item_are_drafts() {
        let d = parse(r#"{"id":"I2","content":{"__typename":"DraftIssue","title":"Idea"},"fieldValueByName":null}"#).unwrap();
        let i = item_of(&d, &field());
        assert_eq!(
            (i.kind, i.number, i.value, i.state),
            ("draft", None, None, None)
        );
        let h = parse(r#"{"id":"I3","content":null}"#).unwrap();
        assert_eq!(item_of(&h, &field()).kind, "draft");
    }

    #[test]
    fn paging_follows_the_cursor_only_while_there_is_a_next_page() {
        let more = parse(r#"{"pageInfo":{"hasNextPage":true,"endCursor":"C"}}"#).unwrap();
        let last = parse(r#"{"pageInfo":{"hasNextPage":false,"endCursor":"C"}}"#).unwrap();
        assert_eq!(has_next(&more).as_deref(), Some("C"));
        assert_eq!(has_next(&last), None);
    }
}
