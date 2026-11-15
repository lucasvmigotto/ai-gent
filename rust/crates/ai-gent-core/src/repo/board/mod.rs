//! `ai-gent repo board`: project boards across platforms (spec 045, `contracts/board-adapters.md`).
//!
//! The canonical model, the adapter trait, platform dispatch and the output of `list`, `show` and
//! `fields`. Every request an adapter makes goes through `api::api_call` (see `net.rs`), so the GraphQL
//! read check and the `--write` refusal decide; slice 1 sends reads only.
pub mod docs;
pub mod github;
pub mod gitlab;
pub mod map;
pub mod net;

use super::remote::detect_repo;
use super::{Ctx, Failed, R};
use crate::tokens::pyjson::{J, dumps, obj};
use map::{BoardCfg, Map};
use std::collections::BTreeMap;

/// The canonical states (D5), in board order.
pub const STATES: [&str; 4] = ["todo", "doing", "review", "done"];

/// How an option got its canonical state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MappedBy {
    /// The board's entry in `.ai-gent/board.yaml`.
    Map,
    /// The alias table of `project_status.rs`.
    Alias,
    /// Neither.
    Unmapped,
}

impl MappedBy {
    /// The word in the output.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Map => "map",
            Self::Alias => "alias",
            Self::Unmapped => "unmapped",
        }
    }
}

/// A board.
#[derive(Debug, Clone)]
pub struct Board {
    /// `github` or `gitlab`.
    pub platform: String,
    /// The platform's id.
    pub id: String,
    /// `github:OWNER/NUMBER` or `gitlab:GROUP/PROJECT/ID`.
    pub key: String,
    /// Title.
    pub title: String,
    /// Web address.
    pub url: String,
    /// Whether the board is closed.
    pub closed: bool,
    /// `field` (GitHub), `labels` or `status` (GitLab).
    pub mode: String,
}

impl Board {
    /// The `--json` object.
    #[must_use]
    pub fn to_json(&self) -> J {
        obj(vec![
            ("platform", J::s(&self.platform)),
            ("id", J::s(&self.id)),
            ("key", J::s(&self.key)),
            ("title", J::s(&self.title)),
            ("url", J::s(&self.url)),
            ("closed", J::Bool(self.closed)),
            ("mode", J::s(&self.mode)),
        ])
    }
}

/// An option of a field (a GitHub option, a GitLab list, a GitLab status).
#[derive(Debug, Clone)]
pub struct Opt {
    /// The platform's id (`open` and `closed` for GitLab's built-in lists).
    pub id: String,
    /// Name (the label's name for a GitLab label list).
    pub name: String,
    /// The canonical state, when mapped.
    pub state: Option<&'static str>,
    /// How it was mapped.
    pub mapped_by: MappedBy,
    /// False for a list that cannot hold a state (assignee, milestone, iteration).
    pub mappable: bool,
    /// GitLab's built-in Open and Closed lists.
    pub builtin: bool,
}

impl Opt {
    /// A plain mappable option.
    #[must_use]
    pub fn new(id: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            state: None,
            mapped_by: MappedBy::Unmapped,
            mappable: true,
            builtin: false,
        }
    }

    fn to_json(&self) -> J {
        obj(vec![
            ("id", J::s(&self.id)),
            ("name", J::s(&self.name)),
            ("state", J::opt_s(self.state)),
            ("mapped_by", J::s(self.mapped_by.word())),
        ])
    }
}

/// The field that holds an item's column.
#[derive(Debug, Clone)]
pub struct Field {
    /// The platform's id.
    pub id: String,
    /// Name.
    pub name: String,
    /// `single_select`, `lists` or `status`.
    pub kind: &'static str,
    /// The options, in board order.
    pub options: Vec<Opt>,
}

impl Field {
    /// The `--json` object.
    #[must_use]
    pub fn to_json(&self) -> J {
        obj(vec![
            ("id", J::s(&self.id)),
            ("name", J::s(&self.name)),
            ("kind", J::s(self.kind)),
            (
                "options",
                J::Arr(self.options.iter().map(Opt::to_json).collect()),
            ),
        ])
    }

