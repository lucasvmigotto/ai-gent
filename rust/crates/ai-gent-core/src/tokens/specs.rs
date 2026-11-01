//! A project's `specs/NNN-*/tasks.md` and `cards.md`: task lines, the cards table and each card's
//! Tokens, Tags and dispatch description.
use super::tags::{Tags, parse_tags};
use super::util::{OMap, expand_ids, is_word, parse_tok, read_text, splitlines};
use std::path::{Path, PathBuf};

/// A task line of `tasks.md`.
#[derive(Debug, Clone)]
pub struct Task {
    /// Zero-based line index in the file.
    pub line: usize,
    /// backend, frontend or qa, from the nearest heading.
    pub layer: Option<String>,
    /// `Executor:` value.
    pub executor: Option<String>,
    /// `Effort:` value.
    pub effort: Option<String>,
    /// `Tokens: projected` value.
    pub projected: Option<i64>,
    /// `Tags:` value.
    pub tags: Tags,
}

/// A card of `cards.md`.
#[derive(Debug, Clone, Default)]
pub struct Card {
    /// Task ids.
    pub tasks: Vec<String>,
    /// Executor role.
    pub executor: Option<String>,
    /// Reviewer role.
    pub reviewer: Option<String>,
    /// low, medium or high.
    pub effort: Option<String>,
    /// Projected tokens.
    pub projected: Option<i64>,
    /// The dispatch description.
    pub dispatch: Option<String>,
    /// The card's tags.
    pub tags: Tags,
}

/// A feature folder.
#[derive(Debug, Clone)]
pub struct Feature {
    /// `001-widget`.
    pub name: String,
    /// `001`.
    pub num: String,
    /// The folder.
    pub dir: PathBuf,
    /// Tasks by id.
    pub tasks: OMap<Task>,
    /// Cards by id.
    pub cards: OMap<Card>,
}

/// Text after the first `key` that `then` accepts (a leftmost `re.search`).
fn search_after<'a, T>(line: &'a str, key: &str, then: impl Fn(&'a str) -> Option<T>) -> Option<T> {
    line.match_indices(key)
        .find_map(|(i, _)| then(&line[i + key.len()..]))
}

fn until<'a>(s: &'a str, stops: &[char]) -> &'a str {
    s.find(|c| stops.contains(&c)).map_or(s, |i| &s[..i])
}

fn word_prefix(s: &str) -> Option<&str> {
    let n: usize = s
        .chars()
        .take_while(|&c| is_word(c))
        .map(char::len_utf8)
        .sum();
    (n > 0).then(|| &s[..n])
}

/// The task id a task line defines: `^(\s*- \[[ xX]\] )(T\d{3,})\b`.
#[must_use]
pub fn task_line_id(line: &str) -> Option<String> {
    let r = line.trim_start().strip_prefix("- [")?;
    let mut it = r.chars();
    let mark = it.next()?;
    if !matches!(mark, ' ' | 'x' | 'X') {
        return None;
    }
    let r = r[1..].strip_prefix("] ")?;
    let r = r.strip_prefix('T')?;
    let n = r.bytes().take_while(u8::is_ascii_digit).count();
    if n < 3 || r[n..].chars().next().is_some_and(is_word) {
        return None;
    }
    Some(format!("T{}", &r[..n]))
}

const LAYER_HEADS: [(&str, &str); 4] = [
    ("backend", "backend"),
    ("frontend", "frontend"),
    ("qa", "qa"),
    ("quality", "qa"),
];

