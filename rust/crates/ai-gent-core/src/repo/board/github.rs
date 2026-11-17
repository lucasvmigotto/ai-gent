//! GitHub Projects v2 reads over GraphQL (contract §3). The owner is tried as a user, then as an
//! organization. Every request is a named query sent through `net::Net`.
use super::docs;
use super::net::{Gql, Net};
use super::{Board, BoardAdapter, BoardRef, Field, Item, ItemRef, Located, Opt, Req, Scope};
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
    fn net(&self) -> &Net {
        &self.net
    }

    fn locate(
        &self,
        ctx: &Ctx,
        board: &Board,
        _bref: &BoardRef,
        field: &Field,
        item: &ItemRef,
    ) -> R<Located> {
        self.net.check_token(ctx)?;
        let url = if item.url.is_empty() {
            let r = detect_repo(ctx);
            if r.str("platform") != "github"
                || r.str("owner").is_empty()
                || r.str("repo").is_empty()
            {
                return Err(Failed::usage(
                    "#N needs a GitHub remote; pass the issue or pull request URL instead",
                ));
            }
            format!(
                "https://github.com/{}/{}/issues/{}",
                r.str("owner"),
                r.str("repo"),
                item.number
            )
        } else {
            item.url.clone()
        };
        let mut after = J::Null;
        for _ in 0..self.net.max_pages {
            let g = self.net.gql(
                ctx,
                docs::ITEM_ON_BOARD,
                &[
                    ("url", J::s(&url)),
                    ("field", J::s(&field.name)),
                    ("after", after.clone()),
                ],
            )?;
            let Some(res) = root(&g, "resource") else {
                return Err(Failed::err(format!("{url}: no such issue or pull request")));
            };
            let kind = match res.get("__typename").and_then(J::as_str) {
                Some("Issue") => "issue",
                Some("PullRequest") => "pull_request",
                _ => {
                    return Err(Failed::err(format!(
                        "{url} is not an issue or a pull request (drafts are never edited by board)"
                    )));
                }
            };
            let mut loc = Located {
                content_id: s(res.get("id")),
                kind,
                number: res.get("number").and_then(J::as_i64),
                url: s(res.get("url")),
                title: s(res.get("title")),
                why_not: format!("not on {}", board.key),
                ..Located::default()
            };
            let items = res.get("projectItems");
            if let Some(J::Arr(nodes)) = items.and_then(|i| i.get("nodes")) {
                if let Some(n) = nodes
                    .iter()
                    .find(|n| s(n.get("project").and_then(|p| p.get("id"))) == board.id)
                {
                    loc.on_board = true;
                    loc.id = s(n.get("id"));
                    loc.from = n
                        .get("fieldValueByName")
                        .and_then(|v| v.get("name"))
                        .and_then(J::as_str)
                        .map(str::to_string);
                    return Ok(loc);
                }
            }
            match items.and_then(has_next) {
                Some(c) => after = J::s(c),
                None => return Ok(loc),
            }
        }
        Err(Failed::err(format!(
            "{url}: the item's board list is longer than --max-pages; raise it"
        )))
    }

    fn plan_set(&self, board: &Board, field: &Field, loc: &Located, to: &Opt) -> R<Vec<Req>> {
        if !loc.on_board {
            return Err(Failed::err(format!(
                "{} is not on {}; add it first with ai-gent repo board add",
                loc.url, board.key
            )));
        }
        if loc.from.as_deref() == Some(to.name.as_str()) {
            return Ok(Vec::new());
        }
        Ok(vec![Req::gql(
            docs::BOARD_SET,
            vec![
                ("project", J::s(&board.id)),
                ("item", J::s(&loc.id)),
                ("field", J::s(&field.id)),
                ("option", J::s(&to.id)),
            ],
        )])
    }

    fn plan_add(
        &self,
        board: &Board,
        field: &Field,
        loc: &Located,
        to: Option<&Opt>,
    ) -> R<Vec<Req>> {
        if loc.on_board {
            return to.map_or_else(|| Ok(Vec::new()), |t| self.plan_set(board, field, loc, t));
        }
        let mut reqs = vec![Req::gql(
            docs::BOARD_ADD,
            vec![
                ("project", J::s(&board.id)),
                ("content", J::s(&loc.content_id)),
            ],
        )];
        if let Some(t) = to {
            let mut set = Req::gql(
                docs::BOARD_SET,
                vec![
                    ("project", J::s(&board.id)),
                    ("item", J::Null),
                    ("field", J::s(&field.id)),
                    ("option", J::s(&t.id)),
                ],
            );
            set.from_prev = Some(("item", &["addProjectV2ItemById", "item", "id"]));
            reqs.push(set);
        }
        Ok(reqs)
    }

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
        let (mut p, _) = self.board_node(ctx, b)?;
        let board = board_of(&p, &b.owner, b.number);
        let mut known: Option<Vec<String>> = None;
        if p.get("field").is_none_or(|f| matches!(f, J::Null)) {
            // The field is looked up by exact name; the tool has always matched it case-insensitively
            // (`--field status`), so a missing one is retried under the board's own spelling.
            let names = known.insert(self.field_names(ctx, &board.id)?);
            let mut hits = names
                .iter()
                .filter(|n| n.eq_ignore_ascii_case(&b.field) && **n != b.field);
            if let (Some(exact), None) = (hits.next(), hits.next()) {
                let mut again = b.clone();
                again.field.clone_from(exact);
                p = self.board_node(ctx, &again)?.0;
            }
        }
        let cell = p.get("field").filter(|f| !matches!(f, J::Null));
        let options = cell.and_then(|f| match f.get("options") {
            Some(J::Arr(a)) => Some(a),
            _ => None,
        });
        let Some((cell, options)) = cell.zip(options) else {
            let names = match known {
                Some(n) => n,
                None => self.field_names(ctx, &board.id)?,
            };
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
        _b: &BoardRef,
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
                    ("field", J::s(&field.name)),
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
