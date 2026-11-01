//! The install tree: `<home>/<ver>/`, `current`, `previous`, `pinned` (written only by `install.sh`).
use crate::env::Env;
use crate::update::version::is_version;
use std::path::{Path, PathBuf};

/// Where the running binary lives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallRoot {
    /// The install root: the parent of the version directory, else `AI_GENT_HOME` or the default.
    pub home: Option<PathBuf>,
    /// The version directory the binary runs from (canonical), for a package install.
    pub version_dir: Option<PathBuf>,
    /// Contents of that directory's `VERSION`.
    pub version: Option<String>,
    /// No `VERSION` beside `bin/`: `cargo run`, a checkout.
    pub dev_build: bool,
}

impl InstallRoot {
    /// Find the root from the binary's real path (`<home>/<ver>/bin/ai-gent[.exe]`), else the env.
    #[must_use]
    pub fn discover(exe: &Path, env: &Env) -> Self {
        let real = exe.canonicalize().unwrap_or_else(|_| exe.to_path_buf());
        let package = real
            .parent()
            .filter(|bin| bin.file_name().is_some_and(|n| n == "bin"))
            .and_then(Path::parent)
            .and_then(|vdir| {
                read_version_file(&vdir.join("VERSION")).map(|v| (vdir.to_path_buf(), v))
            });
        match package {
            Some((vdir, version)) => Self {
                home: vdir.parent().map(Path::to_path_buf),
                version_dir: Some(vdir),
                version: Some(version),
                dev_build: false,
            },
            None => Self {
                home: env.install_root(),
                version_dir: None,
                version: None,
                dev_build: true,
            },
        }
    }

    /// The version a `current` link names.
    #[must_use]
    pub fn current(&self) -> Option<String> {
        self.home
            .as_deref()
            .and_then(|h| link_version(&h.join("current")))
    }

    /// The version a `previous` link names.
    #[must_use]
    pub fn previous(&self) -> Option<String> {
        self.home
            .as_deref()
            .and_then(|h| link_version(&h.join("previous")))
    }

    /// The pinned version, when `<home>/pinned` exists. A present but unreadable or garbled file
    /// still counts as a pin (silence is the safe side), reported as an empty string.
    #[must_use]
    pub fn pinned(&self) -> Option<String> {
        let file = self.home.as_deref()?.join("pinned");
        if !file.exists() {
            return None;
        }
        Some(
            std::fs::read_to_string(&file)
                .ok()
                .map(|t| t.trim().to_string())
                .filter(|t| is_version(t))
                .unwrap_or_default(),
        )
    }

    /// The unpacked versions, oldest first.
    #[must_use]
    pub fn versions(&self) -> Vec<String> {
        let Some(home) = self.home.as_deref() else {
            return Vec::new();
        };
        let mut out: Vec<String> = std::fs::read_dir(home)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
            .filter_map(|e| e.file_name().into_string().ok())
            .filter(|n| is_version(n))
            .collect();
        out.sort_by(|a, b| crate::update::version::compare(a, b));
        out
    }
}

/// First line of a `VERSION` file when it is `X.Y.Z`.
fn read_version_file(p: &Path) -> Option<String> {
    let text = std::fs::read_to_string(p).ok()?;
    let v = text.lines().next()?.trim().to_string();
    is_version(&v).then_some(v)
}

/// The version name a `current`-style link points to (its last path component), if it is one.
/// A plain directory (a Windows junction that reads as one, or a test fixture) has no name.
fn link_version(link: &Path) -> Option<String> {
    let target = std::fs::read_link(link).ok()?;
    let name = target.file_name()?.to_str()?.to_string();
    is_version(&name).then_some(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree(versions: &[&str]) -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        for v in versions {
            let dir = d.path().join(v);
            std::fs::create_dir_all(dir.join("bin")).unwrap();
            std::fs::write(dir.join("VERSION"), format!("{v}\n")).unwrap();
            std::fs::write(dir.join("bin/ai-gent"), "x").unwrap();
        }
        d
    }

    #[cfg(unix)]
    #[test]
    fn package_install_found_through_symlinks() {
        let d = tree(&["2.27.0", "2.28.0"]);
        let home = d.path().canonicalize().unwrap();
        std::os::unix::fs::symlink("2.28.0", home.join("current")).unwrap();
        std::os::unix::fs::symlink("2.27.0", home.join("previous")).unwrap();
        std::fs::write(home.join("pinned"), "2.28.0\n").unwrap();
        // The way ~/.local/bin/ai-gent reaches it: through `current`.
        let r = InstallRoot::discover(&home.join("current/bin/ai-gent"), &Env::default());
        assert!(!r.dev_build);
        assert_eq!(r.version.as_deref(), Some("2.28.0"));
        assert_eq!(r.home.as_deref(), Some(home.as_path()));
        assert_eq!(r.current().as_deref(), Some("2.28.0"));
        assert_eq!(r.previous().as_deref(), Some("2.27.0"));
        assert_eq!(r.pinned().as_deref(), Some("2.28.0"));
        assert_eq!(r.versions(), ["2.27.0", "2.28.0"]);
    }

    #[test]
    fn dev_build_has_no_version_and_falls_back_to_the_env() {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("target/debug")).unwrap();
        std::fs::write(d.path().join("target/debug/ai-gent"), "x").unwrap();
        let env = Env::from_pairs([("AI_GENT_HOME", "/somewhere")]);
        let r = InstallRoot::discover(&d.path().join("target/debug/ai-gent"), &env);
        assert!(r.dev_build && r.version.is_none() && r.version_dir.is_none());
        assert_eq!(r.home, Some(PathBuf::from("/somewhere")));
    }

    #[cfg(unix)]
    #[test]
    fn dangling_current_has_a_name_but_no_directory() {
        let d = tree(&[]);
        std::os::unix::fs::symlink("2.99.0", d.path().join("current")).unwrap();
        let r = InstallRoot {
            home: Some(d.path().to_path_buf()),
            version_dir: None,
            version: None,
            dev_build: true,
        };
        assert_eq!(r.current().as_deref(), Some("2.99.0"));
        assert!(r.versions().is_empty());
        assert!(!d.path().join("current").exists());
    }

    #[test]
    fn garbled_pin_still_pins() {
        let d = tree(&[]);
        std::fs::write(d.path().join("pinned"), "nonsense").unwrap();
        let r = InstallRoot {
            home: Some(d.path().to_path_buf()),
            version_dir: None,
            version: None,
            dev_build: true,
        };
        assert_eq!(r.pinned().as_deref(), Some(""));
    }

    #[test]
    fn version_file_must_be_a_version() {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("bin")).unwrap();
        std::fs::write(d.path().join("VERSION"), "garbage\n").unwrap();
        std::fs::write(d.path().join("bin/ai-gent"), "x").unwrap();
        let r = InstallRoot::discover(&d.path().join("bin/ai-gent"), &Env::default());
        assert!(r.dev_build);
    }
}
