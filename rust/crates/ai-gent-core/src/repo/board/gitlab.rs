//! GitLab reads. Label mode (the default, contract §4.1) goes through the REST boards and issues
//! APIs: a list is a label, the built-in Open and Closed lists are added, and an item's state is the
//! mapped label it carries. Status mode (contract §4.2, opt-in) reads the work item Status field
//! over GraphQL; its document and field names are not verified and it is fixtures-only until G30-21.
use super::docs;
use super::net::{Net, project_segment};
use super::{Board, BoardAdapter, BoardRef, Field, Item, Opt, Scope};
use crate::repo::remote::detect_repo;
use crate::repo::{Ctx, Failed, R};
use crate::tokens::pyjson::J;

/// The GitLab board adapter.
pub struct GitLab {
    net: Net,
    status: bool,
}

impl GitLab {
    /// With the entry, page cap and mode of the command.
    #[must_use]
    pub fn new(entry: Option<&str>, max_pages: usize, status: bool) -> Self {
        Self {
            net: Net::new("gitlab", entry, max_pages),
            status,
        }
    }

    fn board(
        &self,
        ctx: &Ctx,
        project: &str,
        id: &str,
        title: &str,
        mode: &str,
        scope: Vec<String>,
    ) -> R<Board> {
        let origin = self.net.web_origin(ctx)?;
        Ok(Board {
            platform: "gitlab".into(),
            id: id.to_string(),
            key: format!("gitlab:{project}/{id}"),
            title: title.to_string(),
            url: format!("{origin}/{project}/-/boards/{id}"),
            closed: false,
            mode: mode.to_string(),
            scope,
        })
    }
}

fn text(v: Option<&J>) -> String {
    match v {
        Some(J::Str(s)) => s.clone(),
        Some(J::Int(i)) => i.to_string(),
        _ => String::new(),
    }
}

fn non_null<'a>(j: &'a J, key: &str) -> Option<&'a J> {
    j.get(key).filter(|v| !matches!(v, J::Null))
}

/// One board list as an option: a label list keeps its list id; one filtered by assignee, milestone or
/// iteration is shown with a prefixed id and never holds a state.
fn list_option(l: &J) -> Option<Opt> {
    if let Some(lab) = non_null(l, "label") {
        return Some(Opt::new(text(l.get("id")), text(lab.get("name"))));
    }
    let other = |kind: &str, name: String, id: String| {
        let mut o = Opt::new(format!("{kind}:{id}"), format!("{kind}: {name}"));
        o.mappable = false;
        o
    };
    if let Some(a) = non_null(l, "assignee") {
        let u = text(a.get("username"));
        return Some(other("assignee", u.clone(), u));
    }
    if let Some(m) = non_null(l, "milestone") {
        let t = text(m.get("title"));
        return Some(other("milestone", t.clone(), t));
    }
    if let Some(i) = non_null(l, "iteration") {
        let id = text(i.get("id"));
        let t = text(i.get("title"));
        let name = if t.is_empty() { id.clone() } else { t };
        return Some(other("iteration", name, id));
    }
    None
}

fn builtin(id: &str, name: &str) -> Opt {
    let mut o = Opt::new(id, name);
    o.builtin = true;
    o
}

impl GitLab {
    fn label_fields(&self, ctx: &Ctx, b: &BoardRef) -> R<(Board, Field)> {
        let seg = project_segment(&b.project);
        let board = self.net.get(
            ctx,
            &format!("/projects/{seg}/boards/{}", b.board_id),
            Vec::new(),
        )?;
        let lists = match board.get("lists") {
            Some(J::Arr(a)) => a.clone(),
            _ => {
                self.net
                    .get_pages(
                        ctx,
                        &format!("/projects/{seg}/boards/{}/lists", b.board_id),
                        &[],
                    )?
                    .0
            }
        };
        let mut lists: Vec<&J> = lists.iter().collect();
        lists.sort_by_key(|l| l.get("position").and_then(J::as_i64).unwrap_or(i64::MAX));
        let mut options = vec![builtin("open", "Open")];
        options.extend(lists.iter().filter_map(|l| list_option(l)));
        options.push(builtin("closed", "Closed"));
        let title = text(board.get("name"));
        Ok((
            self.board(
                ctx,
                &b.project,
                &b.board_id.to_string(),
                &title,
                "labels",
                board_scope(&board),
            )?,
            Field {
                id: "lists".into(),
                name: "Lists".into(),
                kind: "lists",
                options,
            },
        ))
    }

