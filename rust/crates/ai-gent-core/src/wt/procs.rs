//! Image builds in progress, so `finish` leaves a running build's working containers alone.
use std::process::Command;

const TOOLS: [&str; 4] = ["podman", "docker", "buildah", "nerdctl"];

/// Builds in progress: `(this project's, elsewhere)`. Where a process's directory can't be read,
/// every build counts as this project's (the cautious answer).
#[must_use]
pub fn builds_running(root: &str) -> (Vec<String>, Vec<String>) {
    #[cfg(feature = "test-support")]
    if let Ok(v) = std::env::var("AI_GENT_WT_FAKE_BUILDS") {
        if let Some(j) = crate::tokens::pyjson::parse(&v) {
            let list = |k: &str| match j.get(k) {
                Some(crate::tokens::pyjson::J::Arr(a)) => a
                    .iter()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect(),
                _ => Vec::new(),
            };
            return (list("mine"), list("other"));
        }
    }
    if cfg!(target_os = "linux") && std::path::Path::new("/proc").is_dir() {
        return from_proc(root);
    }
    (from_ps(), Vec::new())
}

/// A word at a word boundary (`\bword\b`) at or after byte `from`; returns its end.
fn find_word(s: &str, word: &str, from: usize) -> Option<usize> {
    let b = s.as_bytes();
    let is_w = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    let mut start = from;
    while let Some(i) = s.get(start..)?.find(word) {
        let (a, z) = (start + i, start + i + word.len());
        if (a == 0 || !is_w(b[a - 1])) && (z == b.len() || !is_w(b[z])) {
            return Some(z);
        }
        start = a + 1;
        if !s.is_char_boundary(start) {
            start += 1;
        }
    }
    None
}

/// `\b(podman|docker|buildah|nerdctl)\b.*\b(build|bud)\b`.
fn is_build_line(line: &str) -> bool {
    TOOLS.iter().any(|t| {
        find_word(line, t, 0).is_some_and(|end| {
            find_word(line, "build", end).is_some() || find_word(line, "bud", end).is_some()
        })
    })
}

fn from_ps() -> Vec<String> {
    let out = if cfg!(windows) {
        Command::new("powershell")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Get-CimInstance Win32_Process | ForEach-Object { $_.CommandLine }",
            ])
            .output()
    } else {
        Command::new("ps").args(["-eo", "args"]).output()
    };
    let Ok(out) = out else { return Vec::new() };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| is_build_line(l))
        .map(str::to_string)
        .collect()
}

fn from_proc(root: &str) -> (Vec<String>, Vec<String>) {
    let (mut mine, mut other) = (Vec::new(), Vec::new());
    let Ok(dir) = std::fs::read_dir("/proc") else {
        return (mine, other);
    };
    for e in dir.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if name.is_empty() || !name.bytes().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let p = e.path();
        let (Ok(raw), Ok(cwd)) = (
            std::fs::read(p.join("cmdline")),
            std::fs::read_link(p.join("cwd")),
        ) else {
            continue;
        };
        let words: Vec<String> = raw
            .split(|c| *c == 0)
            .filter(|a| !a.is_empty())
            .map(|a| String::from_utf8_lossy(a).to_string())
            .collect();
        let Some(first) = words.first() else { continue };
        let base = std::path::Path::new(first)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        if !TOOLS.contains(&base.as_str())
            || !words
                .iter()
                .skip(1)
                .take(3)
                .any(|w| w == "build" || w == "bud")
        {
            continue;
        }
        let cwd = cwd.to_string_lossy().replace('\\', "/");
        let desc: String = words.join(" ").chars().take(120).collect();
        if cwd == root || cwd.starts_with(&format!("{root}/")) {
            mine.push(desc)
        } else {
            other.push(desc)
        }
    }
    (mine, other)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_lines() {
        assert!(is_build_line("podman build -t x ."));
        assert!(is_build_line("/usr/bin/docker  buildx build ."));
        assert!(!is_build_line("podman ps"));
        assert!(!is_build_line("vim rebuild.md"));
        assert!(is_build_line("buildah bud ."));
    }
}
