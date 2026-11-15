//! Parsers of `specsync.py`: `spec.md` stories, `tasks.md` checkboxes, states.
//!
//! The Python patterns are matched by hand here (the workspace `regex` has no Unicode tables); each
//! function says which pattern it stands for.
use crate::tokens::util::{is_word, splitlines};

/// A user story of a feature.
#[derive(Debug, Clone, PartialEq)]
pub struct Story {
    /// The story number.
    pub n: i64,
    /// Its title.
    pub title: String,
    /// `P1`... or empty.
    pub priority: String,
    /// The text under the heading.
    pub text: String,
}

/// A task line of `tasks.md`.
#[derive(Debug, Clone, PartialEq)]
pub struct Task {
    /// `T001`...
    pub id: String,
    /// Checked.
    pub done: bool,
    /// The story it belongs to (a `[USn]` tag, else the phase heading).
    pub story: Option<i64>,
    /// `Frontend`, `Backend`, `QA` or empty.
    pub layer: String,
    /// The description without tags.
    pub text: String,
}

pub(crate) const STATES: [&str; 4] = ["todo", "doing", "review", "done"];
const STATUS_STATE: [(&str, &str); 5] = [
    ("planned", "todo"),
    ("in progress", "doing"),
    ("partial", "doing"),
    ("implemented", "review"),
    ("verified", "done"),
];
pub(crate) const RETIRED: [&str; 6] = [
    "retired",
    "superseded",
    "removed",
    "dropped",
    "cancelled",
    "canceled",
];

fn is_sep(c: char) -> bool {
    matches!(c, '-' | '\u{2014}' | '\u{2013}' | ':')
}

fn digits(s: &str) -> (&str, &str) {
    let n = s.chars().take_while(char::is_ascii_digit).count();
    s.split_at(n)
}

/// Case-insensitive ASCII prefix strip.
fn strip_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    let head = s.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix)
        .then(|| &s[prefix.len()..])
}

/// `\(Priority:\s*(P\d+)\)` at the very end of `s`: (start index, priority as written).
fn priority_suffix(s: &str) -> Option<(usize, String)> {
    let body = s.strip_suffix(')')?;
    let mut from = 0;
    let mut found = None;
    while let Some(i) = body[from..].find('(') {
        let at = from + i;
        if let Some(rest) = strip_ci(&body[at + 1..], "priority:") {
            let rest = rest.trim_start();
            if let Some(r) = rest.strip_prefix(['P', 'p']) {
                let (d, tail) = digits(r);
                if !d.is_empty() && tail.is_empty() {
                    found = Some((at, format!("{}{d}", &rest[..1])));
                    break;
                }
            }
        }
        from = at + 1;
    }
    found
}

/// `STORY_HEAD_RE`: `### User Story N - Title (Priority: Pn)`, case-insensitive.
#[must_use]
pub fn story_head(line: &str) -> Option<(i64, String, String)> {
    let rest = line.strip_prefix("###")?;
    let t = rest.trim_start();
    if t.len() == rest.len() {
        return None;
    }
    let t = strip_ci(t, "user story")?;
    let u = t.trim_start();
    if u.len() == t.len() {
        return None;
    }
    let (d, after) = digits(u);
    if d.is_empty() {
        return None;
    }
    let after = after.trim_start();
    let mut chars = after.chars();
    if !chars.next().is_some_and(is_sep) {
        return None;
    }
    let raw = chars.as_str();
    let body = raw.trim_start().trim_end();
    let n: i64 = d.parse().ok()?;
    if body.is_empty() {
        // `.+?` needs a character: the regex gives back one whitespace character as the title.
        return (!raw.is_empty()).then(|| (n, String::new(), String::new()));
    }
    if let Some((at, p)) = priority_suffix(body) {
        let title = body[..at].trim_end();
        if !title.is_empty() {
            return Some((n, title.to_string(), p));
        }
    }
    Some((n, body.to_string(), String::new()))
}

