//! `readme_row` of `specsync.py`: a feature's statuses from `specs/README.md`.
use crate::tokens::util::splitlines;

fn cells_of(line: &str) -> Vec<String> {
    line.trim()
        .trim_matches('|')
        .split('|')
        .map(|c| c.trim().to_string())
        .collect()
}

/// `{column: value}` for the row of feature `number`, restricted to the status columns, in header
/// order. Empty when the file or the row is missing.
#[must_use]
pub fn readme_row(text: Option<&str>, number: &str) -> Vec<(String, String)> {
    let Some(text) = text else {
        return Vec::new();
    };
    let mut header: Option<Vec<String>> = None;
    for line in splitlines(text) {
        if !line.starts_with('|') {
            if line.trim().is_empty() {
                header = None;
            }
            continue;
        }
        let cells = cells_of(line);
        let Some(h) = &header else {
            header = Some(cells.iter().map(|c| c.to_lowercase()).collect());
            continue;
        };
        if cells.concat().chars().all(|c| matches!(c, '-' | ':' | ' ')) {
            continue;
        }
        let first = cells.first().map_or("", String::as_str);
        if first.trim_start_matches('0') == number.trim_start_matches('0') || first == number {
            let mut row: Vec<(String, String)> = Vec::new();
            for (k, v) in h.iter().zip(&cells) {
                if !["frontend", "backend", "status", "qa", "notes"].contains(&k.as_str()) {
                    continue;
                }
                if let Some(slot) = row.iter_mut().find(|(rk, _)| rk == k) {
                    slot.1 = v.clone();
                } else {
                    row.push((k.clone(), v.clone()));
                }
            }
            return row;
        }
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_by_number_in_header_order() {
        let t = "# Features\n\n| # | Feature | Frontend | Backend |\n|---|---|---|---|\n| 012 | export | In progress | Planned |\n\n| # | Notes |\n|---|---|\n| 013 | **Retired** |\n";
        assert_eq!(
            readme_row(Some(t), "012"),
            vec![
                ("frontend".to_string(), "In progress".to_string()),
                ("backend".into(), "Planned".into())
            ]
        );
        assert_eq!(
            readme_row(Some(t), "013"),
            vec![("notes".to_string(), "**Retired**".to_string())]
        );
        assert!(readme_row(Some(t), "099").is_empty());
        assert!(readme_row(None, "012").is_empty());
    }
}
