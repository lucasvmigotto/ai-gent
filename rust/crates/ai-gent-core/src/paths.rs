//! Install root and per-OS directories.
use std::path::PathBuf;

/// The install root: `$AI_GENT_HOME`, else `${XDG_DATA_HOME:-~/.local/share}/ai-gent` on Unix
/// and `%LOCALAPPDATA%\ai-gent` on Windows. `None` when the needed variables are unset.
#[must_use]
pub fn install_root() -> Option<PathBuf> {
    root_from(|k| {
        std::env::var_os(k)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    })
}

pub(crate) fn root_from(get: impl Fn(&str) -> Option<PathBuf>) -> Option<PathBuf> {
    if let Some(p) = get("AI_GENT_HOME") {
        return Some(p);
    }
    if cfg!(windows) {
        return get("LOCALAPPDATA").map(|p| p.join("ai-gent"));
    }
    if let Some(p) = get("XDG_DATA_HOME") {
        return Some(p.join("ai-gent"));
    }
    get("HOME").map(|p| p.join(".local/share/ai-gent"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_wins() {
        let r = root_from(|k| (k == "AI_GENT_HOME").then(|| PathBuf::from("/x")));
        assert_eq!(r, Some(PathBuf::from("/x")));
    }

    #[cfg(unix)]
    #[test]
    fn xdg_then_home() {
        let r = root_from(|k| (k == "XDG_DATA_HOME").then(|| PathBuf::from("/d")));
        assert_eq!(r, Some(PathBuf::from("/d/ai-gent")));
        let r = root_from(|k| (k == "HOME").then(|| PathBuf::from("/h")));
        assert_eq!(r, Some(PathBuf::from("/h/.local/share/ai-gent")));
    }
}