fn first_title(line: &str) -> String {
    // re.sub(r"^#\s+(Feature Specification:\s*)?(\d{3}\s*[-—–:]\s*)?", "", line).strip()
    let mut s = line.strip_prefix('#').unwrap_or(line).trim_start();
    if let Some(r) = s.strip_prefix("Feature Specification:") {
        s = r.trim_start();
    }
    let (d, after) = digits(s);
    if d.len() >= 3 {
        // \d{3} takes exactly three digits; the separator must follow (after optional spaces)
        let rest = s[3..].trim_start();
        if let Some(c) = rest.chars().next().filter(|c| is_sep(*c)) {
            s = rest[c.len_utf8()..].trim_start();
        }
    }
    let _ = after;
    s.trim().to_string()
}

/// The feature's title and its user stories with their text.
#[must_use]
pub fn parse_spec(text: Option<&str>) -> (String, Vec<Story>) {
    let Some(text) = text else {
        return (String::new(), Vec::new());
    };
    let mut title = String::new();
    let mut stories: Vec<Story> = Vec::new();
    let mut texts: Vec<Vec<&str>> = Vec::new();
    let mut cur: Option<usize> = None;
    for line in splitlines(text) {
        if title.is_empty() && line.starts_with("# ") {
            title = first_title(line);
        }
        if let Some((n, t, p)) = story_head(line) {
            stories.push(Story {
                n,
                title: t.trim().to_string(),
                priority: p,
                text: String::new(),
            });
            texts.push(Vec::new());
            cur = Some(stories.len() - 1);
            continue;
        }
        if let Some(i) = cur {
            if line.starts_with("## ") || line.starts_with("### ") {
                cur = None;
            } else {
                texts[i].push(line);
            }
        }
    }
    for (s, t) in stories.iter_mut().zip(texts) {
        s.text = t.join("\n").trim().to_string();
    }
    if stories.is_empty() {
        stories = listed_stories(text);
    }
    (title, stories)
}

/// `LISTED_RE`: `1. **Title**: text`.
fn listed_item(line: &str) -> Option<(i64, String, String)> {
    let (d, rest) = digits(line);
    if d.is_empty() {
        return None;
    }
    let rest = rest.strip_prefix('.')?;
    let t = rest.trim_start();
    if t.len() == rest.len() {
        return None;
    }
    let t = t.strip_prefix("**")?;
    let skip = t.chars().next().map_or(0, char::len_utf8);
    let end = t.get(skip..)?.find("**")? + skip;
    let title = &t[..end];
    let mut r = t[end + 2..].trim_start();
    if let Some(c) = r
        .chars()
        .next()
        .filter(|c| matches!(c, ':' | '\u{2014}' | '\u{2013}' | '-'))
    {
        r = r[c.len_utf8()..].trim_start();
    }
    Some((d.parse().ok()?, title.to_string(), r.to_string()))
}

/// `project:introspec`'s shape: a numbered list under `## Stories`.
#[must_use]
pub fn listed_stories(text: &str) -> Vec<Story> {
    let mut stories: Vec<Story> = Vec::new();
    let mut texts: Vec<Vec<String>> = Vec::new();
    let mut cur: Option<usize> = None;
    let mut inside = false;
    for line in splitlines(text) {
        if let Some(h) = line.strip_prefix("## ") {
            let l = h.trim().to_lowercase();
            inside = l.starts_with("stories") || l.starts_with("user stories");
            cur = None;
            continue;
        }
        if !inside {
            continue;
        }
        if let Some((n, title, rest)) = listed_item(line) {
            let first = rest.trim();
            stories.push(Story {
                n,
                title: title.trim().trim_end_matches(':').to_string(),
                priority: String::new(),
                text: String::new(),
            });
            texts.push(if first.is_empty() {
                Vec::new()
            } else {
                vec![first.to_string()]
            });
            cur = Some(stories.len() - 1);
        } else if let Some(i) =
            cur.filter(|_| line.starts_with([' ', '\t']) || line.trim().is_empty())
        {
            texts[i].push(if line.starts_with("   ") {
                line[3..].to_string()
            } else {
                line.trim().to_string()
            });
        } else if !line.trim().is_empty() {
            cur = None;
        }
    }
    for (s, t) in stories.iter_mut().zip(texts) {
        s.text = t.join("\n").trim().to_string();
    }
    stories
}

