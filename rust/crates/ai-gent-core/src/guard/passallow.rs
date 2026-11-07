//! The personal `pass show` allowlist of the secrets guard (`shared/secrets.md`).
//!
//! A file the user writes: `$AI_GENT_PASS_ALLOW`, else `${XDG_CONFIG_HOME:-~/.config}/ai-gent/pass-allow`.
//! One rule per line, an entry (exact) or a `prefix/` (everything under it); blank lines and `#`
//! comments are ignored, and a rule that starts with `/`, `-` or `~`, or has a `.` or `..` path
//! part, never allows. A missing or unreadable file allows nothing. Mirrors `allowed_entry` in
//! `plugins/secrets/hooks/guard.sh` and `secretsAllowed` in `opencode/guard/rules.mjs`.
//!
//! Parsing is pure ([`allows`]); [`Source`] is the thin reader of the variable and the file, which
//! tests replace with [`with_source`].
use std::cell::RefCell;
use std::rc::Rc;

/// Reads a file by path (`None`: missing or unreadable).
pub type Reader = Rc<dyn Fn(&str) -> Option<String>>;

/// Where the allowlist comes from: the three environment variables and a file reader.
#[derive(Clone)]
pub struct Source {
    /// `AI_GENT_PASS_ALLOW` (unset and empty both fall back to the config home).
    pub allow_var: Option<String>,
    /// `XDG_CONFIG_HOME`.
    pub xdg: Option<String>,
    /// `HOME`.
    pub home: Option<String>,
    /// File text by path (`None`: missing or unreadable).
    pub read: Reader,
}

thread_local! {
    static OVERRIDE: RefCell<Option<Source>> = const { RefCell::new(None) };
}

/// The text of a regular file, like the shell guard's `-f` and `-r`: a FIFO or a device would block
/// the read, so anything else is no file.
fn read_regular(path: &str) -> Option<String> {
    std::fs::metadata(path)
        .ok()
        .filter(std::fs::Metadata::is_file)
        .and_then(|_| std::fs::read_to_string(path).ok())
}

impl Source {
    /// The real environment and file system.
    #[must_use]
    pub fn system() -> Self {
        Source {
            allow_var: std::env::var("AI_GENT_PASS_ALLOW").ok(),
            xdg: std::env::var("XDG_CONFIG_HOME").ok(),
            home: std::env::var("HOME").ok(),
            read: Rc::new(read_regular),
        }
    }

    /// The source in force: the one [`with_source`] installed on this thread, else the system.
    #[must_use]
    pub fn current() -> Self {
        OVERRIDE
            .with(|o| o.borrow().clone())
            .unwrap_or_else(Self::system)
    }

    /// The allowlist file's path.
    #[must_use]
    pub fn file(&self) -> String {
        let set = |v: &Option<String>| v.clone().filter(|s| !s.is_empty());
        if let Some(p) = set(&self.allow_var) {
            return p;
        }
        let base = set(&self.xdg)
            .unwrap_or_else(|| format!("{}/.config", self.home.as_deref().unwrap_or("")));
        format!("{base}/ai-gent/pass-allow")
    }

    /// Whether the allowlist file lists `entry`.
    #[must_use]
    pub fn allows(&self, entry: &str) -> bool {
        (self.read)(&self.file()).is_some_and(|text| allows(&text, entry))
    }
}

/// Run `f` with `source` standing in for the environment and file on this thread.
pub fn with_source<T>(source: Source, f: impl FnOnce() -> T) -> T {
    let prev = OVERRIDE.with(|o| o.replace(Some(source)));
    let out = f();
    OVERRIDE.with(|o| *o.borrow_mut() = prev);
    out
}

/// Whether a rule of the file's `text` names `entry`.
#[must_use]
pub fn allows(text: &str, entry: &str) -> bool {
    let blank = |c: char| c == ' ' || c == '\t';
    text.split('\n').any(|raw| {
        let rule = raw.strip_suffix('\r').unwrap_or(raw).trim_matches(blank);
        if rule.is_empty() || rule.starts_with(['#', '/', '-', '~']) {
            return false;
        }
        if rule.split('/').any(|p| p == "." || p == "..") {
            return false;
        }
        if rule.ends_with('/') {
            entry.starts_with(rule) && entry.len() > rule.len()
        } else {
            rule == entry
        }
    })
}

