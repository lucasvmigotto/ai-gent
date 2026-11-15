//! `.ai-gent/board.yaml`: a hand-written parser for the YAML subset the board map uses (spec 045 FR-010,
//! contract §2), and the resolution of a canonical state to a board's own option (FR-011, FR-022).
//!
//! The subset: nested mappings by indentation (spaces), `key: value` lines, plain or quoted scalars,
//! `#` comments. Everything else is refused with the line number: tabs, anchors and aliases, tags, flow
//! style, block scalars and multi-line values, lists, duplicate keys, unknown keys, a file over 64 KiB.
use super::{MappedBy, Opt, STATES};
use crate::repo::project_status::alias_index;
use crate::repo::{Ctx, Failed, R};
use std::collections::BTreeMap;

/// The largest map file read.
pub const MAX_BYTES: usize = 64 * 1024;

/// One board's entry in the file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BoardCfg {
    /// The name the file gives it.
    pub name: String,
    /// `github` or `gitlab`.
    pub platform: String,
    /// GitHub: the owner's login.
    pub owner: String,
    /// GitHub: the project number.
    pub number: i64,
    /// GitHub: the single-select field (default `Status`).
    pub field: String,
    /// GitLab: the project path or numeric id.
    pub project: String,
    /// GitLab: the board id.
    pub board: i64,
    /// GitLab: `labels` (default) or `status`.
    pub mode: String,
    /// Canonical state to the board's option or label name.
    pub states: BTreeMap<String, String>,
}

/// The whole file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Map {
    /// The board used when none is named and several are configured.
    pub default: Option<String>,
    /// The boards, in file order.
    pub boards: Vec<BoardCfg>,
}

#[derive(Debug, Clone)]
enum Node {
    Scalar(String),
    Map(Vec<Entry>),
}

#[derive(Debug, Clone)]
struct Entry {
    key: String,
    line: usize,
    node: Node,
}

struct Ln {
    no: usize,
    indent: usize,
    key: String,
    value: Option<String>,
}

fn refuse(line: usize, msg: impl std::fmt::Display) -> Failed {
    Failed::usage(format!(".ai-gent/board.yaml line {line}: {msg}"))
}

fn scalar(raw: &str, line: usize) -> R<Option<String>> {
    let t = raw.trim();
    if t.is_empty() || t.starts_with('#') {
        return Ok(None);
    }
    let first = t.chars().next().unwrap_or(' ');
    match first {
        '&' | '*' => return Err(refuse(line, "anchors and aliases are not supported")),
        '!' => return Err(refuse(line, "tags are not supported")),
        '{' | '[' => return Err(refuse(line, "flow style ({} or []) is not supported")),
        '|' | '>' => return Err(refuse(line, "block scalars are not supported")),
        '@' | '`' | '%' => return Err(refuse(line, "a value cannot start with that character")),
        _ => {}
    }
    if first == '"' || first == '\'' {
        let mut out = String::new();
        let mut it = t.char_indices().skip(1);
        let mut end = None;
        while let Some((i, c)) = it.next() {
            if first == '"' && c == '\\' {
                match it.next() {
                    Some((_, '"')) => out.push('"'),
                    Some((_, '\\')) => out.push('\\'),
                    _ => return Err(refuse(line, "only \\\" and \\\\ escapes are supported")),
                }
            } else if first == '\'' && c == '\'' {
                if t[i + 1..].starts_with('\'') {
                    out.push('\'');
                    it.next();
                } else {
                    end = Some(i);
                    break;
                }
            } else if first == '"' && c == '"' {
                end = Some(i);
                break;
            } else {
                out.push(c);
            }
        }
        let Some(e) = end else {
            return Err(refuse(
                line,
                "unterminated quoted value (multi-line values are not supported)",
            ));
        };
        let tail = t[e + 1..].trim();
        if !(tail.is_empty() || tail.starts_with('#')) {
            return Err(refuse(line, "text after the closing quote"));
        }
        return Ok(Some(out));
    }
    let plain = match t.find(" #") {
        Some(i) => t[..i].trim_end(),
        None => t,
    };
    if plain.contains(": ") || plain.ends_with(':') {
        return Err(refuse(
            line,
            "a plain value cannot contain `: ` (quote it, or this is a nested key)",
        ));
    }
    Ok(Some(plain.to_string()))
}