/// Task id to task for a feature's `tasks.md` (the first definition wins).
#[must_use]
pub fn parse_tasks_md(path: &Path) -> OMap<Task> {
    let mut tasks = OMap::new();
    let Ok(text) = read_text(path) else {
        return tasks;
    };
    let mut layer: Option<String> = None;
    for (n, line) in text.split('\n').enumerate() {
        if line.starts_with('#') {
            let head = line.trim_start_matches(['#', ' ']).trim().to_lowercase();
            layer = LAYER_HEADS
                .iter()
                .find(|(key, _)| head.starts_with(key))
                .map(|(_, v)| (*v).to_string())
                .or_else(|| {
                    if line.starts_with("###") {
                        layer.clone()
                    } else {
                        None
                    }
                });
            continue;
        }
        let Some(id) = task_line_id(line) else {
            continue;
        };
        if tasks.contains_key(&id) {
            continue;
        }
        let word_after =
            |key: &str| search_after(line, key, |r| word_prefix(r.trim_start()).map(String::from));
        let projected = search_after(line, "Tokens:", |r| {
            let r = r.trim_start().strip_prefix("projected")?;
            Some(parse_tok(until(r.trim_start(), &['|', '·', '\n'])))
        })
        .flatten();
        let tags = search_after(line, "Tags:", |r| {
            Some(parse_tags(until(r.trim_start(), &['·', '|', '\n'])))
        })
        .unwrap_or_default();
        tasks.set(
            &id,
            Task {
                line: n,
                layer: layer.clone(),
                executor: word_after("Executor:"),
                effort: word_after("Effort:"),
                projected,
                tags,
            },
        );
    }
    tasks
}

fn first_word(s: &str) -> Option<String> {
    s.split_whitespace().next().map(String::from)
}

/// Card id to card for a feature's `cards.md`: the summary table gives the route, each
/// `## Card `id`` section its Tokens, Tags and dispatch description.
#[must_use]
pub fn parse_cards_md(path: &Path) -> OMap<Card> {
    let mut cards: OMap<Card> = OMap::new();
    let Ok(text) = read_text(path) else {
        return cards;
    };
    let mut cols: Option<Vec<String>> = None;
    for line in splitlines(&text) {
        if !line.starts_with('|') {
            cols = None;
            continue;
        }
        let cells: Vec<String> = line
            .trim()
            .trim_matches('|')
            .split('|')
            .map(|c| c.trim().to_string())
            .collect();
        if cells.first().is_some_and(|c| c.to_lowercase() == "card") {
            cols = Some(cells.iter().map(|c| c.to_lowercase()).collect());
            continue;
        }
        let Some(cols) = &cols else { continue };
        if line
            .replace('|', "")
            .trim()
            .chars()
            .all(|c| "-: ".contains(c))
        {
            continue;
        }
        let row = |k: &str| {
            cols.iter()
                .position(|c| c == k)
                .and_then(|i| cells.get(i))
                .map(String::as_str)
        };
        let cid = row("card").unwrap_or("").trim_matches('`').to_string();
        if cid.is_empty() {
            continue;
        }
        cards.set(
            &cid,
            Card {
                tasks: expand_ids(row("tasks").unwrap_or("")),
                executor: row("executor").and_then(first_word),
                reviewer: row("reviewer").map(String::from),
                effort: row("effort").and_then(first_word),
                ..Card::default()
            },
        );
    }
    // Sections: `## Card `id`...\n` up to the next line that starts with `## `.
    let lines: Vec<&str> = text.split('\n').collect();
    let mut i = 0;
    while i < lines.len() {
        let head = lines[i];
        let id = head
            .strip_prefix("## Card `")
            .and_then(|r| r.split_once('`'))
            .map(|(id, _)| id);
        match id {
            Some(id) if !id.is_empty() && i + 1 < lines.len() => {
                let mut j = i + 1;
                while j < lines.len() && !lines[j].starts_with("## ") {
                    j += 1;
                }
                let body = lines[i + 1..j].join("\n");
                let c = cards.entry(id, || Card {
                    tasks: expand_ids(head),
                    ..Card::default()
                });
                c.projected = search_after(&body, "Tokens:", |r| {
                    let r = r.trim_start().strip_prefix("projected")?;
                    Some(parse_tok(until(r.trim_start(), &['|', '·', '`', '\n'])))
                })
                .flatten();
                c.tags = search_after(&body, "Tags:", |r| {
                    Some(parse_tags(until(r.trim_start(), &['·', '|', '`', '\n'])))
                })
                .unwrap_or_default();
                c.dispatch = search_after(&body, "Dispatch description:", |r| {
                    let r = r.trim_start().strip_prefix('`')?;
                    let end = r.find('`')?;
                    (end > 0).then(|| r[..end].to_string())
                });
                if c.tasks.is_empty() {
                    c.tasks = expand_ids(head);
                }
                i = j;
            }
            _ => i += 1,
        }
    }
    cards
}