    fn status_fields(&self, ctx: &Ctx, b: &BoardRef) -> R<(Board, Field)> {
        let g = self.net.gql(
            ctx,
            docs::GITLAB_STATUS_FIELDS,
            &[("fullPath", J::s(&b.project))],
        )?;
        let Some(project) = g.data.get("project").filter(|p| !matches!(p, J::Null)) else {
            return Err(Failed::err(format!(
                "GitLab project '{}' was not found",
                b.project
            )));
        };
        let mut options = Vec::new();
        if let Some(J::Arr(types)) = project.get("workItemTypes").and_then(|t| t.get("nodes")) {
            for t in types {
                let Some(J::Arr(defs)) = t.get("widgetDefinitions") else {
                    continue;
                };
                for d in defs {
                    if let Some(J::Arr(st)) = d.get("allowedStatuses") {
                        for s in st {
                            options.push(Opt::new(text(s.get("id")), text(s.get("name"))));
                        }
                    }
                }
                if !options.is_empty() {
                    break;
                }
            }
        }
        if options.is_empty() {
            return Err(Failed::err(format!(
                "GitLab status mode needs the work item Status field, which this project doesn't expose (Premium and Ultimate only); use mode: labels for {}",
                b.project
            )));
        }
        Ok((
            self.board(
                ctx,
                &b.project,
                &b.board_id.to_string(),
                &b.project,
                "status",
                Vec::new(),
            )?,
            Field {
                id: "status".into(),
                name: "Status".into(),
                kind: "status",
                options,
            },
        ))
    }

    fn status_items(&self, ctx: &Ctx, b: &BoardRef, field: &Field) -> R<(Vec<Item>, bool)> {
        let mut out = Vec::new();
        let mut after = J::Null;
        let mut truncated = false;
        for page_no in 1..=self.net.max_pages {
            let g = self.net.gql(
                ctx,
                docs::GITLAB_STATUS_ITEMS,
                &[("fullPath", J::s(&b.project)), ("after", after.clone())],
            )?;
            let Some(page) = g
                .data
                .get("project")
                .and_then(|p| p.get("workItems"))
                .filter(|p| !matches!(p, J::Null))
            else {
                return Err(Failed::err(format!(
                    "GitLab project '{}' was not found",
                    b.project
                )));
            };
            if let Some(J::Arr(nodes)) = page.get("nodes") {
                for n in nodes {
                    let value = match n.get("widgets") {
                        Some(J::Arr(ws)) => ws
                            .iter()
                            .find_map(|w| non_null(w, "status").and_then(|s| s.get("name")))
                            .and_then(J::as_str)
                            .map(str::to_string),
                        _ => None,
                    };
                    let state = value
                        .as_deref()
                        .and_then(|v| field.option(v))
                        .and_then(|o| o.state)
                        .map(str::to_string);
                    out.push(Item {
                        id: text(n.get("id")),
                        kind: "issue",
                        number: text(n.get("iid")).parse().ok(),
                        url: text(n.get("webUrl")),
                        title: text(n.get("title")),
                        open: text(n.get("state")).eq_ignore_ascii_case("opened")
                            || text(n.get("state")).eq_ignore_ascii_case("open"),
                        value,
                        state,
                    });
                }
            }
            let info = page.get("pageInfo");
            if matches!(info.and_then(|i| i.get("hasNextPage")), Some(J::Bool(true))) {
                after = J::opt_s(info.and_then(|i| i.get("endCursor")).and_then(J::as_str));
                if page_no == self.net.max_pages {
                    truncated = true;
                }
            } else {
                break;
            }
        }
        Ok((out, truncated))
    }

    fn label_items(
        &self,
        ctx: &Ctx,
        resolved: &Board,
        b: &BoardRef,
        field: &Field,
    ) -> R<(Vec<Item>, bool)> {
        let path = format!("/projects/{}/issues", project_segment(&b.project));
        let (scope_labels, scope_pairs) = scope_query(&resolved.scope);
        let label_lists: Vec<&str> = field
            .options
            .iter()
            .filter(|o| !o.builtin && !o.id.contains(':'))
            .map(|o| o.name.as_str())
            .collect();
        let mut truncated = false;
        // issue id -> (issue, indexes of the lists it sits in), first seen first.
        let mut seen: Vec<(String, J, Vec<usize>)> = Vec::new();
        for (idx, o) in field.options.iter().enumerate() {
            let mut q: Vec<(String, String)> = Vec::new();
            let mut labels = scope_labels.clone();
            let mut skip_scope_key = "";
            if o.id == "open" || o.id == "closed" {
                // the board's scope still applies
            } else if let Some((kind, v)) = o.id.split_once(':') {
                let key = match kind {
                    "assignee" => "assignee_username",
                    "milestone" => "milestone",
                    _ => "iteration_id",
                };
                skip_scope_key = key;
                q.push((key.into(), v.to_string()));
            } else {
                labels.insert(0, o.name.clone());
            }
            if !labels.is_empty() {
                q.insert(0, ("labels".into(), labels.join(",")));
            }
            q.push((
                "state".into(),
                if o.id == "closed" { "closed" } else { "opened" }.into(),
            ));
            q.extend(
                scope_pairs
                    .iter()
                    .filter(|(k, _)| k != skip_scope_key)
                    .cloned(),
            );
            let (rows, cut) = self.net.get_pages(ctx, &path, &q)?;
            truncated |= cut;
            for r in rows {
                if o.id == "open" {
                    let carries = matches!(r.get("labels"), Some(J::Arr(ls))
                        if ls.iter().filter_map(J::as_str).any(|l| label_lists.contains(&l)));
                    if carries {
                        continue;
                    }
                }
                let id = text(r.get("id"));
                match seen.iter_mut().find(|(i, _, _)| *i == id) {
                    Some((_, _, lists)) => lists.push(idx),
                    None => seen.push((id, r, vec![idx])),
                }
            }
        }
        let items = seen
            .into_iter()
            .map(|(id, r, lists)| {
                let (value, state) = classify(&r, &lists, field);
                Item {
                    id,
                    kind: "issue",
                    number: r.get("iid").and_then(J::as_i64),
                    url: text(r.get("web_url")),
                    title: text(r.get("title")),
                    open: text(r.get("state")) == "opened",
                    value,
                    state,
                }
            })
            .collect();
        Ok((items, truncated))
    }
}

