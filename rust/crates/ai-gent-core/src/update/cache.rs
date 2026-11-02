//! The update cache: one line `<epoch-seconds> <X.Y.Z>`, the file `scripts/update-check.sh` uses.
//!
//! Both implementations read and write it, so the shell script and the binary coexist until 3.0.0.
//! The shell reads the first two fields and ignores a third: the day (`YYYY-MM-DD`, UTC) the notice
//! was last printed for that release, which caps the notice at once a day (spec 034, open question 1).
use std::io::Write;
use std::path::{Path, PathBuf};

/// File name inside the state directory.
pub const FILE: &str = "update-check";

/// What the cache holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cache {
    /// Seconds since the epoch of the last refresh; `None` when the field is not a number (stale).
    pub checked: Option<u64>,
    /// The newest release known, `X.Y.Z`.
    pub latest: String,
    /// The UTC day (`YYYY-MM-DD`) the notice for `latest` was last printed, if any.
    pub notified: Option<String>,
}

/// The cache file inside a state directory.
#[must_use]
pub fn path_in(state_dir: &Path) -> PathBuf {
    state_dir.join(FILE)
}

/// Read the cache. `None` when absent, empty or without a version.
#[must_use]
pub fn read(path: &Path) -> Option<Cache> {
    let text = std::fs::read_to_string(path).ok()?;
    parse(text.lines().next()?)
}

fn parse(line: &str) -> Option<Cache> {
    let mut it = line.split_whitespace();
    let checked = it.next()?;
    let tag = it.next()?;
    let latest = tag.strip_prefix('v').unwrap_or(tag).to_string();
    // A tag is what `update --check` compares; anything else is not worth a notice.
    if !super::version::is_version(&latest) {
        return None;
    }
    Some(Cache {
        checked: checked.parse().ok(),
        latest,
        notified: it.next().filter(|d| is_day(d)).map(str::to_string),
    })
}

fn is_day(d: &str) -> bool {
    let b = d.as_bytes();
    b.len() == 10
        && b.iter().enumerate().all(|(i, c)| {
            if i == 4 || i == 7 {
                *c == b'-'
            } else {
                c.is_ascii_digit()
            }
        })
}

/// The UTC day (`YYYY-MM-DD`) of an epoch time.
#[must_use]
pub fn day_of(secs: u64) -> String {
    crate::guard::shadow::iso_utc(secs)[..10].to_string()
}

/// Whether the cache is due for a refresh at `now`: older than `max_age` seconds, unreadable, or
/// stamped in the future (clock skew).
#[must_use]
pub fn is_stale(c: &Cache, now: u64, max_age: u64) -> bool {
    match c.checked {
        None => true,
        Some(t) if t > now => true,
        Some(t) => now - t >= max_age,
    }
}

/// Write the cache atomically: a temporary file in the same directory, then a rename.
///
/// # Errors
/// When the directory or the file cannot be written; the previous cache is then left as it was.
pub fn write(path: &Path, now: u64, latest: &str) -> std::io::Result<()> {
    // A refresh keeps the notice day while the release is the same; a new release notifies afresh.
    let kept = read(path)
        .filter(|c| c.latest == latest)
        .and_then(|c| c.notified);
    write_with(path, now, latest, kept.as_deref())
}

/// Record that the notice for the cached release was printed on `day`, leaving the other fields as
/// they are.
///
/// # Errors
/// When the cache cannot be read or written.
pub fn mark_notified(path: &Path, day: &str) -> std::io::Result<()> {
    let c = read(path).ok_or_else(|| std::io::Error::other("no cache to mark"))?;
    let checked = c
        .checked
        .map_or_else(|| "unknown".to_string(), |t| t.to_string());
    write_line(path, &format!("{checked} {} {day}", c.latest))
}

fn write_with(path: &Path, now: u64, latest: &str, notified: Option<&str>) -> std::io::Result<()> {
    let line = match notified {
        Some(d) => format!("{now} {latest} {d}"),
        None => format!("{now} {latest}"),
    };
    write_line(path, &line)
}

