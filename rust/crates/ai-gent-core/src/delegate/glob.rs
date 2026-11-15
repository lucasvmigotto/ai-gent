//! A small glob for `allowed_paths`: `*` and `?` inside a segment, `**` for any number of segments, a
//! trailing `/` for a whole directory. Paths use forward slashes.

/// A repository path normalized for matching: forward slashes, no leading `./` or `/`.
#[must_use]
pub fn norm(p: &str) -> String {
    let s = p.replace('\\', "/");
    let s = s.trim_start_matches("./").trim_start_matches('/');
    s.to_string()
}

fn seg_match(p: &[u8], s: &[u8]) -> bool {
    let (mut pi, mut si) = (0, 0);
    let (mut star, mut mark) = (None, 0);
    while si < s.len() {
        if pi < p.len() && (p[pi] == b'?' || p[pi] == s[si]) {
            pi += 1;
            si += 1;
        } else if pi < p.len() && p[pi] == b'*' {
            star = Some(pi);
            mark = si;
            pi += 1;
        } else if let Some(st) = star {
            pi = st + 1;
            mark += 1;
            si = mark;
        } else {
            return false;
        }
    }
    p[pi..].iter().all(|&c| c == b'*')
}

fn segs_match(p: &[&str], s: &[&str]) -> bool {
    match p.split_first() {
        None => s.is_empty(),
        Some((&"**", rest)) => (0..=s.len()).any(|i| segs_match(rest, &s[i..])),
        Some((head, rest)) => s
            .split_first()
            .is_some_and(|(h, t)| seg_match(head.as_bytes(), h.as_bytes()) && segs_match(rest, t)),
    }
}

/// Whether `path` matches `pattern`.
#[must_use]
pub fn matches(pattern: &str, path: &str) -> bool {
    let mut pat = norm(pattern);
    if pat.ends_with('/') {
        pat.push_str("**");
    }
    let path = norm(path);
    let ps: Vec<&str> = pat.split('/').collect();
    let ss: Vec<&str> = path.split('/').collect();
    segs_match(&ps, &ss)
}

/// The literal directory or file prefix of a pattern, up to its first wildcard segment.
#[must_use]
pub fn literal_prefix(pattern: &str) -> String {
    let pat = norm(pattern);
    let mut out = Vec::new();
    for seg in pat.split('/') {
        if seg.contains(['*', '?']) {
            break;
        }
        out.push(seg);
    }
    out.join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn globs() {
        assert!(matches("rust/crates/x/src/**", "rust/crates/x/src/a/b.rs"));
        assert!(matches("rust/crates/x/src/**", "rust/crates/x/src/b.rs"));
        assert!(!matches("rust/crates/x/src/**", "rust/crates/x/Cargo.toml"));
        assert!(matches("rust/crates/x/", "rust/crates/x/src/b.rs"));
        assert!(matches("specs/*/tasks.md", "specs/038-db/tasks.md"));
        assert!(!matches("specs/*/tasks.md", "specs/038-db/x/tasks.md"));
        assert!(matches("**/*.rs", "a/b/c.rs"));
        assert!(matches("**/*.rs", "c.rs"));
        assert!(matches("a?c.txt", "abc.txt"));
        assert!(matches("./a.txt", "a.txt"));
        assert!(matches("a.txt", "a.txt"));
        assert!(!matches("a.txt", "a.txt.bak"));
        assert!(matches("a\\b.txt", "a/b.txt"));
    }

    #[test]
    fn prefix() {
        assert_eq!(literal_prefix("rust/crates/x/src/**"), "rust/crates/x/src");
        assert_eq!(literal_prefix("**"), "");
        assert_eq!(literal_prefix("*.md"), "");
        assert_eq!(literal_prefix("specs/038/tasks.md"), "specs/038/tasks.md");
    }
}