/// Whether `entry` is a clean name the allowlist may open: no odd characters, no empty, `.` or
/// `..` part, and never under `ai-gent/db/` (only dbrun reads those).
#[must_use]
pub fn clean_entry(entry: &str) -> bool {
    if entry.is_empty()
        || entry.starts_with('/')
        || entry.ends_with('/')
        || entry.contains("//")
        || !entry
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._@+/-".contains(c))
        || entry.split('/').any(|p| p == "." || p == "..")
    {
        return false;
    }
    let lower = entry.to_ascii_lowercase();
    lower != "ai-gent/db" && !lower.starts_with("ai-gent/db/")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn src(var: Option<&str>, text: Option<&'static str>) -> Source {
        Source {
            allow_var: var.map(str::to_string),
            xdg: None,
            home: Some("/h".into()),
            read: Rc::new(move |_| text.map(str::to_string)),
        }
    }

    #[test]
    fn a_listed_entry_and_a_prefix_allow() {
        let t = "ai-gent/github/pat\nwork/\n";
        assert!(allows(t, "ai-gent/github/pat"));
        assert!(allows(t, "work/vpn"));
        assert!(allows(t, "work/a/b"));
        assert!(!allows(t, "work/"));
        assert!(!allows(t, "work"));
        assert!(!allows(t, "ai-gent/github/pat2"));
        assert!(!allows(t, "other"));
    }

    #[test]
    fn comments_blanks_spaces_and_carriage_returns() {
        let t = "# work/vpn\n\n   \n  work/vpn  \r\n\t#x\n";
        assert!(allows(t, "work/vpn"));
        assert!(!allows("# work/vpn\n", "work/vpn"));
        assert!(!allows("#work/\n", "work/x"));
    }

    #[test]
    fn malformed_rules_never_allow() {
        for rule in [
            "", "/abs", "-x", "~/x", "a/../b", "./a", "a/./b", "..", ".", "a/..", "../",
        ] {
            let t = format!("{rule}\n");
            for e in [rule, "a/b", "a/x", "x", "b"] {
                assert!(!allows(&t, e), "{rule:?} allowed {e:?}");
            }
        }
        assert!(!allows("", "x"));
    }

    #[test]
    fn entries_are_clean_or_not() {
        for ok in ["a", "ai-gent/github/pat", "a.b/c_d@e+f-g", "ai-gent/dbx"] {
            assert!(clean_entry(ok), "{ok}");
        }
        for bad in [
            "",
            "/a",
            "a/",
            "a//b",
            "a b",
            "a;b",
            ".",
            "..",
            "./a",
            "a/..",
            "a/./b",
            "ai-gent/db",
            "AI-Gent/DB/x",
            "ai-gent/db/x",
            "a*",
        ] {
            assert!(!clean_entry(bad), "{bad}");
        }
    }

    #[test]
    fn the_file_comes_from_the_variable_then_the_config_home() {
        assert_eq!(src(Some("/x/allow"), None).file(), "/x/allow");
        assert_eq!(src(Some(""), None).file(), "/h/.config/ai-gent/pass-allow");
        let mut s = src(None, None);
        assert_eq!(s.file(), "/h/.config/ai-gent/pass-allow");
        s.xdg = Some("/cfg".into());
        assert_eq!(s.file(), "/cfg/ai-gent/pass-allow");
    }

    #[test]
    fn a_missing_file_allows_nothing() {
        assert!(!src(None, None).allows("a/b"));
        assert!(src(None, Some("a/b\n")).allows("a/b"));
    }

    #[test]
    fn the_override_is_scoped() {
        let inside = with_source(src(Some("/p"), Some("a/\n")), || Source::current().file());
        assert_eq!(inside, "/p");
        assert_ne!(Source::current().file(), "/p");
    }

    #[cfg(unix)]
    #[test]
    fn a_fifo_is_no_file_and_does_not_block() {
        let dir = std::env::temp_dir().join(format!("pass-allow-fifo-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let fifo = dir.join("pass-allow");
        assert!(
            std::process::Command::new("mkfifo")
                .arg(&fifo)
                .status()
                .unwrap()
                .success()
        );
        let path = fifo.to_string_lossy().into_owned();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(read_regular(&path));
        });
        let got = rx.recv_timeout(std::time::Duration::from_secs(5));
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(got, Ok(None), "a FIFO must read as no file, not block");
    }
}