fn write_line(path: &Path, line: &str) -> std::io::Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| std::io::Error::other("cache path has no directory"))?;
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(format!("{FILE}.{}.tmp", std::process::id()));
    let result = (|| {
        let mut f = std::fs::File::create(&tmp)?;
        writeln!(f, "{line}")?;
        f.sync_all()?;
        std::fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_atomic_replace() {
        let d = tempfile::tempdir().unwrap();
        let p = path_in(&d.path().join("nested/ai-gent"));
        assert_eq!(read(&p), None);
        write(&p, 1000, "2.28.0").unwrap();
        write(&p, 2000, "2.29.0").unwrap();
        assert_eq!(
            read(&p),
            Some(Cache {
                checked: Some(2000),
                latest: "2.29.0".into(),
                notified: None
            })
        );
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "2000 2.29.0\n");
        let leftovers = std::fs::read_dir(p.parent().unwrap()).unwrap().count();
        assert_eq!(leftovers, 1, "no temporary file left behind");
    }

    // The format the real scripts/update-check.sh refresh writes (`printf '%s %s\n'`, tag as the
    // API gave it, possibly with a leading v), plus a third field from a later version.
    #[test]
    fn reads_what_the_shell_writes() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join(FILE);
        std::fs::write(&p, "1761990000 v9.9.9\n").unwrap();
        assert_eq!(read(&p).unwrap().latest, "9.9.9");
        std::fs::write(&p, "1761990000 9.9.9 2026-11-01\n").unwrap();
        assert_eq!(read(&p).unwrap().checked, Some(1_761_990_000));
        std::fs::write(&p, "garbage 9.9.9\n").unwrap();
        assert_eq!(read(&p).unwrap().checked, None);
        std::fs::write(&p, "1761990000 nonsense\n").unwrap();
        assert_eq!(read(&p), None);
        std::fs::write(&p, "").unwrap();
        assert_eq!(read(&p), None);
    }

    #[test]
    fn the_shell_reads_what_we_write() {
        // `read -r checked remote <file` splits on whitespace: two fields, one line.
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join(FILE);
        write(&p, 1_761_990_000, "2.28.0").unwrap();
        let text = std::fs::read_to_string(&p).unwrap();
        let fields: Vec<&str> = text.trim_end_matches('\n').split(' ').collect();
        assert_eq!(fields, ["1761990000", "2.28.0"]);
    }

    #[test]
    fn staleness_rules() {
        let c = |t| Cache {
            checked: t,
            latest: "1.0.0".into(),
            notified: None,
        };
        let week = 7 * 86_400;
        assert!(!is_stale(&c(Some(1000)), 1000 + week - 1, week));
        assert!(is_stale(&c(Some(1000)), 1000 + week, week));
        assert!(is_stale(&c(Some(5000)), 1000, week), "future timestamp");
        assert!(is_stale(&c(None), 1000, week));
    }

    #[test]
    fn notice_day_is_kept_for_the_same_release_and_dropped_for_a_new_one() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join(FILE);
        write(&p, 1000, "2.28.0").unwrap();
        mark_notified(&p, "2026-11-02").unwrap();
        assert_eq!(
            std::fs::read_to_string(&p).unwrap(),
            "1000 2.28.0 2026-11-02\n"
        );
        assert_eq!(read(&p).unwrap().notified.as_deref(), Some("2026-11-02"));
        write(&p, 2000, "2.28.0").unwrap(); // refresh, same release
        assert_eq!(read(&p).unwrap().notified.as_deref(), Some("2026-11-02"));
        write(&p, 3000, "2.29.0").unwrap(); // a new release
        assert_eq!(read(&p).unwrap().notified, None);
        std::fs::write(&p, "1000 2.28.0 notaday\n").unwrap();
        assert_eq!(read(&p).unwrap().notified, None);
    }

    #[test]
    fn day_of_is_utc() {
        assert_eq!(day_of(0), "1970-01-01");
        assert_eq!(day_of(1_793_577_599), "2026-11-01");
        assert_eq!(day_of(1_793_577_600), "2026-11-02");
    }
}
