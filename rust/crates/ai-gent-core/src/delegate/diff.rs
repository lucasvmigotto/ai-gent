//! A unified diff (`git diff`) read into the files it changes and the lines it adds and removes.

/// How a file changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A new file.
    Added,
    /// An existing file edited.
    Modified,
    /// A file removed.
    Deleted,
    /// A file moved (and maybe edited).
    Renamed,
}

/// One changed file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChange {
    /// The path after the change (the path removed, for a deletion).
    pub path: String,
    /// The path before a rename.
    pub old_path: Option<String>,
    /// How it changed.
    pub kind: Kind,
    /// Lines added (without the leading `+`).
    pub added: Vec<String>,
    /// Lines removed (without the leading `-`).
    pub removed: Vec<String>,
}

impl FileChange {
    /// An added file with this content.
    #[must_use]
    pub fn added_file(path: &str, content: &str) -> Self {
        Self {
            path: path.to_string(),
            old_path: None,
            kind: Kind::Added,
            added: content.lines().map(str::to_string).collect(),
            removed: Vec::new(),
        }
    }
}

fn header_path(rest: &str) -> String {
    // "diff --git a/<p> b/<p>" with the same path on both sides (the common case, spaces included);
    // for a rename the `rename to` line overrides this.
    for (i, _) in rest.match_indices(' ') {
        let (l, r) = (&rest[..i], &rest[i + 1..]);
        if let (Some(a), Some(b)) = (l.strip_prefix("a/"), r.strip_prefix("b/"))
            && a == b
        {
            return a.to_string();
        }
    }
    rest.rsplit_once(" b/")
        .map_or_else(|| rest.to_string(), |(_, b)| b.to_string())
}

/// Parse `git diff` output (use `-c core.quotepath=off`, so paths are not quoted).
#[must_use]
pub fn parse(text: &str) -> Vec<FileChange> {
    let mut out: Vec<FileChange> = Vec::new();
    let mut in_hunk = false;
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("diff --git ") {
            in_hunk = false;
            out.push(FileChange {
                path: header_path(rest),
                old_path: None,
                kind: Kind::Modified,
                added: Vec::new(),
                removed: Vec::new(),
            });
            continue;
        }
        let Some(cur) = out.last_mut() else { continue };
        if line.starts_with("@@") {
            in_hunk = true;
        } else if in_hunk {
            if let Some(a) = line.strip_prefix('+') {
                cur.added.push(a.to_string());
            } else if let Some(r) = line.strip_prefix('-') {
                cur.removed.push(r.to_string());
            }
        } else if line.starts_with("new file mode") {
            cur.kind = Kind::Added;
        } else if line.starts_with("deleted file mode") {
            cur.kind = Kind::Deleted;
        } else if let Some(p) = line.strip_prefix("rename from ") {
            cur.kind = Kind::Renamed;
            cur.old_path = Some(p.to_string());
        } else if let Some(p) = line.strip_prefix("rename to ") {
            cur.kind = Kind::Renamed;
            cur.path = p.to_string();
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const D: &str = "diff --git a/src/a b.rs b/src/a b.rs\n\
index 1..2 100644\n--- a/src/a b.rs\n+++ b/src/a b.rs\n@@ -1,2 +1,2 @@\n keep\n-old\n+new\n\
diff --git a/n.txt b/n.txt\nnew file mode 100644\n--- /dev/null\n+++ b/n.txt\n@@ -0,0 +1 @@\n+hello\n\
diff --git a/gone.rs b/gone.rs\ndeleted file mode 100644\n--- a/gone.rs\n+++ /dev/null\n@@ -1 +0,0 @@\n-bye\n\
diff --git a/o.md b/p.md\nsimilarity index 90%\nrename from o.md\nrename to p.md\n@@ -1 +1 @@\n-x\n+y\n\
diff --git a/img.png b/img.png\nBinary files a/img.png and b/img.png differ\n";

    #[test]
    fn parses_files_kinds_and_lines() {
        let v = parse(D);
        assert_eq!(v.len(), 5);
        assert_eq!(v[0].path, "src/a b.rs");
        assert_eq!(
            (v[0].added.as_slice(), v[0].removed.as_slice()),
            (&["new".to_string()][..], &["old".to_string()][..])
        );
        assert_eq!(v[1].kind, Kind::Added);
        assert_eq!(v[1].added, ["hello"]);
        assert_eq!(v[2].kind, Kind::Deleted);
        assert_eq!(v[2].removed, ["bye"]);
        assert_eq!(
            (v[3].kind, v[3].path.as_str(), v[3].old_path.as_deref()),
            (Kind::Renamed, "p.md", Some("o.md"))
        );
        assert!(v[4].added.is_empty());
    }

    #[test]
    fn a_line_starting_with_plus_plus_plus_inside_a_hunk_is_content() {
        let v = parse("diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -1 +1 @@\n-a\n+++ b\n");
        assert_eq!(v[0].added, ["++ b"]);
    }
}