    /// The option with this name.
    #[must_use]
    pub fn option(&self, name: &str) -> Option<&Opt> {
        self.options.iter().find(|o| o.name == name)
    }
}

/// An item on a board.
#[derive(Debug, Clone)]
pub struct Item {
    /// The platform's id.
    pub id: String,
    /// `issue`, `pull_request` or `draft`.
    pub kind: &'static str,
    /// Issue or pull request number.
    pub number: Option<i64>,
    /// Web address.
    pub url: String,
    /// Title.
    pub title: String,
    /// Whether it is open.
    pub open: bool,
    /// The option's name it sits in.
    pub value: Option<String>,
    /// A canonical state, `conflict`, or none.
    pub state: Option<String>,
}

impl Item {
    fn to_json(&self) -> J {
        obj(vec![
            ("id", J::s(&self.id)),
            ("kind", J::s(self.kind)),
            ("number", J::opt_i(self.number)),
            ("url", J::s(&self.url)),
            ("title", J::s(&self.title)),
            ("open", J::Bool(self.open)),
            ("value", J::opt_s(self.value.as_deref())),
            ("state", J::opt_s(self.state.as_deref())),
        ])
    }
}

/// A board named on the command line, resolved against the map file.
#[derive(Debug, Clone, Default)]
pub struct BoardRef {
    /// `github` or `gitlab`.
    pub platform: String,
    /// The name in the file, when it came from there.
    pub name: Option<String>,
    /// GitHub owner.
    pub owner: String,
    /// GitHub project number.
    pub number: i64,
    /// GitHub field.
    pub field: String,
    /// GitLab project path or id.
    pub project: String,
    /// GitLab board id.
    pub board_id: i64,
    /// GitLab mode.
    pub mode: String,
    /// Canonical state to option or label name.
    pub states: BTreeMap<String, String>,
}

impl BoardRef {
    fn from_cfg(c: &BoardCfg) -> Self {
        Self {
            platform: c.platform.clone(),
            name: Some(c.name.clone()),
            owner: c.owner.clone(),
            number: c.number,
            field: c.field.clone(),
            project: c.project.clone(),
            board_id: c.board,
            mode: c.mode.clone(),
            states: c.states.clone(),
        }
    }

    /// `github:OWNER/NUMBER` or `gitlab:GROUP/PROJECT/ID`.
    #[must_use]
    pub fn key(&self) -> String {
        if self.platform == "github" {
            format!("github:{}/{}", self.owner, self.number)
        } else {
            format!("gitlab:{}/{}", self.project, self.board_id)
        }
    }
}

/// What `list` looks at.
#[derive(Debug, Clone, Default)]
pub struct Scope {
    /// GitHub: a user or organization login.
    pub owner: Option<String>,
    /// GitLab: a project path or id.
    pub project: Option<String>,
}

/// One platform's reads (contract §1). The writes arrive with slice 2.
pub trait BoardAdapter {
    /// The boards in a scope.
    ///
    /// # Errors
    /// Transport, scope and not-found failures.
    fn list(&self, ctx: &Ctx, scope: &Scope) -> R<Vec<Board>>;
    /// The board and its column field, options not yet mapped.
    ///
    /// # Errors
    /// Transport, scope, not-found and field failures.
    fn fields(&self, ctx: &Ctx, board: &BoardRef) -> R<(Board, Field)>;
    /// The items with their column, and whether the page cap cut the list.
    ///
    /// # Errors
    /// Transport failures.
    fn items(
        &self,
        ctx: &Ctx,
        resolved: &Board,
        board: &BoardRef,
        field: &Field,
    ) -> R<(Vec<Item>, bool)>;
}

/// The refusal for a platform with no adapter in 3.0.0 (FR-028).
fn deferred(platform: &str) -> Failed {
    match platform {
        "azure-devops" => Failed::usage(
            "ai-gent repo board: Azure DevOps boards are deferred until after 3.0.0 (spec 045, contract section 5); use the work item states with ai-gent repo api for now",
        ),
        "jira" | "forgejo" | "bitbucket" => Failed::usage(format!(
            "ai-gent repo board: {platform} boards arrive in the 3.x line (spec 045); GitHub and GitLab only for now"
        )),
        other => Failed::usage(format!(
            "ai-gent repo board: unknown platform '{other}' (github or gitlab)"
        )),
    }
}