fn lines(text: &str) -> R<Vec<Ln>> {
    let mut out = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let no = i + 1;
        let body = raw.trim_end();
        let trimmed = body.trim_start_matches([' ', '\t']);
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let lead = &body[..body.len() - trimmed.len()];
        if lead.contains('\t') {
            return Err(refuse(no, "tabs are not supported (indent with spaces)"));
        }
        if trimmed.starts_with("- ") || trimmed == "-" {
            return Err(refuse(no, "lists are not supported"));
        }
        let colon = trimmed
            .char_indices()
            .find(|(_, c)| *c == ':')
            .map(|(i, _)| i);
        let Some(ci) = colon else {
            return Err(refuse(no, "expected `key: value`"));
        };
        let key = &trimmed[..ci];
        let after = &trimmed[ci + 1..];
        let valid_key = !key.is_empty()
            && key
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
            && key
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'));
        if !valid_key || !(after.is_empty() || after.starts_with(' ')) {
            return Err(refuse(
                no,
                "expected `key: value` (keys are letters, digits, `_`, `-`, `.`)",
            ));
        }
        out.push(Ln {
            no,
            indent: lead.len(),
            key: key.to_string(),
            value: scalar(after, no)?,
        });
    }
    Ok(out)
}

fn block(lns: &[Ln], i: &mut usize, indent: usize, depth: usize) -> R<Vec<Entry>> {
    let mut out: Vec<Entry> = Vec::new();
    while *i < lns.len() {
        let l = &lns[*i];
        if l.indent < indent {
            break;
        }
        if l.indent > indent {
            return Err(refuse(l.no, "unexpected indentation"));
        }
        if out.iter().any(|e| e.key == l.key) {
            return Err(refuse(l.no, format!("duplicate key `{}`", l.key)));
        }
        *i += 1;
        let child_indent = lns.get(*i).map(|n| n.indent).filter(|n| *n > indent);
        let node = match (&l.value, child_indent) {
            (Some(_), Some(_)) => {
                return Err(refuse(
                    lns[*i].no,
                    "a value and nested keys under one key (multi-line values are not supported)",
                ));
            }
            (Some(v), None) => Node::Scalar(v.clone()),
            (None, Some(ci)) => {
                if depth >= 4 {
                    return Err(refuse(lns[*i].no, "nested too deep"));
                }
                Node::Map(block(lns, i, ci, depth + 1)?)
            }
            (None, None) => Node::Map(Vec::new()),
        };
        out.push(Entry {
            key: l.key.clone(),
            line: l.no,
            node,
        });
    }
    Ok(out)
}

fn want_scalar(e: &Entry) -> R<&str> {
    match &e.node {
        Node::Scalar(s) => Ok(s),
        Node::Map(_) => Err(refuse(e.line, format!("`{}` needs a value", e.key))),
    }
}

fn want_map(e: &Entry) -> R<&[Entry]> {
    match &e.node {
        Node::Map(m) => Ok(m),
        Node::Scalar(_) => Err(refuse(e.line, format!("`{}` needs nested keys", e.key))),
    }
}

fn want_int(e: &Entry) -> R<i64> {
    want_scalar(e)?
        .parse::<i64>()
        .ok()
        .filter(|n| *n > 0)
        .ok_or_else(|| refuse(e.line, format!("`{}` must be a positive integer", e.key)))
}

fn board_cfg(e: &Entry) -> R<BoardCfg> {
    let mut c = BoardCfg {
        name: e.key.clone(),
        ..BoardCfg::default()
    };
    let mut seen_platform = false;
    for f in want_map(e)? {
        match f.key.as_str() {
            "platform" => {
                let p = want_scalar(f)?;
                if p != "github" && p != "gitlab" {
                    return Err(refuse(f.line, "platform must be github or gitlab"));
                }
                c.platform = p.to_string();
                seen_platform = true;
            }
            "owner" => c.owner = want_scalar(f)?.to_string(),
            "number" => c.number = want_int(f)?,
            "field" => c.field = want_scalar(f)?.to_string(),
            "project" => c.project = want_scalar(f)?.to_string(),
            "board" => c.board = want_int(f)?,
            "mode" => {
                let m = want_scalar(f)?;
                if m != "labels" && m != "status" {
                    return Err(refuse(f.line, "mode must be labels or status"));
                }
                c.mode = m.to_string();
            }
            "states" => {
                for s in want_map(f)? {
                    if !STATES.contains(&s.key.as_str()) {
                        return Err(refuse(
                            s.line,
                            format!("unknown state `{}` (todo, doing, review, done)", s.key),
                        ));
                    }
                    c.states
                        .insert(s.key.clone(), want_scalar(s)?.trim().to_string());
                }
            }
            other => return Err(refuse(f.line, format!("unknown key `{other}`"))),
        }
    }
    if !seen_platform {
        return Err(refuse(e.line, "board needs `platform`"));
    }
    if c.platform == "github" {
        if c.owner.is_empty() || c.number == 0 {
            return Err(refuse(e.line, "a github board needs `owner` and `number`"));
        }
        if !c.project.is_empty() || c.board != 0 || !c.mode.is_empty() {
            return Err(refuse(
                e.line,
                "`project`, `board` and `mode` belong to gitlab boards",
            ));
        }
        if c.field.is_empty() {
            c.field = "Status".into();
        }
    } else {
        if c.project.is_empty() || c.board == 0 {
            return Err(refuse(e.line, "a gitlab board needs `project` and `board`"));
        }
        if !c.owner.is_empty() || c.number != 0 || !c.field.is_empty() {
            return Err(refuse(
                e.line,
                "`owner`, `number` and `field` belong to github boards",
            ));
        }
        if c.mode.is_empty() {
            c.mode = "labels".into();
        }
    }
    Ok(c)
}

