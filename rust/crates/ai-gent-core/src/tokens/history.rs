//! History records (`docs/product/token-history.jsonl`, `~/.ai-gent/token-history.jsonl`): reading,
//! upserting by id, and merging several files.
use super::pyjson::{J, dumps, parse};
use super::util::{OMap, ensure_parent, expanduser, read_text};
use std::path::{Path, PathBuf};

/// The repository's history file, relative to the working directory.
pub const DEFAULT_HISTORY: &str = "docs/product/token-history.jsonl";

/// `$AI_GENT_HOME`, else `~/.ai-gent`.
#[must_use]
pub fn ai_gent_home() -> PathBuf {
    match std::env::var("AI_GENT_HOME").ok().filter(|v| !v.is_empty()) {
        Some(h) => PathBuf::from(h),
        None => PathBuf::from(expanduser("~/.ai-gent")),
    }
}

/// The cross-project index.
#[must_use]
pub fn default_index() -> PathBuf {
    ai_gent_home().join("token-history.jsonl")
}

/// The record's id.
#[must_use]
pub fn rec_id(r: &J) -> &str {
    r.get("id").and_then(J::as_str).unwrap_or("")
}

/// Records of a JSONL file; blank and invalid lines are skipped.
#[must_use]
pub fn read_history(path: &Path) -> Vec<J> {
    let Ok(text) = read_text(path) else {
        return Vec::new();
    };
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .filter_map(parse)
        .filter(|r| matches!(r, J::Obj(_)))
        .collect()
}

fn sort_key(r: &J) -> (String, String) {
    (
        r.get("date").and_then(J::as_str).unwrap_or("").to_string(),
        rec_id(r).to_string(),
    )
}

/// Write records sorted by (date, id), one JSON object per line.
pub fn write_history(path: &Path, records: &[J]) -> std::io::Result<()> {
    ensure_parent(path)?;
    let mut v: Vec<&J> = records.iter().collect();
    v.sort_by_key(|r| sort_key(r));
    let mut out = String::new();
    for r in v {
        out.push_str(&dumps(r, false, None));
        out.push('\n');
    }
    std::fs::write(path, out)
}

/// Add or replace records by id; the file stays sorted, so re-running never duplicates a line.
pub fn upsert_history(path: &Path, records: Vec<J>) -> std::io::Result<usize> {
    let mut merged: OMap<J> = OMap::new();
    for r in read_history(path).into_iter().chain(records) {
        let id = rec_id(&r).to_string();
        merged.set(&id, r);
    }
    let all: Vec<J> = merged.values().cloned().collect();
    write_history(path, &all)?;
    Ok(all.len())
}

/// Drop a seeded record whose agent a measured record already counts, so one agent never counts
/// twice. First occurrence of an id wins.
#[must_use]
pub fn load_history_from(records: Vec<J>) -> Vec<J> {
    let mut by_id: OMap<J> = OMap::new();
    for r in records {
        let id = rec_id(&r).to_string();
        if !by_id.contains_key(&id) {
            by_id.set(&id, r);
        }
    }
    let agents = |r: &J| -> Vec<String> {
        match r.get("agents") {
            Some(J::Arr(a)) => a
                .iter()
                .filter_map(|x| x.as_str().map(String::from))
                .collect(),
            _ => vec![],
        }
    };
    let is_seed = |r: &J| r.get("source").and_then(J::as_str) == Some("seed");
    let measured: std::collections::HashSet<String> = by_id
        .values()
        .filter(|r| !is_seed(r))
        .flat_map(agents)
        .collect();
    by_id
        .values()
        .filter(|r| !(is_seed(r) && agents(r).iter().any(|a| measured.contains(a))))
        .cloned()
        .collect()
}

/// Records from the given files, by id (the first file wins).
#[must_use]
pub fn load_history(paths: &[PathBuf]) -> Vec<J> {
    load_history_from(paths.iter().flat_map(|p| read_history(p)).collect())
}

/// `list(extra) + [p for p in (DEFAULT_HISTORY, default_index()) if exists]`.
#[must_use]
pub fn history_paths(extra: &[String]) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = extra.iter().map(PathBuf::from).collect();
    for p in [PathBuf::from(DEFAULT_HISTORY), default_index()] {
        if p.exists() {
            v.push(p);
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(id: &str, source: &str, agent: &str, used: i64) -> J {
        parse(&format!(r#"{{"id": "{id}", "source": "{source}", "agents": ["{agent}"], "used": {used}, "date": "2026-10-20"}}"#)).unwrap()
    }

    #[test]
    fn load_history_drops_a_seed_the_repo_already_measured() {
        let d = tempfile::tempdir().unwrap();
        let (a, b) = (d.path().join("a.jsonl"), d.path().join("b.jsonl"));
        write_history(&a, &[rec("p:1", "measured", "x", 10)]).unwrap();
        write_history(
            &b,
            &[rec("p:2", "seed", "x", 20), rec("p:3", "seed", "y", 30)],
        )
        .unwrap();
        let mut used: Vec<i64> = load_history(&[a, b])
            .iter()
            .filter_map(|r| r.get("used").and_then(J::as_i64))
            .collect();
        used.sort_unstable();
        assert_eq!(used, [10, 30]);
    }

    #[test]
    fn upsert_replaces_by_id_and_keeps_sorted() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("sub/h.jsonl");
        assert_eq!(
            upsert_history(
                &p,
                vec![rec("b", "measured", "1", 1), rec("a", "measured", "2", 2)]
            )
            .unwrap(),
            2
        );
        assert_eq!(
            upsert_history(&p, vec![rec("b", "measured", "1", 9)]).unwrap(),
            2
        );
        let r = read_history(&p);
        assert_eq!(r.iter().map(rec_id).collect::<Vec<_>>(), ["a", "b"]);
        assert_eq!(r[1].get("used"), Some(&J::Int(9)));
    }
}