/// The column and canonical state of one issue. An open issue's mapped labels decide, read from its
/// own `labels` (two of them is `conflict`), so the page cap of any one list cannot hide a conflict;
/// otherwise the lists it was found in decide.
fn classify(r: &J, lists: &[usize], field: &Field) -> (Option<String>, Option<String>) {
    if text(r.get("state")) == "opened" {
        let labels: Vec<&str> = match r.get("labels") {
            Some(J::Arr(a)) => a.iter().filter_map(J::as_str).collect(),
            _ => Vec::new(),
        };
        let mapped: Vec<&Opt> = field
            .options
            .iter()
            .filter(|o| {
                !o.builtin
                    && o.mappable
                    && !o.id.contains(':')
                    && o.state.is_some()
                    && labels.contains(&o.name.as_str())
            })
            .collect();
        match mapped.as_slice() {
            [] => {}
            [one] => return (Some(one.name.clone()), one.state.map(str::to_string)),
            many => {
                let names: Vec<&str> = many.iter().map(|o| o.name.as_str()).collect();
                return (Some(names.join(", ")), Some("conflict".to_string()));
            }
        }
    }
    let mapped: Vec<&Opt> = lists
        .iter()
        .map(|i| &field.options[*i])
        .filter(|o| o.state.is_some())
        .collect();
    match mapped.as_slice() {
        [] => (lists.first().map(|i| field.options[*i].name.clone()), None),
        [one] => (Some(one.name.clone()), one.state.map(str::to_string)),
        many => {
            let names: Vec<&str> = many.iter().map(|o| o.name.as_str()).collect();
            (Some(names.join(", ")), Some("conflict".to_string()))
        }
    }
}

/// How a board is scoped (Premium: labels, milestone, assignee, weight), as `kind:value` entries.
fn board_scope(b: &J) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(J::Arr(ls)) = b.get("labels") {
        for l in ls {
            let n = text(l.get("name"));
            let n = if n.is_empty() {
                text(l.get("title"))
            } else {
                n
            };
            if !n.is_empty() {
                out.push(format!("label:{n}"));
            }
        }
    }
    if let Some(m) = non_null(b, "milestone") {
        let t = text(m.get("title"));
        if !t.is_empty() && !t.eq_ignore_ascii_case("any milestone") {
            out.push(format!("milestone:{t}"));
        }
    }
    if let Some(a) = non_null(b, "assignee") {
        let u = text(a.get("username"));
        if !u.is_empty() {
            out.push(format!("assignee:{u}"));
        }
    }
    if let Some(w) = non_null(b, "weight") {
        let w = text(Some(w));
        if !w.is_empty() {
            out.push(format!("weight:{w}"));
        }
    }
    out
}

/// A scope as issue-query parts: the labels to add to every list's `labels`, and the other pairs.
fn scope_query(scope: &[String]) -> (Vec<String>, Vec<(String, String)>) {
    let (mut labels, mut pairs) = (Vec::new(), Vec::new());
    for e in scope {
        match e.split_once(':') {
            Some(("label", v)) => labels.push(v.to_string()),
            Some(("milestone", v)) => pairs.push(("milestone".to_string(), v.to_string())),
            Some(("assignee", v)) => pairs.push(("assignee_username".to_string(), v.to_string())),
            Some(("weight", v)) => pairs.push(("weight".to_string(), v.to_string())),
            _ => {}
        }
    }
    (labels, pairs)
}

