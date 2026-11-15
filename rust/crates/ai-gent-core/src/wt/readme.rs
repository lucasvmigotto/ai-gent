//! `plan`: features ready to start, from `specs/README.md`.
use super::{Failed, Out, Res, pad};
use crate::tokens::pyjson::{J, dumps, obj};
use std::path::Path;

/// One row of the feature table.
#[derive(Debug, Clone)]
pub struct Row {
    /// Three digits.
    pub number: String,
    /// The `Feature` cell.
    pub name: String,
    /// Dependency numbers.
    pub depends: Vec<String>,
    /// `frontend`, `backend`, `status`, `qa` cells in header order.
    pub sides: Vec<(String, String)>,
}

const STATES: [(&str, i32); 5] = [
    ("planned", 0),
    ("in progress", 1),
    ("partial", 1),
    ("implemented", 2),
    ("verified", 3),
];

/// Non-overlapping runs of three digits, left to right (`re.findall(r"\d{3}")`).
fn three_digits(s: &str) -> Vec<String> {
    let c: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i + 3 <= c.len() {
        if c[i..i + 3].iter().all(char::is_ascii_digit) {
            out.push(c[i..i + 3].iter().collect());
            i += 3;
        } else {
            i += 1;
        }
    }
    out
}

/// The rows of `specs/README.md` under `root`.
pub fn readme_rows(root: &str) -> Res<Vec<Row>> {
    let path = Path::new(root).join("specs").join("README.md");
    let Ok(bytes) = std::fs::read(&path) else {
        return Err(Failed(
            "no specs/README.md: name the tracks yourself (wt.py add per branch)".into(),
        ));
    };
    let text = String::from_utf8_lossy(&bytes);
    let mut rows = Vec::new();
    let mut header: Option<Vec<String>> = None;
    for line in text.lines() {
        if !line.starts_with('|') {
            header = None;
            continue;
        }
        let cells: Vec<String> = line
            .trim()
            .trim_matches('|')
            .split('|')
            .map(|c| c.trim().to_string())
            .collect();
        let Some(h) = &header else {
            header = Some(cells.iter().map(|c| c.to_lowercase()).collect());
            continue;
        };
        let joined: String = cells.concat();
        let separator = joined.chars().all(|c| "-: ".contains(c));
        if separator || !(cells[0].len() == 3 && cells[0].chars().all(|c| c.is_ascii_digit())) {
            continue;
        }
        // dict(zip(header, cells)): a repeated header keeps its first position, the last value
        let mut row: Vec<(String, String)> = Vec::new();
        for (k, v) in h.iter().zip(cells.iter()) {
            if let Some(slot) = row.iter_mut().find(|(rk, _)| rk == k) {
                slot.1.clone_from(v);
            } else {
                row.push((k.clone(), v.clone()));
            }
        }
        let get = |k: &str| row.iter().find(|(rk, _)| rk == k).map(|(_, v)| v.clone());
        let deps = ["depends on", "depends", "dependencies"]
            .iter()
            .filter_map(|k| get(k))
            .find(|v| !v.is_empty())
            .unwrap_or_default();
        rows.push(Row {
            number: cells[0].clone(),
            name: get("feature").unwrap_or_default(),
            depends: three_digits(&deps),
            sides: row
                .iter()
                .filter(|(k, _)| ["frontend", "backend", "status", "qa"].contains(&k.as_str()))
                .cloned()
                .collect(),
        });
    }
    Ok(rows)
}

/// The status word of a cell as a level, `None` for N/A, a dash, retired.
#[must_use]
pub fn level(value: &str) -> Option<i32> {
    let v = value.to_lowercase();
    let v = v.trim_matches(|c| "*_ ".contains(c));
    STATES
        .iter()
        .find(|(w, _)| v.starts_with(w))
        .map(|(_, n)| *n)
}

struct Item<'a> {
    row: &'a Row,
    todo: Vec<(String, String)>,
    waiting: Vec<String>,
}

fn row_json(i: &Item) -> J {
    let strs = |v: &[(String, String)]| {
        J::Obj(
            v.iter()
                .map(|(k, x)| (k.clone(), J::s(x.clone())))
                .collect(),
        )
    };
    obj(vec![
        ("number", J::s(i.row.number.clone())),
        ("name", J::s(i.row.name.clone())),
        ("depends", J::strs(&i.row.depends)),
        ("sides", strs(&i.row.sides)),
        ("todo", strs(&i.todo)),
        ("waiting", J::strs(&i.waiting)),
    ])
}