/// Feature folders `specs/[0-9][0-9][0-9]-*` with a `tasks.md` or `cards.md`.
#[must_use]
pub fn load_features(specs_dir: &Path) -> OMap<Feature> {
    let mut feats = OMap::new();
    let Ok(rd) = std::fs::read_dir(specs_dir) else {
        return feats;
    };
    let mut dirs: Vec<PathBuf> = rd.filter_map(Result::ok).map(|e| e.path()).collect();
    dirs.sort();
    for d in dirs {
        let name = d
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let b = name.as_bytes();
        if !(b.len() >= 4 && b[..3].iter().all(u8::is_ascii_digit) && b[3] == b'-') {
            continue;
        }
        let (tm, cm) = (d.join("tasks.md"), d.join("cards.md"));
        let tasks = if tm.exists() {
            parse_tasks_md(&tm)
        } else {
            OMap::new()
        };
        let cards = if cm.exists() {
            parse_cards_md(&cm)
        } else {
            OMap::new()
        };
        if !tasks.is_empty() || !cards.is_empty() {
            let num = name[..3].to_string();
            feats.set(
                &name.clone(),
                Feature {
                    name,
                    num,
                    dir: d,
                    tasks,
                    cards,
                },
            );
        }
    }
    feats
}

#[cfg(test)]
mod tests {
    use super::*;

    pub const TASKS_MD: &str = "# Tasks\n## Phase 1\n- [ ] T001 [US1] build the widget in `src/a.py` — Executor: builder · Reviewer: thinker · Effort: high\n- [ ] T002 [US1] test the widget — Executor: builder · Reviewer: thinker · Effort: medium\n- [ ] T003 [US1] polish — Executor: builder · Reviewer: thinker · Effort: low\n## Backend\n- [ ] T004 [US1] serve it — Executor: builder · Reviewer: thinker · Effort: medium\n";
    pub const CARDS_MD: &str = "# Cards\n| Card | Tasks | Executor | Reviewer | Effort |\n|---|---|---|---|---|\n| `001-core` | T001, T002 | builder | thinker | high |\n| `001-polish` | T003 | builder | thinker | low |\n\n## Card `001-core`: T001, T002\n\n**Route**: Executor `builder` · Effort `high` · Tags: extend, backend, protocol, rust, tokio, known-pattern, integration, M · Tokens: projected ~400k | used — · Dispatch description: `T001, T002: 001-widget core`\n\n## Card `001-polish`: T003\n\n**Route**: Executor `builder` · Effort `low` · Tokens: projected ~100k | used — · Dispatch description: `T003: 001-widget polish`\n";

    #[test]
    fn tasks_and_cards_parse() {
        let d = tempfile::tempdir().unwrap();
        let f = d.path().join("specs/001-widget");
        std::fs::create_dir_all(&f).unwrap();
        std::fs::write(f.join("tasks.md"), TASKS_MD).unwrap();
        std::fs::write(f.join("cards.md"), CARDS_MD).unwrap();
        let feats = load_features(&d.path().join("specs"));
        let w = feats.get("001-widget").unwrap();
        assert_eq!(
            w.tasks.get("T004").unwrap().layer.as_deref(),
            Some("backend")
        );
        assert_eq!(w.tasks.get("T001").unwrap().effort.as_deref(), Some("high"));
        let core = w.cards.get("001-core").unwrap();
        assert_eq!(
            (core.projected, core.tasks.clone()),
            (Some(400_000), vec!["T001".to_string(), "T002".to_string()])
        );
        assert_eq!(
            core.dispatch.as_deref(),
            Some("T001, T002: 001-widget core")
        );
        assert_eq!(core.effort.as_deref(), Some("high"));
        assert_eq!(w.cards.get("001-polish").unwrap().projected, Some(100_000));
        assert_eq!(task_line_id("  - [x] T123 done").as_deref(), Some("T123"));
        assert_eq!(task_line_id("- [ ] T12 no"), None);
    }
}