/// Parse the file's text.
///
/// # Errors
/// A usage error naming the line, for anything outside the subset or the contract's keys.
pub fn parse(text: &str) -> R<Map> {
    if text.len() > MAX_BYTES {
        return Err(Failed::usage(
            ".ai-gent/board.yaml is over 64 KiB; the board map is a few lines",
        ));
    }
    let lns = lines(text)?;
    let mut i = 0;
    let top = if lns.is_empty() {
        Vec::new()
    } else {
        block(&lns, &mut i, 0, 0)?
    };
    let mut map = Map::default();
    let (mut version, mut boards) = (false, false);
    for e in &top {
        match e.key.as_str() {
            "version" => {
                if want_scalar(e)? != "1" {
                    return Err(refuse(e.line, "version must be 1"));
                }
                version = true;
            }
            "default" => map.default = Some(want_scalar(e)?.to_string()),
            "boards" => {
                boards = true;
                for b in want_map(e)? {
                    map.boards.push(board_cfg(b)?);
                }
            }
            other => return Err(refuse(e.line, format!("unknown key `{other}`"))),
        }
    }
    if !version {
        return Err(Failed::usage(
            ".ai-gent/board.yaml: `version: 1` is missing",
        ));
    }
    if !boards {
        return Err(Failed::usage(".ai-gent/board.yaml: `boards:` is missing"));
    }
    if let Some(d) = &map.default {
        if !map.boards.iter().any(|b| &b.name == d) {
            return Err(Failed::usage(format!(
                ".ai-gent/board.yaml: default `{d}` is not one of the boards"
            )));
        }
    }
    Ok(map)
}

/// The map of this clone, `None` when the file does not exist.
///
/// # Errors
/// An unreadable or invalid file.
pub fn load(ctx: &Ctx) -> R<Option<Map>> {
    let top = crate::repo::remote::git(ctx, &["rev-parse", "--show-toplevel"]);
    let base = if top.is_empty() {
        std::env::current_dir().unwrap_or_default()
    } else {
        std::path::PathBuf::from(top)
    };
    let path = base.join(".ai-gent").join("board.yaml");
    match std::fs::metadata(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(Failed::err(format!(".ai-gent/board.yaml: {}", e.kind())));
        }
        Ok(m) if m.len() > MAX_BYTES as u64 => {
            return Err(Failed::usage(
                ".ai-gent/board.yaml is over 64 KiB; the board map is a few lines",
            ));
        }
        Ok(_) => {}
    }
    let text = std::fs::read_to_string(&path)
        .map_err(|e| Failed::err(format!(".ai-gent/board.yaml: {}", e.kind())))?;
    parse(&text).map(Some)
}