/// The adapter for a platform.
///
/// # Errors
/// A usage error for a platform without a board adapter yet.
pub fn adapter_for(
    platform: &str,
    entry: Option<&str>,
    max_pages: usize,
    status_mode: bool,
) -> R<Box<dyn BoardAdapter>> {
    match platform {
        "github" => Ok(Box::new(github::GitHub::new(entry, max_pages))),
        "gitlab" => Ok(Box::new(gitlab::GitLab::new(entry, max_pages, status_mode))),
        other => Err(deferred(other)),
    }
}

/// `--platform`, else the remote's platform.
///
/// # Errors
/// A usage error when neither names one.
pub fn pick_platform(ctx: &Ctx, flag: Option<&str>) -> R<String> {
    if let Some(p) = flag.filter(|p| !p.is_empty()) {
        return Ok(p.to_string());
    }
    let from_remote = detect_repo(ctx).str("platform").to_string();
    if from_remote.is_empty() {
        Err(Failed::usage(
            "no platform: pass --platform github|gitlab (the remote doesn't name one)",
        ))
    } else {
        Ok(from_remote)
    }
}

/// Resolve the BOARD argument (FR-002): a name from the file, a literal `github:OWNER/NUMBER` or
/// `gitlab:GROUP/PROJECT/BOARD_ID`, or, with the argument omitted, the file's only or default board.
///
/// # Errors
/// A usage error when nothing names one board.
pub fn resolve_ref(arg: Option<&str>, map: Option<&Map>) -> R<BoardRef> {
    let Some(arg) = arg.filter(|a| !a.is_empty()) else {
        let Some(m) = map else {
            return Err(Failed::usage(
                "no board: name one (github:OWNER/NUMBER or gitlab:GROUP/PROJECT/BOARD_ID) or add .ai-gent/board.yaml",
            ));
        };
        let cfg = match (m.boards.len(), &m.default) {
            (1, _) => m.boards.first(),
            (_, Some(d)) => m.boards.iter().find(|b| &b.name == d),
            _ => None,
        };
        return cfg.map(BoardRef::from_cfg).ok_or_else(|| {
            Failed::usage(format!(
                "several boards in .ai-gent/board.yaml and no default: name one of {}",
                m.boards
                    .iter()
                    .map(|b| b.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        });
    };
    if let Some(cfg) = map.and_then(|m| m.boards.iter().find(|b| b.name == arg)) {
        return Ok(BoardRef::from_cfg(cfg));
    }
    let mut r = BoardRef::default();
    if let Some(rest) = arg.strip_prefix("github:") {
        let (owner, number) = rest.split_once('/').ok_or_else(bad_literal)?;
        r.platform = "github".into();
        r.owner = owner.to_string();
        r.number = number
            .parse::<i64>()
            .ok()
            .filter(|n| *n > 0)
            .ok_or_else(bad_literal)?;
        r.field = "Status".into();
        if owner.is_empty() {
            return Err(bad_literal());
        }
    } else if let Some(rest) = arg.strip_prefix("gitlab:") {
        let (project, id) = rest.rsplit_once('/').ok_or_else(bad_literal)?;
        r.platform = "gitlab".into();
        r.project = project.to_string();
        r.board_id = id
            .parse::<i64>()
            .ok()
            .filter(|n| *n > 0)
            .ok_or_else(bad_literal)?;
        r.mode = "labels".into();
        if project.is_empty() {
            return Err(bad_literal());
        }
    } else {
        return Err(Failed::usage(format!(
            "'{arg}' is not a board in .ai-gent/board.yaml; use github:OWNER/NUMBER or gitlab:GROUP/PROJECT/BOARD_ID"
        )));
    }
    // A literal that the file also describes keeps the file's states and field.
    if let Some(cfg) = map.and_then(|m| {
        m.boards.iter().find(|b| {
            b.platform == r.platform
                && if r.platform == "github" {
                    b.owner == r.owner && b.number == r.number
                } else {
                    b.project == r.project && b.board == r.board_id
                }
        })
    }) {
        return Ok(BoardRef::from_cfg(cfg));
    }
    Ok(r)
}

fn bad_literal() -> Failed {
    Failed::usage(
        "a board is github:OWNER/NUMBER or gitlab:GROUP/PROJECT/BOARD_ID (positive numbers)",
    )
}

// ----------------------------------------------------------------------------- output

fn pad_table(rows: &[Vec<String>]) -> Vec<String> {
    let cols = rows.iter().map(Vec::len).max().unwrap_or(0);
    let widths: Vec<usize> = (0..cols)
        .map(|c| {
            rows.iter()
                .map(|r| r.get(c).map_or(0, |s| s.chars().count()))
                .max()
                .unwrap_or(0)
        })
        .collect();
    rows.iter()
        .map(|r| {
            let mut line = String::new();
            for (c, cell) in r.iter().enumerate() {
                line.push_str(cell);
                if c + 1 < r.len() {
                    let pad = widths[c] - cell.chars().count();
                    line.push_str(&" ".repeat(pad + 2));
                }
            }
            line.trim_end().to_string()
        })
        .collect()
}

fn yes(b: bool) -> String {
    if b { "yes" } else { "no" }.to_string()
}

/// `list --json` and its text.
#[must_use]
pub fn render_list(boards: &[Board], json: bool) -> Vec<String> {
    if json {
        let j = J::Arr(boards.iter().map(Board::to_json).collect());
        return vec![dumps(&j, false, Some(2))];
    }
    if boards.is_empty() {
        return vec!["no boards".to_string()];
    }
    let mut rows = vec![vec![
        "BOARD".to_string(),
        "TITLE".to_string(),
        "CLOSED".to_string(),
        "URL".to_string(),
    ]];
    for b in boards {
        rows.push(vec![
            b.key.clone(),
            b.title.clone(),
            yes(b.closed),
            b.url.clone(),
        ]);
    }
    pad_table(&rows)
}

/// The states no option maps to.
#[must_use]
pub fn unmapped_states(field: &Field) -> Vec<&'static str> {
    STATES
        .iter()
        .copied()
        .filter(|s| !field.options.iter().any(|o| o.state == Some(*s)))
        .collect()
}

/// `fields --json` and its text.
#[must_use]
pub fn render_fields(
    board: &Board,
    field: &Field,
    stale: &[(String, String)],
    json: bool,
) -> Vec<String> {
    let unmapped = unmapped_states(field);
    if json {
        let j = obj(vec![
            ("board", board.to_json()),
            ("field", field.to_json()),
            ("unmapped", J::strs(&unmapped)),
            (
                "stale",
                J::Arr(
                    stale
                        .iter()
                        .map(|(s, n)| obj(vec![("state", J::s(s)), ("name", J::s(n))]))
                        .collect(),
                ),
            ),
        ]);
        return vec![dumps(&j, false, Some(2))];
    }
    let mut out = vec![
        format!("{} ({}, mode {})", board.key, board.title, board.mode),
        format!("field: {} ({})", field.name, field.kind),
    ];
    let mut rows = vec![vec![
        "OPTION".to_string(),
        "STATE".to_string(),
        "MAPPED BY".to_string(),
    ]];
    for o in &field.options {
        rows.push(vec![
            o.name.clone(),
            o.state.unwrap_or("-").to_string(),
            o.mapped_by.word().to_string(),
        ]);
    }
    out.extend(pad_table(&rows));
    for (s, n) in stale {
        out.push(format!(
            "stale: {s} is mapped to '{n}', which the board no longer has"
        ));
    }
    if !unmapped.is_empty() {
        out.push(format!("unmapped states: {}", unmapped.join(", ")));
    }
    out
}

/// `show --json` and its text.
#[must_use]
pub fn render_show(
    board: &Board,
    field: &Field,
    items: &[Item],
    truncated: bool,
    json: bool,
) -> Vec<String> {
    if json {
        let j = obj(vec![
            ("board", board.to_json()),
            ("field", field.to_json()),
            ("items", J::Arr(items.iter().map(Item::to_json).collect())),
            ("truncated", J::Bool(truncated)),
        ]);
        return vec![dumps(&j, false, Some(2))];
    }
    let mut out = vec![format!(
        "{} ({}) \u{b7} field {}",
        board.key, board.title, field.name
    )];
    let mut rows = vec![vec![
        "STATE".to_string(),
        "COLUMN".to_string(),
        "KIND".to_string(),
        "#".to_string(),
        "TITLE".to_string(),
        "URL".to_string(),
    ]];
    for i in items {
        rows.push(vec![
            i.state.clone().unwrap_or_else(|| "-".into()),
            i.value.clone().unwrap_or_else(|| "-".into()),
            i.kind.to_string(),
            i.number.map_or_else(|| "-".into(), |n| n.to_string()),
            i.title.clone(),
            i.url.clone(),
        ]);
    }
    out.extend(pad_table(&rows));
    out.push(format!(
        "{} item(s){}",
        items.len(),
        if truncated {
            " \u{b7} truncated by --max-pages"
        } else {
            ""
        }
    ));
    out
}

#[cfg(test)]
mod model {
    use super::*;

    fn map() -> Map {
        map::parse(
            "version: 1\ndefault: b\nboards:\n  a:\n    platform: github\n    owner: o\n    number: 1\n  b:\n    platform: gitlab\n    project: g/p\n    board: 7\n",
        )
        .unwrap()
    }

    #[test]
    fn a_board_is_a_name_or_a_literal() {
        let m = map();
        assert_eq!(
            resolve_ref(Some("a"), Some(&m)).unwrap().key(),
            "github:o/1"
        );
        assert_eq!(resolve_ref(None, Some(&m)).unwrap().key(), "gitlab:g/p/7");
        let l = resolve_ref(Some("gitlab:grp/sub/proj/12"), None).unwrap();
        assert_eq!((l.project.as_str(), l.board_id), ("grp/sub/proj", 12));
        let g = resolve_ref(Some("github:acme/3"), Some(&m)).unwrap();
        assert_eq!(
            (g.owner.as_str(), g.number, g.field.as_str()),
            ("acme", 3, "Status")
        );
    }

    #[test]
    fn bad_boards_are_usage_errors() {
        assert_eq!(resolve_ref(Some("nope"), None).unwrap_err().code, 2);
        assert_eq!(resolve_ref(Some("github:o/x"), None).unwrap_err().code, 2);
        assert_eq!(resolve_ref(None, None).unwrap_err().code, 2);
    }

    #[test]
    fn other_platforms_are_refused_with_the_plan() {
        for (p, word) in [
            ("azure-devops", "deferred"),
            ("jira", "3.x"),
            ("forgejo", "3.x"),
            ("bitbucket", "3.x"),
        ] {
            let e = adapter_for(p, None, 10, false).err().unwrap();
            assert_eq!(e.code, 2, "{p}");
            assert!(e.msg.contains(word), "{p}: {}", e.msg);
        }
    }

    #[test]
    fn the_model_prints_the_same_keys() {
        let b = Board {
            platform: "github".into(),
            id: "P1".into(),
            key: "github:o/1".into(),
            title: "T".into(),
            url: "u".into(),
            closed: false,
            mode: "field".into(),
        };
        let out = render_list(&[b], true).join("\n");
        for k in ["platform", "id", "key", "title", "url", "closed", "mode"] {
            assert!(out.contains(&format!("\"{k}\"")), "{k}");
        }
    }
}

// ----------------------------------------------------------------------------- the reads

/// What the three read actions take from the command line.
#[derive(Debug, Clone, Default)]
pub struct ReadOpts<'a> {
    /// `--platform` (list).
    pub platform: Option<&'a str>,
    /// `--owner` (list, GitHub).
    pub owner: Option<&'a str>,
    /// `--project` (list, GitLab).
    pub project: Option<&'a str>,
    /// The BOARD argument (show, fields).
    pub board: Option<&'a str>,
    /// `--state` (show).
    pub state: Option<&'a str>,
    /// `--entry`.
    pub entry: Option<&'a str>,
    /// `--max-pages`.
    pub max_pages: usize,
    /// `--json`.
    pub json: bool,
    /// `--introspect` (fields, GitLab).
    pub introspect: bool,
}

/// `board list`: the lines to print.
///
/// # Errors
/// Usage errors, a platform without an adapter, and the adapter's failures.
pub fn run_list(ctx: &Ctx, o: &ReadOpts<'_>) -> R<Vec<String>> {
    let platform = match (o.platform, o.owner, o.project) {
        (Some(p), _, _) if !p.is_empty() => p.to_string(),
        (_, Some(_), _) => "github".to_string(),
        (_, _, Some(_)) => "gitlab".to_string(),
        _ => pick_platform(ctx, None)?,
    };
    if platform == "github" && o.project.is_some() {
        return Err(Failed::usage(
            "--project is for GitLab; GitHub boards use --owner",
        ));
    }
    if platform == "gitlab" && o.owner.is_some() {
        return Err(Failed::usage(
            "--owner is for GitHub; GitLab boards use --project",
        ));
    }
    let adapter = adapter_for(&platform, o.entry, o.max_pages, false)?;
    let boards = adapter.list(
        ctx,
        &Scope {
            owner: o.owner.map(str::to_string),
            project: o.project.map(str::to_string),
        },
    )?;
    Ok(render_list(&boards, o.json))
}

fn resolved(
    ctx: &Ctx,
    o: &ReadOpts<'_>,
) -> R<(
    Box<dyn BoardAdapter>,
    BoardRef,
    Board,
    Field,
    Vec<(String, String)>,
)> {
    let map = map::load(ctx)?;
    let bref = resolve_ref(o.board, map.as_ref())?;
    let adapter = adapter_for(&bref.platform, o.entry, o.max_pages, bref.mode == "status")?;
    let (board, mut field) = adapter.fields(ctx, &bref)?;
    let stale = map::apply(&bref.states, &mut field.options);
    Ok((adapter, bref, board, field, stale))
}

/// `board fields`: the lines to print.
///
/// # Errors
/// As `run_list`, plus a board or field that does not exist.
pub fn run_fields(ctx: &Ctx, o: &ReadOpts<'_>) -> R<Vec<String>> {
    if o.introspect {
        let map = map::load(ctx)?;
        let bref = resolve_ref(o.board, map.as_ref())?;
        if bref.platform != "gitlab" {
            return Err(Failed::usage("--introspect is for GitLab boards"));
        }
        let j = gitlab::GitLab::new(o.entry, o.max_pages, true).introspect(ctx)?;
        return Ok(vec![dumps(&j, false, Some(2))]);
    }
    let (_, _, board, field, stale) = resolved(ctx, o)?;
    Ok(render_fields(&board, &field, &stale, o.json))
}

/// `board show`: the lines to print.
///
/// # Errors
/// As `run_fields`, and a `--state` no option maps to.
pub fn run_show(ctx: &Ctx, o: &ReadOpts<'_>) -> R<Vec<String>> {
    let (adapter, bref, board, field, stale) = resolved(ctx, o)?;
    if let Some(s) = o.state {
        if !field.options.iter().any(|x| x.state == Some(s)) {
            let names: Vec<&str> = field.options.iter().map(|x| x.name.as_str()).collect();
            return Err(Failed::err(format!(
                "no option on '{}' maps to state '{s}'; options: {}; map it in .ai-gent/board.yaml (board fields shows how each option is mapped)",
                field.name,
                names.join(", ")
            )));
        }
    }
    for (state, name) in &stale {
        super::note(&format!(
            "stale map: {state} is mapped to '{name}', which the board no longer has"
        ));
    }
    let (mut items, truncated) = adapter.items(ctx, &board, &bref, &field)?;
    if let Some(s) = o.state {
        items.retain(|i| i.state.as_deref() == Some(s));
    }
    Ok(render_show(&board, &field, &items, truncated, o.json))
}
