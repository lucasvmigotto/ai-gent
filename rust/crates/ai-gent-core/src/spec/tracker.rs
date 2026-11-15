//! `tracker.md` read and write (`read_tracker`, `write_tracker` of `specsync.py`).
use crate::tokens::util::splitlines;

/// The tracker's record of one item.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Tracked {
    /// `#101` or similar.
    pub item_ref: String,
    /// The platform's id.
    pub id: String,
    /// Last synced state.
    pub state: String,
    /// Last synced digest.
    pub hash: String,
    /// Link.
    pub url: String,
}

/// The parsed `tracker.md`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Tracker {
    /// First word after `- Platform:`.
    pub platform: String,
    /// Items by key, in file order.
    pub items: Vec<(String, Tracked)>,
}

impl Tracker {
    /// The item for a key.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Tracked> {
        self.items.iter().find(|(k, _)| k == key).map(|(_, t)| t)
    }

    /// The item for a key, created empty when missing (`dict.setdefault`).
    pub fn entry(&mut self, key: &str) -> &mut Tracked {
        if let Some(i) = self.items.iter().position(|(k, _)| k == key) {
            return &mut self.items[i].1;
        }
        self.items.push((key.to_string(), Tracked::default()));
        &mut self.items.last_mut().expect("just pushed").1
    }
}

/// Parse `tracker.md` text.
#[must_use]
pub fn read_tracker(text: Option<&str>) -> Tracker {
    let mut out = Tracker::default();
    let Some(text) = text else {
        return out;
    };
    for line in splitlines(text) {
        if let Some(rest) = line.strip_prefix("- Platform:") {
            if let Some(w) = rest.split_whitespace().next() {
                out.platform = w.to_string();
            }
        }
        if line.starts_with('|')
            && !line.starts_with("| Key")
            && !line.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
        {
            let cells: Vec<String> = line
                .trim()
                .trim_matches('|')
                .split('|')
                .map(|c| c.trim().to_string())
                .collect();
            if cells.len() >= 6 {
                let t = Tracked {
                    item_ref: cells[1].clone(),
                    id: cells[2].clone(),
                    state: cells[3].clone(),
                    hash: cells[4].clone(),
                    url: cells[5].clone(),
                };
                if let Some(slot) = out.items.iter_mut().find(|(k, _)| *k == cells[0]) {
                    slot.1 = t;
                } else {
                    out.items.push((cells[0].clone(), t));
                }
            }
        }
    }
    out
}

fn order(key: &str) -> (u8, i64) {
    if key == "feature" {
        return (0, 0);
    }
    let tail = key.get(2..).unwrap_or("");
    (
        1,
        if !tail.is_empty() && tail.chars().all(|c| c.is_ascii_digit()) {
            tail.parse().unwrap_or(0)
        } else {
            0
        },
    )
}

/// Today's date (UTC) as `YYYY-MM-DD`.
#[must_use]
pub fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let days = i64::try_from(secs / 86_400).unwrap_or(0);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

/// The text of `tracker.md` for a feature directory name.
#[must_use]
pub fn render_tracker(
    dir_name: &str,
    platform: &str,
    wher: &str,
    items: &[(String, Tracked)],
    date: &str,
) -> String {
    let mut lines = vec![
        format!("# Tracker \u{2014} {dir_name}"),
        String::new(),
        "Written by `repo:sync` (`plugins/repo/references/sync.md`): the tracker items that"
            .to_string(),
        "mirror this feature. Commits and pull requests reference the `Item` column.".to_string(),
        String::new(),
        format!(
            "- Platform: {platform}{}",
            if wher.is_empty() {
                String::new()
            } else {
                format!(" ({wher})")
            }
        ),
        format!("- Last sync: {date}"),
        String::new(),
        "| Key | Item | Id | State | Hash | URL |".to_string(),
        "|---|---|---|---|---|---|".to_string(),
    ];
    let mut sorted: Vec<&(String, Tracked)> = items.iter().collect();
    sorted.sort_by_key(|(k, _)| order(k));
    for (key, i) in sorted {
        lines.push(format!(
            "| {key} | {} | {} | {} | {} | {} |",
            i.item_ref, i.id, i.state, i.hash, i.url
        ));
    }
    lines.join("\n") + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let items = vec![
            (
                "US2".to_string(),
                Tracked {
                    item_ref: "#3".into(),
                    id: "9".into(),
                    state: "todo".into(),
                    hash: "h".into(),
                    url: "u".into(),
                },
            ),
            (
                "feature".to_string(),
                Tracked {
                    item_ref: "#1".into(),
                    ..Tracked::default()
                },
            ),
            ("US10".to_string(), Tracked::default()),
        ];
        let text = render_tracker("012-export", "github", "o/r", &items, "2026-11-01");
        assert!(text.contains("- Platform: github (o/r)\n- Last sync: 2026-11-01\n"));
        let t = read_tracker(Some(&text));
        assert_eq!(t.platform, "github");
        let keys: Vec<&str> = t.items.iter().map(|(k, _)| k.as_str()).collect();
        assert_eq!(keys, ["feature", "US2", "US10"]);
        assert_eq!(t.get("US2").unwrap().item_ref, "#3");
    }

    #[test]
    fn today_has_the_date_shape() {
        let d = today();
        assert_eq!(d.len(), 10);
        assert_eq!(&d[4..5], "-");
    }
}