/// Set each option's canonical state: the board's entry in the file, else the alias table (FR-011).
/// Returns the file's entries that name an option the board no longer has, as `(state, name)` (FR-022):
/// those are never replaced by an alias guess.
pub fn apply(states: &BTreeMap<String, String>, opts: &mut [Opt]) -> Vec<(String, String)> {
    let mut stale = Vec::new();
    for s in STATES {
        let Some(v) = states.get(s) else { continue };
        let v = v.trim();
        let hit = opts
            .iter()
            .position(|o| o.mappable && (o.name == v || (o.builtin && o.id == v)));
        match hit {
            Some(i) if opts[i].state.is_none() => {
                opts[i].state = Some(s);
                opts[i].mapped_by = MappedBy::Map;
            }
            Some(_) => {}
            None => stale.push((s.to_string(), v.to_string())),
        }
    }
    for s in STATES {
        if states.contains_key(s) || opts.iter().any(|o| o.state == Some(s)) {
            continue;
        }
        let free: Vec<usize> = (0..opts.len())
            .filter(|i| opts[*i].mappable && opts[*i].state.is_none())
            .collect();
        let names: Vec<String> = free.iter().map(|i| opts[*i].name.clone()).collect();
        if let Some(k) = alias_index(&names, s) {
            let i = free[k];
            opts[i].state = Some(s);
            opts[i].mapped_by = MappedBy::Alias;
        }
    }
    for o in opts.iter_mut() {
        if o.state.is_none() && !o.mappable {
            o.mapped_by = MappedBy::Unmapped;
        }
    }
    stale
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = "# per clone\nversion: 1\ndefault: app\nboards:\n  app:\n    platform: github\n    owner: acme   # the org\n    number: 3\n    states:\n      todo: Backlog\n      doing: \"In progress\"\n      done: Done\n  web:\n    platform: gitlab\n    project: grp/proj\n    board: 12\n    states:\n      todo: 'workflow::todo'\n      done: closed\n";

    fn opt(id: &str, name: &str) -> Opt {
        Opt {
            id: id.into(),
            name: name.into(),
            state: None,
            mapped_by: MappedBy::Unmapped,
            mappable: true,
            builtin: false,
        }
    }

    #[test]
    fn a_good_file_parses() {
        let m = parse(GOOD).unwrap();
        assert_eq!(m.default.as_deref(), Some("app"));
        assert_eq!(m.boards.len(), 2);
        let a = &m.boards[0];
        assert_eq!(
            (a.owner.as_str(), a.number, a.field.as_str()),
            ("acme", 3, "Status")
        );
        assert_eq!(a.states["doing"], "In progress");
        let w = &m.boards[1];
        assert_eq!((w.mode.as_str(), w.board), ("labels", 12));
        assert_eq!(w.states["todo"], "workflow::todo");
    }

    #[test]
    fn refusals_name_the_line() {
        let cases: [(&str, &str); 14] = [
            ("version: 1\nboards:\n\tapp:\n", "line 3: tabs"),
            ("version: 1\nboards: &x\n", "line 2: anchors"),
            ("version: 1\nboards: {a: b}\n", "line 2: flow style"),
            ("version: 1\nboards: |\n", "line 2: block scalars"),
            ("version: 1\nversion: 1\n", "line 2: duplicate key"),
            ("version: 1\nzzz: 1\nboards:\n", "line 2: unknown key"),
            (
                "version: 1\nboards:\n  a:\n    platform: github\n    owner: o\n    number: 1\n    color: red\n",
                "line 7: unknown key",
            ),
            (
                "version: 1\nboards:\n  a:\n    platform: azure\n",
                "line 4: platform",
            ),
            (
                "version: 1\nboards:\n  a:\n    platform: github\n    owner: o\n    number: 1\n    states:\n      later: X\n",
                "line 8: unknown state",
            ),
            ("version: 1\nboards:\n  - a\n", "line 3: lists"),
            (
                "version: 1\nboards: x\n",
                "line 2: `boards` needs nested keys",
            ),
            (
                "version: 1\nboards:\n  a:\n    platform: github\n    owner: o\n    number: 1\n    field: \"x\n",
                "line 7: unterminated",
            ),
            (
                "version: 1\nboards:\n  a:\n    platform: gitlab\n    project: p\n    board: 1\n    mode: weird\n",
                "line 7: mode",
            ),
            ("boards:\n", "`version: 1` is missing"),
        ];
        for (text, want) in cases {
            let e = parse(text).unwrap_err();
            assert_eq!(e.code, 2, "{text}");
            assert!(e.msg.contains(want), "{text:?}: {}", e.msg);
        }
    }

    #[test]
    fn multi_line_values_and_size_are_refused() {
        let e = parse("version: 1\nboards:\n  a: x\n    b: y\n").unwrap_err();
        assert!(e.msg.contains("line 4"), "{}", e.msg);
        let big = format!("version: 1\nboards:\n# {}\n", "x".repeat(MAX_BYTES));
        assert!(parse(&big).unwrap_err().msg.contains("64 KiB"));
    }

    #[test]
    fn mapping_prefers_the_file_then_aliases() {
        let mut opts = vec![
            opt("1", "Backlog"),
            opt("2", "Building"),
            opt("3", "In Review"),
            opt("4", "Shipped"),
        ];
        let mut states = BTreeMap::new();
        states.insert("doing".to_string(), "Building".to_string());
        states.insert("done".to_string(), "Gone".to_string());
        let stale = apply(&states, &mut opts);
        assert_eq!(stale, vec![("done".to_string(), "Gone".to_string())]);
        assert_eq!(
            (opts[0].state, opts[0].mapped_by),
            (Some("todo"), MappedBy::Alias)
        );
        assert_eq!(
            (opts[1].state, opts[1].mapped_by),
            (Some("doing"), MappedBy::Map)
        );
        assert_eq!(
            (opts[2].state, opts[2].mapped_by),
            (Some("review"), MappedBy::Alias)
        );
        assert_eq!(
            (opts[3].state, opts[3].mapped_by),
            (None, MappedBy::Unmapped)
        );
    }

    #[test]
    fn a_stale_entry_is_never_replaced_by_an_alias() {
        let mut opts = vec![opt("1", "Done")];
        let mut states = BTreeMap::new();
        states.insert("done".to_string(), "Finished".to_string());
        let stale = apply(&states, &mut opts);
        assert_eq!(stale.len(), 1);
        assert_eq!(opts[0].state, None);
    }
}