/// `plan [--json]` for the checkout at `here`.
pub fn plan(here: &str, json: bool, out: &mut Out) -> Res<()> {
    let rows = readme_rows(here)?;
    let built = |n: &str| -> bool {
        let Some(r) = rows.iter().rev().find(|r| r.number == n) else {
            return false;
        };
        let levels: Vec<i32> = r.sides.iter().filter_map(|(_, v)| level(v)).collect();
        levels.iter().min().is_some_and(|m| *m >= 2)
    };
    let (mut ready, mut blocked, mut done) = (Vec::new(), Vec::new(), Vec::new());
    for r in &rows {
        let todo: Vec<(String, String)> = r
            .sides
            .iter()
            .filter(|(_, v)| level(v).is_some_and(|l| l < 2))
            .cloned()
            .collect();
        if todo.is_empty() {
            done.push(r.number.clone());
            continue;
        }
        let waiting: Vec<String> = r.depends.iter().filter(|d| !built(d)).cloned().collect();
        let item = Item {
            row: r,
            todo,
            waiting,
        };
        if item.waiting.is_empty() {
            ready.push(item)
        } else {
            blocked.push(item)
        }
    }
    if json {
        let j = obj(vec![
            ("ready", J::Arr(ready.iter().map(row_json).collect())),
            ("blocked", J::Arr(blocked.iter().map(row_json).collect())),
            ("done", J::strs(&done)),
        ]);
        out.line(dumps(&j, true, Some(2)));
        return Ok(());
    }
    out.line("ready to start in parallel (dependencies Implemented):");
    for x in &ready {
        let todo: Vec<String> = x.todo.iter().map(|(k, v)| format!("{k} {v}")).collect();
        out.line(format!(
            "  {} {} {}",
            x.row.number,
            pad(&x.row.name, 28),
            todo.join(", ")
        ));
    }
    if ready.is_empty() {
        out.line("  (none)");
    }
    for x in &blocked {
        out.line(format!(
            "waiting  {} {} on {}",
            x.row.number,
            pad(&x.row.name, 28),
            x.waiting.join(", ")
        ));
    }
    out.line(format!(
        "\n{} feature(s) already Implemented or Verified. Split a ready feature into frontend, backend and QA tracks only once its contract is settled; share nothing else without an owner.",
        done.len()
    ));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const README: &str = "# Features\n\n| # | Feature | Priority | Depends on | Frontend | Backend |\n|---|---|---|---|---|---|\n| 001 | auth | P0 | \u{2014} | Implemented | Verified |\n| 002 | orgs | P0 | 001 | Planned | In progress |\n| 003 | billing | P1 | 002 | Planned | Planned |\n| 004 | search | P2 | 001 | N/A | Planned |\n| 005 | reports | P2 | \u{2014} | Verified | Verified |\n";

    fn plan_of(text: &str) -> String {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir(d.path().join("specs")).unwrap();
        std::fs::write(d.path().join("specs/README.md"), text).unwrap();
        let mut out = Out::default();
        plan(d.path().to_str().unwrap(), true, &mut out).unwrap();
        out.stdout
    }

    #[test]
    fn levels_match_by_prefix() {
        assert_eq!(level("**Implemented** (v2)"), Some(2));
        assert_eq!(level("In progress"), Some(1));
        assert_eq!(level("N/A"), None);
        assert_eq!(level("\u{2014}"), None);
        assert_eq!(level("retired"), None);
    }

    #[test]
    fn digits_are_taken_three_at_a_time() {
        assert_eq!(three_digits("001, 002 and 1234"), ["001", "002", "123"]);
    }

    #[test]
    fn the_plan_splits_ready_blocked_done() {
        let j = crate::tokens::pyjson::parse(&plan_of(README)).unwrap();
        let nums = |k: &str| match j.get(k) {
            Some(J::Arr(a)) => a
                .iter()
                .map(|x| x.get("number").and_then(J::as_str).unwrap().to_string())
                .collect::<Vec<_>>(),
            _ => vec![],
        };
        assert_eq!(nums("ready"), ["002", "004"]);
        assert_eq!(nums("blocked"), ["003"]);
        assert_eq!(j.get("done"), Some(&J::strs(&["001", "005"])));
    }

    #[test]
    fn a_missing_readme_is_a_refusal() {
        let d = tempfile::tempdir().unwrap();
        let e = plan(d.path().to_str().unwrap(), false, &mut Out::default()).unwrap_err();
        assert!(e.0.starts_with("no specs/README.md: "));
    }
}