impl BoardAdapter for GitLab {
    fn list(&self, ctx: &Ctx, scope: &Scope) -> R<(Vec<Board>, bool)> {
        self.net.check_token(ctx)?;
        let project = if let Some(p) = scope.project.clone().filter(|p| !p.is_empty()) {
            p
        } else {
            let r = detect_repo(ctx);
            if r.str("platform") == "gitlab" {
                r.str("project_path").to_string()
            } else {
                String::new()
            }
        };
        if project.is_empty() {
            return Err(Failed::usage(
                "no project: pass --project <group/project or id> (the remote isn't on GitLab)",
            ));
        }
        let (rows, truncated) = self.net.get_pages(
            ctx,
            &format!("/projects/{}/boards", project_segment(&project)),
            &[],
        )?;
        let boards = rows
            .iter()
            .map(|r| {
                self.board(
                    ctx,
                    &project,
                    &text(r.get("id")),
                    &text(r.get("name")),
                    "labels",
                    board_scope(r),
                )
            })
            .collect::<R<Vec<Board>>>()?;
        Ok((boards, truncated))
    }

    fn fields(&self, ctx: &Ctx, b: &BoardRef) -> R<(Board, Field)> {
        self.net.check_token(ctx)?;
        if self.status {
            self.status_fields(ctx, b)
        } else {
            self.label_fields(ctx, b)
        }
    }

    fn items(
        &self,
        ctx: &Ctx,
        resolved: &Board,
        b: &BoardRef,
        field: &Field,
    ) -> R<(Vec<Item>, bool)> {
        if self.status {
            self.status_items(ctx, b, field)
        } else {
            self.label_items(ctx, resolved, b, field)
        }
    }
}

impl GitLab {
    /// The read-only introspection query of G30-21, the raw answer: it shows which names of the status
    /// documents the instance really has.
    ///
    /// # Errors
    /// Transport failures.
    pub fn introspect(&self, ctx: &Ctx) -> R<J> {
        self.net.check_token(ctx)?;
        Ok(self.net.gql(ctx, docs::GITLAB_STATUS_INTROSPECT, &[])?.data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::pyjson::parse;

    #[test]
    fn a_list_is_a_label_or_a_filter_that_holds_no_state() {
        let l = parse(r#"{"id":5,"label":{"name":"workflow::todo"},"position":0}"#).unwrap();
        let o = list_option(&l).unwrap();
        assert_eq!(
            (o.id.as_str(), o.name.as_str(), o.mappable),
            ("5", "workflow::todo", true)
        );
        let a = parse(r#"{"id":6,"label":null,"assignee":{"username":"alice"}}"#).unwrap();
        let o = list_option(&a).unwrap();
        assert_eq!((o.id.as_str(), o.mappable), ("assignee:alice", false));
        let m = parse(r#"{"id":7,"milestone":{"title":"v1"}}"#).unwrap();
        assert_eq!(list_option(&m).unwrap().id, "milestone:v1");
        assert!(list_option(&parse(r#"{"id":8}"#).unwrap()).is_none());
    }

    #[test]
    fn a_conflict_comes_from_the_issues_own_labels_not_from_the_lists_it_was_found_in() {
        let mut todo = Opt::new("1", "workflow::todo");
        todo.state = Some("todo");
        let mut review = Opt::new("2", "workflow::review");
        review.state = Some("review");
        let field = Field {
            id: "lists".into(),
            name: "Lists".into(),
            kind: "lists",
            options: vec![builtin("open", "Open"), todo, review],
        };
        let issue =
            parse(r#"{"state":"opened","labels":["workflow::todo","workflow::review"]}"#).unwrap();
        // found in one list only (the other list's page cap cut it off): still a conflict
        let (value, state) = classify(&issue, &[1], &field);
        assert_eq!(state.as_deref(), Some("conflict"));
        assert_eq!(value.as_deref(), Some("workflow::todo, workflow::review"));
        let one = parse(r#"{"state":"opened","labels":["workflow::review","bug"]}"#).unwrap();
        assert_eq!(classify(&one, &[2], &field).1.as_deref(), Some("review"));
    }

    #[test]
    fn a_board_scope_is_read_and_turned_into_query_parts() {
        let b = parse(r#"{"labels":[{"name":"team::a"}],"milestone":{"title":"v1"},"assignee":{"username":"al"},"weight":3}"#).unwrap();
        let scope = board_scope(&b);
        assert_eq!(
            scope,
            ["label:team::a", "milestone:v1", "assignee:al", "weight:3"]
        );
        let (labels, pairs) = scope_query(&scope);
        assert_eq!(labels, ["team::a"]);
        assert_eq!(pairs.len(), 3);
        assert!(board_scope(&parse(r#"{"labels":[],"milestone":null}"#).unwrap()).is_empty());
    }

    #[test]
    fn builtin_lists_are_marked() {
        let o = builtin("closed", "Closed");
        assert!(o.builtin && o.mappable);
    }
}
