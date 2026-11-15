//! Directory walk: the supported source files under a root, in the order of `iter_source_files`.

use std::path::{Path, PathBuf};

const SKIP_DIRS: &[&str] = &[
    ".git",
    ".hg",
    ".svn",
    "node_modules",
    "target",
    "build",
    "dist",
    "out",
    ".gradle",
    ".idea",
    ".vscode",
    "__pycache__",
    ".venv",
    "venv",
    "vendor",
    ".next",
    ".nuxt",
    "bin",
    "obj",
    ".mypy_cache",
    ".pytest_cache",
    "coverage",
];

/// Files of `root` whose language is known: the files of a directory (sorted) before its
/// sub-directories (sorted), vendored, build and hidden directories skipped. Symbolic links to
/// directories are not followed.
#[must_use]
pub fn source_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk(root, &mut out);
    out
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let (mut files, mut dirs) = (Vec::new(), Vec::new());
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_dir() {
            if !SKIP_DIRS.contains(&name.as_str()) && !name.starts_with('.') {
                dirs.push(name);
            }
        } else {
            files.push(name);
        }
    }
    files.sort();
    dirs.sort();
    for f in files {
        let p = dir.join(&f);
        if crate::langs::detect(&p.to_string_lossy()).is_some() {
            out.push(p);
        }
    }
    for d in dirs {
        walk(&dir.join(d), out);
    }
}