/// `TASK_RE`: `- [x] T001a text` -> (checked, id, text).
fn task_line(line: &str) -> Option<(bool, String, String)> {
    let s = line.trim_start().strip_prefix("- [")?;
    let mark = s.chars().next()?;
    if !matches!(mark, ' ' | 'x' | 'X') {
        return None;
    }
    let s = s[1..].strip_prefix("] T")?;
    let (d, mut after) = digits(s);
    if d.is_empty() {
        return None;
    }
    let mut id_len = 1 + d.len();
    if let Some(c) = after.chars().next().filter(char::is_ascii_lowercase) {
        id_len += 1;
        after = &after[1..];
        let _ = c;
    }
    if after.chars().next().is_some_and(is_word) {
        return None;
    }
    let id_full = format!("T{}", &s[..id_len - 1]);
    Some((mark != ' ', id_full, after.trim_start().to_string()))
}

/// `[US3]` tag number, if any (`re.search(r"\[US(\d+)\]", text)`).
fn us_tag(text: &str) -> Option<i64> {
    let mut from = 0;
    while let Some(i) = text[from..].find("[US") {
        let at = from + i + 3;
        let (d, rest) = digits(&text[at..]);
        if !d.is_empty() && rest.starts_with(']') {
            return d.parse().ok();
        }
        from = at;
    }
    None
}

/// `re.sub(r"\[(P|US\d+|TUI|CLI)\]\s*", "", text).strip()`.
fn strip_tags(text: &str) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i < text.len() {
        if text[i..].starts_with('[') {
            let rest = &text[i + 1..];
            let hit = ["P]", "TUI]", "CLI]"]
                .iter()
                .find_map(|t| rest.starts_with(t).then_some(t.len()))
                .or_else(|| {
                    let r = rest.strip_prefix("US")?;
                    let (d, tail) = digits(r);
                    (!d.is_empty() && tail.starts_with(']')).then(|| 2 + d.len() + 1)
                });
            if let Some(len) = hit {
                let after = &rest[len..];
                i = text.len() - after.trim_start().len();
                continue;
            }
        }
        let c = text[i..].chars().next().unwrap_or(' ');
        out.push(c);
        i += c.len_utf8();
    }
    out.trim().to_string()
}

/// `LAYER_RE`: `## Frontend|Backend|QA` followed by a word boundary.
fn layer_heading(line: &str) -> Option<&'static str> {
    let rest = line.strip_prefix("##")?.trim_start();
    for (name, canon) in [
        ("frontend", "Frontend"),
        ("backend", "Backend"),
        ("qa", "QA"),
    ] {
        if let Some(after) = strip_ci(rest, name) {
            if !after.chars().next().is_some_and(is_word) {
                return Some(canon);
            }
        }
    }
    None
}

/// `PHASE_STORY_RE`: `## ... User Story N` (2 or 3 hashes).
fn phase_story(line: &str) -> Option<i64> {
    let n = line.chars().take_while(|c| *c == '#').count();
    if !(2..=3).contains(&n) {
        return None;
    }
    let rest = &line[n..];
    let t = rest.trim_start();
    if t.len() == rest.len() {
        return None;
    }
    let lower = t.to_ascii_lowercase();
    let mut from = 0;
    while let Some(i) = lower[from..].find("user story") {
        let at = from + i + "user story".len();
        let r = t[at..].trim_start();
        if r.len() < t[at..].len() {
            let (d, tail) = digits(r);
            if !d.is_empty() && !tail.chars().next().is_some_and(is_word) {
                return d.parse().ok();
            }
        }
        from = at;
    }
    None
}

