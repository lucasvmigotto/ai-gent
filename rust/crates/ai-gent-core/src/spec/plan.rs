//! `build_plan` of `specsync.py`: the items a feature implies, and what changed since the last sync.
use super::digest::digest;
use super::html::to_html;
use super::parse::{
    RETIRED, Story, Task, feature_state, parse_spec, parse_tasks, status_word, tasks_state,
};
use super::readme::readme_row;
use super::tracker::{Tracked, Tracker, read_tracker};
use crate::tokens::pyjson::{J, obj};
use std::path::Path;

/// One tracker item of a plan.
#[derive(Debug, Clone)]
pub struct Item {
    /// `feature` or `US<n>`.
    pub key: String,
    /// `feature` or `story`.
    pub kind: &'static str,
    /// Title.
    pub title: String,
    /// todo, doing, review or done.
    pub state: String,
    /// `feature` for stories.
    pub parent: Option<&'static str>,
    /// Stories only.
    pub priority: Option<String>,
    /// Markdown body.
    pub body: String,
    /// Content digest.
    pub hash: String,
    /// HTML body.
    pub body_html: String,
    /// What `tracker.md` holds for the key.
    pub existing: Option<Tracked>,
    /// create, none, or update / state joined with `+`.
    pub action: String,
}

/// A feature's plan.
#[derive(Debug, Clone)]
pub struct Plan {
    /// Directory name.
    pub feature: String,
    /// Retired in the README.
    pub retired: bool,
    /// Directory path, posix.
    pub path: String,
    /// Platform from `tracker.md`.
    pub platform: String,
    /// README statuses (without notes).
    pub statuses: Vec<(String, String)>,
    /// The items.
    pub items: Vec<Item>,
    /// Keys in `tracker.md` that the specs no longer have.
    pub orphans: Vec<String>,
}

fn marker(path: &str, key: &str) -> String {
    format!("<!-- ai-gent:sync {path} {key} -->")
}

