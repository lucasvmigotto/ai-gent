//! `.worktrees/.ports.json` and the baseline file.
use super::{DIR, Failed, Res};
use crate::tokens::pyjson::{J, dumps, parse};
use std::path::PathBuf;

fn file(root: &str) -> PathBuf {
    PathBuf::from(root).join(DIR).join(".ports.json")
}

/// Branch to offset; empty when the file is missing or unreadable.
#[must_use]
pub fn load(root: &str) -> Vec<(String, i64)> {
    let Ok(bytes) = std::fs::read(file(root)) else {
        return Vec::new();
    };
    match parse(&String::from_utf8_lossy(&bytes)) {
        Some(J::Obj(pairs)) => pairs
            .into_iter()
            .filter_map(|(k, v)| v.as_i64().map(|n| (k, n)))
            .collect(),
        _ => Vec::new(),
    }
}

/// Write the map with sorted keys, indent 2 and a trailing newline.
pub fn save(root: &str, ports: &[(String, i64)]) -> Res<()> {
    let mut sorted = ports.to_vec();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));
    let j = J::Obj(sorted.into_iter().map(|(k, v)| (k, J::Int(v))).collect());
    write(&file(root), &(dumps(&j, true, Some(2)) + "\n"))
}

/// Write a file, creating its directory.
pub fn write(path: &std::path::Path, text: &str) -> Res<()> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).map_err(|e| Failed(format!("{}: {e}", d.display())))?;
    }
    std::fs::write(path, text).map_err(|e| Failed(format!("{}: {e}", path.display())))
}

/// The smallest multiple of the step not already used by a live track.
#[must_use]
pub fn next_offset(ports: &[(String, i64)]) -> i64 {
    (1..1000)
        .map(|k| k * super::PORT_STEP)
        .find(|o| !ports.iter().any(|(_, v)| v == o))
        .unwrap_or(1000 * super::PORT_STEP)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offsets_skip_live_ones() {
        let live = vec![("b".to_string(), 200)];
        assert_eq!(next_offset(&live), 100);
        let live = vec![("a".to_string(), 100), ("b".to_string(), 200)];
        assert_eq!(next_offset(&live), 300);
    }

    #[test]
    fn save_sorts_keys_and_ends_with_a_newline() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path().to_str().unwrap().replace('\\', "/");
        save(&root, &[("z".into(), 200), ("a".into(), 100)]).unwrap();
        let text = std::fs::read_to_string(d.path().join(".worktrees/.ports.json")).unwrap();
        assert_eq!(text, "{\n  \"a\": 100,\n  \"z\": 200\n}\n");
        assert_eq!(
            load(&root),
            vec![("a".to_string(), 100), ("z".to_string(), 200)]
        );
    }
}