/// Tasks with their story (a `[USn]` tag, else the phase heading) and layer.
#[must_use]
pub fn parse_tasks(text: Option<&str>) -> Vec<Task> {
    let Some(text) = text else {
        return Vec::new();
    };
    let mut tasks = Vec::new();
    let mut story: Option<i64> = None;
    let mut layer = String::new();
    for line in splitlines(text) {
        if line.starts_with('#') {
            if let Some(l) = layer_heading(line) {
                layer = l.to_string();
                story = None;
                continue;
            }
            story = match phase_story(line) {
                Some(n) => Some(n),
                None if line.starts_with("## ") || line.starts_with("### Phase") => None,
                None => story,
            };
            continue;
        }
        let Some((done, id, rest)) = task_line(line) else {
            continue;
        };
        tasks.push(Task {
            id,
            done,
            story: us_tag(&rest).or(story),
            layer: layer.clone(),
            text: strip_tags(&rest),
        });
    }
    tasks
}

/// `Implemented (containers; ...)` -> `implemented`; empty when no known word starts it.
#[must_use]
pub fn status_word(value: &str) -> &'static str {
    let v = value.to_lowercase();
    let v = v.trim_matches(|c| matches!(c, '*' | '_' | ' '));
    for w in STATUS_STATE.iter().map(|(w, _)| *w).chain(RETIRED) {
        if v.starts_with(w) {
            return w;
        }
    }
    ""
}

fn state_of_word(word: &str) -> Option<&'static str> {
    STATUS_STATE
        .iter()
        .find(|(w, _)| *w == word)
        .map(|(_, s)| *s)
}

/// The least advanced state implied by the statuses, else the tasks' state.
#[must_use]
pub fn feature_state(statuses: &[(String, String)], tasks: &[Task]) -> &'static str {
    let states: Vec<&str> = statuses
        .iter()
        .filter_map(|(_, v)| state_of_word(status_word(v)))
        .collect();
    if let Some(min) = states
        .iter()
        .min_by_key(|s| STATES.iter().position(|x| x == *s))
    {
        return min;
    }
    tasks_state(tasks)
}

/// todo / doing / review from checked tasks.
#[must_use]
pub fn tasks_state(tasks: &[Task]) -> &'static str {
    if tasks.is_empty() || !tasks.iter().any(|t| t.done) {
        "todo"
    } else if tasks.iter().all(|t| t.done) {
        "review"
    } else {
        "doing"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn story_headings() {
        assert_eq!(
            story_head("### User Story 1 - Export a timesheet (Priority: P1)"),
            Some((1, "Export a timesheet".into(), "P1".into()))
        );
        assert_eq!(
            story_head("### user story 2 \u{2014} Schedule exports (priority: p2)  "),
            Some((2, "Schedule exports".into(), "p2".into()))
        );
        assert_eq!(
            story_head("### User Story 3: Plain"),
            Some((3, "Plain".into(), String::new()))
        );
        assert_eq!(story_head("## User Story 3 - x"), None);
        assert_eq!(story_head("### User Story - x"), None);
    }

    #[test]
    fn task_lines() {
        let t = parse_tasks(Some(
            "## Phase 2: User Story 1 - X (P1)\n- [x] T002 [P] Test it\n- [ ] T003a [US2] Go\n- [ ] T004x9 no\n- [ ] T5\n",
        ));
        assert_eq!(t.len(), 3);
        assert_eq!(
            (t[0].id.as_str(), t[0].done, t[0].story),
            ("T002", true, Some(1))
        );
        assert_eq!(t[0].text, "Test it");
        assert_eq!((t[1].id.as_str(), t[1].story), ("T003a", Some(2)));
        assert_eq!(t[2].id, "T5");
    }

    #[test]
    fn status_words() {
        assert_eq!(status_word("**Implemented** (x)"), "implemented");
        assert_eq!(status_word("In progress"), "in progress");
        assert_eq!(status_word("\u{2014}"), "");
        assert_eq!(status_word("Retired by BCR-1"), "retired");
    }
}