fn checklist(tasks: &[&Task]) -> String {
    tasks
        .iter()
        .map(|t| {
            format!(
                "- [{}] {} {}{}",
                if t.done { 'x' } else { ' ' },
                t.id,
                t.text,
                if t.layer.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", t.layer)
                }
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn read_opt(p: &Path) -> Option<String> {
    crate::tokens::util::read_text(p).ok()
}

/// Cap a story's text like Python: 6000 characters, cut at the last newline, then an ellipsis.
fn cap_text(text: &str) -> String {
    if text.chars().count() <= 6000 {
        return text.to_string();
    }
    let cut: String = text.chars().take(6000).collect();
    let head = cut.rsplit_once('\n').map_or(cut.as_str(), |(h, _)| h);
    format!("{head}\n\n\u{2026}")
}

/// The plan for a feature directory. `rel` is the posix path shown in markers; `specs_dir` holds `README.md`.
#[must_use]
pub fn build_plan(fdir: &Path, rel: &str, specs_dir: &Path) -> Plan {
    let name_full = fdir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let (number, name) = name_full
        .split_once('-')
        .map_or((name_full.as_str(), name_full.as_str()), |(a, b)| (a, b));
    let (title, stories) = parse_spec(read_opt(&fdir.join("spec.md")).as_deref());
    let tasks = parse_tasks(read_opt(&fdir.join("tasks.md")).as_deref());
    let row = readme_row(read_opt(&specs_dir.join("README.md")).as_deref(), number);
    let statuses: Vec<(String, String)> =
        row.iter().filter(|(k, _)| k != "notes").cloned().collect();
    let retired = !row.is_empty()
        && row
            .iter()
            .all(|(_, v)| RETIRED.contains(&status_word(v)) || status_word(v).is_empty())
        && row.iter().any(|(_, v)| RETIRED.contains(&status_word(v)));
    let fstate = feature_state(&statuses, &tasks);
    let tracker: Tracker = read_tracker(read_opt(&fdir.join("tracker.md")).as_deref());
    let shown_title = if title.is_empty() {
        name
    } else {
        title.as_str()
    };

    let known: Vec<i64> = stories.iter().map(|s| s.n).collect();
    let loose: Vec<&Task> = tasks
        .iter()
        .filter(|t| t.story.is_none_or(|n| !known.contains(&n)))
        .collect();
    let mut fbody = vec![
        marker(rel, "feature"),
        format!("Feature **{number} {shown_title}** \u{2014} `{rel}/spec.md`"),
        String::new(),
    ];
    if !statuses.is_empty() {
        let parts: Vec<String> = statuses
            .iter()
            .map(|(k, v)| format!("{}: {v}", capitalize(k)))
            .collect();
        fbody.push(parts.join(" \u{b7} "));
        fbody.push(String::new());
    }
    if !stories.is_empty() {
        fbody.push("Stories:".to_string());
        for s in &stories {
            fbody.push(format!(
                "- US{} {}{}",
                s.n,
                s.title,
                if s.priority.is_empty() {
                    String::new()
                } else {
                    format!(" ({})", s.priority)
                }
            ));
        }
        fbody.push(String::new());
    }
    if !loose.is_empty() {
        fbody.push("Tasks outside the stories:".to_string());
        fbody.push(checklist(&loose));
        fbody.push(String::new());
    }
    let mut items = vec![Item {
        key: "feature".into(),
        kind: "feature",
        title: format!("[{number}] {shown_title}"),
        state: fstate.to_string(),
        parent: None,
        priority: None,
        body: format!("{}\n", fbody.join("\n").trim()),
        hash: String::new(),
        body_html: String::new(),
        existing: None,
        action: String::new(),
    }];

    for s in &stories {
        items.push(story_item(s, &tasks, fstate, number, shown_title, rel));
    }

    for item in &mut items {
        item.hash = digest(&[&item.title, &item.body]);
        item.body_html = to_html(&item.body);
        let prev = tracker.get(&item.key).cloned();
        item.action = match &prev {
            Some(p) if !p.item_ref.is_empty() => {
                let mut changes = Vec::new();
                if p.hash != item.hash {
                    changes.push("update");
                }
                if p.state != item.state {
                    changes.push("state");
                }
                if changes.is_empty() {
                    "none".to_string()
                } else {
                    changes.join("+")
                }
            }
            _ => "create".to_string(),
        };
        item.existing = prev;
    }
    if retired {
        items.clear();
    }
    let mut orphans: Vec<String> = tracker
        .items
        .iter()
        .map(|(k, _)| k.clone())
        .filter(|k| !items.iter().any(|i| &i.key == k))
        .collect();
    orphans.sort();
    Plan {
        feature: name_full.clone(),
        retired,
        path: rel.to_string(),
        platform: tracker.platform,
        statuses,
        items,
        orphans,
    }
}

fn story_item(
    s: &Story,
    tasks: &[Task],
    fstate: &str,
    number: &str,
    title: &str,
    rel: &str,
) -> Item {
    let st: Vec<&Task> = tasks.iter().filter(|t| t.story == Some(s.n)).collect();
    let owned: Vec<Task> = st.iter().map(|t| (*t).clone()).collect();
    let state = if fstate == "done" {
        "done"
    } else if st.is_empty() {
        fstate
    } else {
        tasks_state(&owned)
    };
    let text = cap_text(&s.text);
    let mut body = vec![
        marker(rel, &format!("US{}", s.n)),
        format!(
            "User story {} of **{number} {title}**{} \u{2014} `{rel}/spec.md`",
            s.n,
            if s.priority.is_empty() {
                String::new()
            } else {
                format!(" \u{b7} {}", s.priority)
            }
        ),
        String::new(),
        text,
        String::new(),
    ];
    if !st.is_empty() {
        body.push("### Tasks".to_string());
        body.push(checklist(&st));
        body.push(String::new());
    }
    Item {
        key: format!("US{}", s.n),
        kind: "story",
        title: format!("[{number}] US{}: {}", s.n, s.title),
        state: state.to_string(),
        parent: Some("feature"),
        priority: Some(s.priority.clone()),
        body: format!("{}\n", body.join("\n").trim()),
        hash: String::new(),
        body_html: String::new(),
        existing: None,
        action: String::new(),
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next().map_or_else(String::new, |f| {
        f.to_uppercase().collect::<String>() + &c.as_str().to_lowercase()
    })
}

fn tracked_json(t: &Tracked) -> J {
    obj(vec![
        ("ref", J::s(&t.item_ref)),
        ("id", J::s(&t.id)),
        ("state", J::s(&t.state)),
        ("hash", J::s(&t.hash)),
        ("url", J::s(&t.url)),
    ])
}

impl Plan {
    /// The `--json` shape of `build_plan`.
    #[must_use]
    pub fn to_json(&self) -> J {
        let items = self
            .items
            .iter()
            .map(|i| {
                let mut m = vec![
                    ("key", J::s(&i.key)),
                    ("kind", J::s(i.kind)),
                    ("title", J::s(&i.title)),
                    ("state", J::s(&i.state)),
                    ("parent", J::opt_s(i.parent)),
                ];
                if let Some(p) = &i.priority {
                    m.push(("priority", J::s(p)));
                }
                m.push(("body", J::s(&i.body)));
                m.push(("hash", J::s(&i.hash)));
                m.push(("body_html", J::s(&i.body_html)));
                m.push((
                    "existing",
                    i.existing.as_ref().map_or(J::Null, tracked_json),
                ));
                m.push(("action", J::s(&i.action)));
                obj(m)
            })
            .collect();
        obj(vec![
            ("feature", J::s(&self.feature)),
            ("retired", J::Bool(self.retired)),
            ("path", J::s(&self.path)),
            ("platform", J::s(&self.platform)),
            (
                "statuses",
                J::Obj(
                    self.statuses
                        .iter()
                        .map(|(k, v)| (k.clone(), J::s(v)))
                        .collect(),
                ),
            ),
            ("items", J::Arr(items)),
            ("orphans", J::strs(&self.orphans)),
        ])
    }
}
