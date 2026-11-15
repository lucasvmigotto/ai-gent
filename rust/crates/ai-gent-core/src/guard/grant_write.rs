//! Writing the autonomy grant: the logic behind `ai-gent autonomy` (parsing the user's duration,
//! datetime, actions and branches; the canonical file text; the atomic write; the git exclude;
//! and reading the grant back for `show`). The file format and what it lifts are in
//! `plugins/git/skills/workflow/references/autonomy.md`; [`super::grant::parse`] is the reader
//! every file written here must satisfy.
use super::grant::{self, FILE};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The longest a grant may last.
pub const MAX_SECS: i64 = 30 * 86_400;

/// Branches no grant may cover: promotion to them is always the user's own step.
pub const PROTECTED: [&str; 4] = ["main", "master", "homolog", "staging"];

/// Default branches of a grant that names none.
pub const DEFAULT_BRANCHES: [&str; 2] = ["develop", "dev"];

/// One action of the vocabulary.
pub struct Action {
    /// The word the file stores.
    pub name: &'static str,
    /// Other words the command accepts for it.
    pub aliases: &'static [&'static str],
    /// What the guards stop asking about and what stays blocked, in one line.
    pub note: &'static str,
    /// What the action is called on each platform, and whether the guards have rules for it.
    pub platforms: &'static [(&'static str, bool)],
}

/// The vocabulary: one table, mirrored in `autonomy.md` and in the `grant` help.
pub const ACTIONS: [Action; 4] = [
    Action {
        name: "push",
        aliases: &[],
        note: "lifts: git push of a work branch or develop; still asked: main, master, homolog, staging, --all, --tags, --delete, +refspec",
        platforms: &[("any git remote", true)],
    },
    Action {
        name: "pr",
        aliases: &["mr"],
        note: "lifts: opening a PR/MR into a covered branch (--base); still asked: any other base",
        platforms: &[
            ("GitHub pull requests", true),
            ("GitLab merge requests", false),
            ("Azure DevOps pull requests", false),
            ("Bitbucket pull requests", false),
        ],
    },
    Action {
        name: "merge",
        aliases: &[],
        note: "lifts: merging a PR through reporun (it refuses a base the grant does not cover); main is always yours",
        platforms: &[
            ("GitHub pull requests", true),
            ("GitLab merge requests", false),
            ("Azure DevOps pull requests", false),
            ("Bitbucket pull requests", false),
        ],
    },
    Action {
        name: "issues",
        aliases: &["workitems"],
        note: "lifts: creating, commenting on and editing issues through reporun; still asked: close, labels, the API",
        platforms: &[
            ("GitHub issues", true),
            ("GitLab issues", false),
            ("Azure DevOps Boards work items", false),
            ("Jira issues", false),
            ("Bitbucket issues", false),
        ],
    },
];

/// The canonical word of an action or alias (case-insensitive).
#[must_use]
pub fn canonical_action(word: &str) -> Option<&'static str> {
    let w = word.to_ascii_lowercase();
    ACTIONS
        .iter()
        .find(|a| a.name == w || a.aliases.contains(&w.as_str()))
        .map(|a| a.name)
}

/// The words of a comma or space separated list.
fn words(list: &str) -> Vec<&str> {
    list.split(|c: char| c == ',' || c.is_whitespace())
        .filter(|w| !w.is_empty())
        .collect()
}

/// Canonical actions of a list, in vocabulary order, without repeats.
pub fn parse_actions(list: &str) -> Result<Vec<&'static str>, String> {
    let mut found = Vec::new();
    for w in words(list) {
        let Some(c) = canonical_action(w) else {
            return Err(format!(
                "unknown action '{w}' (push, pr or mr, merge, issues or workitems)"
            ));
        };
        if !found.contains(&c) {
            found.push(c);
        }
    }
    if found.is_empty() {
        return Err("no action named".into());
    }
    found.sort_by_key(|c| ACTIONS.iter().position(|a| a.name == *c));
    Ok(found)
}

/// Branch names of a list; a name the file could not hold is refused.
pub fn parse_branches(list: &str) -> Result<Vec<String>, String> {
    let mut found: Vec<String> = Vec::new();
    for w in words(list) {
        let ok = !w.starts_with('-')
            && w.chars()
                .all(|c| c.is_ascii_alphanumeric() || "._/-".contains(c));
        if !ok {
            return Err(format!("'{w}' is not a usable branch name"));
        }
        if !found.iter().any(|b| b == w) {
            found.push(w.to_string());
        }
    }
    if found.is_empty() {
        return Err("no branch named".into());
    }
    Ok(found)
}

/// The protected branches among `branches`.
#[must_use]
pub fn protected_in(branches: &[String]) -> Vec<&str> {
    branches
        .iter()
        .map(String::as_str)
        .filter(|b| is_protected(b))
        .collect()
}

/// Whether a branch name is a protected one once normalised: lowercase, without a leading
/// `refs/heads/` and `origin/` (so `Main`, `origin/main` and `refs/heads/main` all count).
#[must_use]
pub fn is_protected(branch: &str) -> bool {
    let low = branch.trim().to_ascii_lowercase();
    let n = low.strip_prefix("refs/heads/").unwrap_or(&low);
    let n = n.strip_prefix("origin/").unwrap_or(n);
    PROTECTED.contains(&n)
}

/// Refuse unless a person is at a terminal: stdin and stdout are terminals and a controlling
/// terminal exists. This is the structural guarantee that an agent (whose shell has pipes, not a
/// terminal) cannot write or remove the grant, whatever text its command uses. Under the
/// `test-support` feature only, `AI_GENT_TEST_TTY=1` stands in for a terminal.
pub fn require_terminal() -> Result<(), String> {
    use std::io::IsTerminal;
    #[cfg(feature = "test-support")]
    if std::env::var("AI_GENT_TEST_TTY").is_ok_and(|v| v == "1") {
        return Ok(());
    }
    let ttys = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
    if ttys && controlling_terminal() {
        Ok(())
    } else {
        Err("refusing: this command writes or removes the autonomy grant, so it needs a real terminal (stdin and stdout on a terminal); run it yourself in your own terminal. An agent cannot run it, and --yes does not change that".into())
    }
}

#[cfg(unix)]
fn controlling_terminal() -> bool {
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/tty")
        .is_ok()
}

#[cfg(not(unix))]
fn controlling_terminal() -> bool {
    true
}

/// A duration such as `30m`, `4h`, `1d` or `1h30m`, in seconds.
pub fn parse_duration(s: &str) -> Result<i64, String> {
    let bad = || format!("'{s}' is not a duration (use 30m, 4h, 1d)");
    let mut total: i64 = 0;
    let mut digits = String::new();
    let mut seen = false;
    for c in s.trim().chars() {
        if c.is_ascii_digit() {
            digits.push(c);
            continue;
        }
        let unit = match c.to_ascii_lowercase() {
            'm' => 60,
            'h' => 3_600,
            'd' => 86_400,
            _ => return Err(bad()),
        };
        if digits.is_empty() || digits.len() > 6 {
            return Err(bad());
        }
        let n: i64 = digits.parse().map_err(|_| bad())?;
        total = total.checked_add(n * unit).ok_or_else(bad)?;
        digits.clear();
        seen = true;
    }
    if !digits.is_empty() || !seen || total == 0 {
        return Err(bad());
    }
    Ok(total)
}

/// Days since 1970-01-01 of a civil date (the inverse of [`grant::format_utc`]'s date part).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn days_in_month(y: i64, m: i64) -> i64 {
    match m {
        2 if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// A zone given in the text: none (local), `Z` or an offset in seconds east of UTC.
enum Zone {
    Local,
    Fixed(i64),
}

fn num(s: &str) -> Option<i64> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

/// A datetime for `--until`: RFC 3339 (`2026-11-07T18:00:00Z`, `…-03:00`, fractions ignored) or
/// local `YYYY-MM-DD HH:MM` (also with `T` and seconds). `local_offset` gives the local UTC
/// offset in seconds at a UTC instant (pass [`local_offset_at`]). Returns UTC seconds.
#[allow(clippy::many_single_char_names)]
pub fn parse_until(s: &str, local_offset: &dyn Fn(i64) -> i64) -> Result<i64, String> {
    let bad = || {
        format!(
            "'{s}' is not a datetime (use YYYY-MM-DD HH:MM in local time, or RFC 3339 such as 2026-11-07T18:00:00Z)"
        )
    };
    let t = s.trim();
    if t.len() < 16 || !t.is_char_boundary(10) {
        return Err(bad());
    }
    let (date, rest) = t.split_at(10);
    let mut rest = rest.chars();
    if !matches!(rest.next(), Some('T' | 't' | ' ')) {
        return Err(bad());
    }
    let rest = rest.as_str();
    let (time, zone) = match rest.find(['Z', 'z', '+', '-']) {
        Some(i) => {
            let z = &rest[i..];
            let zone = if z.eq_ignore_ascii_case("z") {
                Zone::Fixed(0)
            } else {
                let sign = if z.starts_with('-') { -1 } else { 1 };
                let body = &z[1..];
                let (hh, mm) = body.split_once(':').ok_or_else(bad)?;
                let (h, m) = (num(hh).ok_or_else(bad)?, num(mm).ok_or_else(bad)?);
                if hh.len() != 2 || mm.len() != 2 || h > 23 || m > 59 {
                    return Err(bad());
                }
                Zone::Fixed(sign * (h * 3_600 + m * 60))
            };
            (&rest[..i], zone)
        }
        None => (rest, Zone::Local),
    };
    let time = time.split('.').next().unwrap_or(time);
    let dp: Vec<&str> = date.split('-').collect();
    let tp: Vec<&str> = time.split(':').collect();
    if dp.len() != 3 || !(2..=3).contains(&tp.len()) {
        return Err(bad());
    }
    let (y, mo, d) = (
        num(dp[0]).ok_or_else(bad)?,
        num(dp[1]).ok_or_else(bad)?,
        num(dp[2]).ok_or_else(bad)?,
    );
    let (h, mi) = (num(tp[0]).ok_or_else(bad)?, num(tp[1]).ok_or_else(bad)?);
    let sec = match tp.get(2) {
        Some(x) => num(x).ok_or_else(bad)?,
        None => 0,
    };
    if dp[0].len() != 4
        || !(1..=12).contains(&mo)
        || d < 1
        || d > days_in_month(y, mo)
        || h > 23
        || mi > 59
        || sec > 59
    {
        return Err(bad());
    }
    let wall = days_from_civil(y, mo, d) * 86_400 + h * 3_600 + mi * 60 + sec;
    Ok(match zone {
        Zone::Fixed(off) => wall - off,
        Zone::Local => {
            // Two steps so the offset is the one in force at the resulting instant.
            let first = wall - local_offset(wall);
            wall - local_offset(first)
        }
    })
}

/// Refuse an expiry that is not ahead or lies more than 30 days away.
pub fn check_expiry(now: i64, expires: i64) -> Result<(), String> {
    if expires <= now {
        return Err("the expiry is in the past".into());
    }
    if expires - now > MAX_SECS {
        return Err("a grant lasts at most 30 days".into());
    }
    Ok(())
}

/// The file's text, in the one form the parser accepts.
#[must_use]
pub fn render(expires: i64, actions: &[&str], branches: &[String]) -> String {
    format!(
        "expires: {}\nbranches: {}\nactions: {}\n",
        grant::format_utc(expires),
        branches.join(" "),
        actions.join(" ")
    )
}

/// `4h 59m`, `2d 3h`, `45s`: the time left, rounded down to two units.
#[must_use]
pub fn human_left(secs: i64) -> String {
    let (d, h, m) = (secs / 86_400, secs % 86_400 / 3_600, secs % 3_600 / 60);
    match (d, h, m) {
        (0, 0, 0) => format!("{secs}s"),
        (0, 0, m) => format!("{m}m"),
        (0, h, m) => format!("{h}h {m}m"),
        (d, h, _) => format!("{d}d {h}h"),
    }
}

/// `+02:00` or `-03:30` for an offset in seconds.
#[must_use]
pub fn fmt_offset(off: i64) -> String {
    let a = off.abs();
    format!(
        "{}{:02}:{:02}",
        if off < 0 { '-' } else { '+' },
        a / 3_600,
        a % 3_600 / 60
    )
}

/// `2026-11-07 18:00 -03:00` for an instant at a given offset.
#[must_use]
pub fn fmt_local(utc: i64, off: i64) -> String {
    let t = grant::format_utc(utc + off);
    format!("{} {} {}", &t[..10], &t[11..16], fmt_offset(off))
}

/// The local UTC offset in seconds at the UTC instant `utc` (0 when it cannot be told).
#[must_use]
pub fn local_offset_at(utc: i64) -> i64 {
    #[cfg(feature = "test-support")]
    if let Some(o) = std::env::var("AI_GENT_TEST_UTC_OFFSET")
        .ok()
        .and_then(|v| v.parse().ok())
    {
        return o;
    }
    let run = |prog: &str, args: &[&str]| -> Option<String> {
        let out = Command::new(prog)
            .args(args)
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
    };
    let at = utc.to_string();
    let text = if cfg!(windows) {
        run(
            "powershell",
            &[
                "-NoProfile",
                "-Command",
                &format!(
                    "[int][TimeZoneInfo]::Local.GetUtcOffset([DateTimeOffset]::FromUnixTimeSeconds({utc})).TotalSeconds"
                ),
            ],
        )
        .and_then(|v| v.parse::<i64>().ok())
    } else {
        // GNU date, then BSD date.
        run("date", &["-d", &format!("@{at}"), "+%z"])
            .or_else(|| run("date", &["-r", &at, "+%z"]))
            .and_then(|z| parse_zone(&z))
    };
    text.unwrap_or(0)
}

/// `+0200` / `-0330` as seconds east of UTC.
fn parse_zone(z: &str) -> Option<i64> {
    let sign = match z.as_bytes().first()? {
        b'+' => 1,
        b'-' => -1,
        _ => return None,
    };
    let d = z.get(1..5)?;
    let (h, m) = (num(&d[..2])?, num(&d[2..])?);
    Some(sign * (h * 3_600 + m * 60))
}

/// The current time in UTC seconds (`AI_GENT_TEST_NOW` overrides it under `test-support`).
#[must_use]
pub fn now_secs() -> i64 {
    #[cfg(feature = "test-support")]
    if let Some(n) = std::env::var("AI_GENT_TEST_NOW")
        .ok()
        .and_then(|v| v.parse().ok())
    {
        return n;
    }
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0))
}

fn git_out(dir: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    out.status.success().then(|| {
        String::from_utf8_lossy(&out.stdout)
            .trim_end_matches('\n')
            .to_string()
    })
}

/// The repository root of `dir`.
pub fn repo_root(dir: &Path) -> Result<PathBuf, String> {
    git_out(dir, &["rev-parse", "--show-toplevel"])
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| "not inside a git repository".to_string())
}

/// The grant file of a repository.
#[must_use]
pub fn path_in(root: &Path) -> PathBuf {
    root.join(FILE)
}

/// Write `text` to the grant file: a temp file beside it (mode 0600 on unix), then a rename.
pub fn write_atomic(root: &Path, text: &str) -> Result<PathBuf, String> {
    use std::io::Write;
    let target = path_in(root);
    let dir = target.parent().ok_or("grant path has no parent")?;
    std::fs::create_dir_all(dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let tmp = dir.join(format!(".autonomy.tmp{}", std::process::id()));
    let _ = std::fs::remove_file(&tmp);
    let mut opts = std::fs::OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let res = opts
        .open(&tmp)
        .and_then(|mut f| f.write_all(text.as_bytes()).and_then(|()| f.sync_all()))
        .and_then(|()| std::fs::rename(&tmp, &target));
    if let Err(e) = res {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("cannot write {}: {e}", target.display()));
    }
    Ok(target)
}

/// Whether git already ignores the grant file (even before it exists).
#[must_use]
pub fn is_ignored(root: &Path) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["check-ignore", "-q", "--no-index", FILE])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Make git ignore the grant file: nothing when it already does, else a line in
/// `.git/info/exclude`. Returns the line added.
pub fn ensure_excluded(root: &Path) -> Result<Option<String>, String> {
    if is_ignored(root) {
        return Ok(None);
    }
    let rel = git_out(root, &["rev-parse", "--git-path", "info/exclude"])
        .ok_or("cannot find .git/info/exclude")?;
    let path = if Path::new(&rel).is_absolute() {
        PathBuf::from(rel)
    } else {
        root.join(rel)
    };
    let mut text = std::fs::read_to_string(&path).unwrap_or_default();
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    let _ = writeln!(text, "{FILE}");
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p).map_err(|e| format!("cannot create {}: {e}", p.display()))?;
    }
    std::fs::write(&path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    Ok(Some(FILE.to_string()))
}

/// What `show` found.
#[derive(Debug, PartialEq, Eq)]
pub enum Status {
    /// An active grant.
    Active {
        /// Expiry, UTC seconds.
        expires: i64,
        /// Actions as written.
        actions: Vec<String>,
        /// Branches covered.
        branches: Vec<String>,
    },
    /// No file.
    Missing,
    /// The file is there but `expires` has passed.
    Expired(i64),
    /// The file is there but is not a grant (the reason).
    Unreadable(String),
}

/// UTC seconds of `YYYY-MM-DDTHH:MM:SSZ` (the grant's own form).
fn epoch_of(expires: &str) -> Option<i64> {
    parse_until(expires, &|_| 0).ok()
}

/// Read the grant of a repository root, as of `now`.
#[must_use]
pub fn inspect(root: &Path, now: i64) -> Status {
    let text = match std::fs::read_to_string(path_in(root)) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Status::Missing,
        Err(e) => return Status::Unreadable(e.to_string()),
    };
    inspect_text(&text, now)
}

/// [`inspect`] on the file's text.
#[must_use]
pub fn inspect_text(text: &str, now: i64) -> Status {
    if let Some(g) = grant::parse(text, &grant::format_utc(now)) {
        let expires = text
            .lines()
            .find_map(|l| l.strip_prefix("expires:"))
            .and_then(|v| epoch_of(v.trim()))
            .unwrap_or(now);
        return Status::Active {
            expires,
            actions: g.actions().to_vec(),
            branches: g.branches().to_vec(),
        };
    }
    match text
        .lines()
        .find_map(|l| l.strip_prefix("expires:"))
        .map(|v| v.trim_start().trim_end_matches('\r'))
        .filter(|v| v.len() == 20 && v.ends_with('Z'))
        .and_then(epoch_of)
    {
        Some(e) => Status::Expired(e),
        None => Status::Unreadable("no valid expires: line (YYYY-MM-DDTHH:MM:SSZ)".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_793_966_400; // 2026-11-06T12:00:00Z

    fn fixed(off: i64) -> impl Fn(i64) -> i64 {
        move |_| off
    }

    #[test]
    fn durations() {
        assert_eq!(parse_duration("30m"), Ok(1_800));
        assert_eq!(parse_duration("4h"), Ok(14_400));
        assert_eq!(parse_duration("1d"), Ok(86_400));
        assert_eq!(parse_duration("1h30m"), Ok(5_400));
        for bad in ["", "0m", "h", "4", "4x", "-1h", "1.5h", "99999999d", "4h 1"] {
            assert!(parse_duration(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn rfc3339_with_fixed_offsets() {
        let none = fixed(0);
        assert_eq!(parse_until("2026-11-06T12:00:00Z", &none), Ok(NOW));
        assert_eq!(parse_until("2026-11-06T09:00:00-03:00", &none), Ok(NOW));
        assert_eq!(parse_until("2026-11-06T14:00:00+02:00", &none), Ok(NOW));
        assert_eq!(parse_until("2026-11-06T12:00:00.250Z", &none), Ok(NOW));
        assert_eq!(parse_until("2026-11-06T12:00Z", &none), Ok(NOW));
    }

    #[test]
    fn local_datetimes_use_the_local_offset() {
        assert_eq!(parse_until("2026-11-06 09:00", &fixed(-3 * 3_600)), Ok(NOW));
        assert_eq!(parse_until("2026-11-06T09:00", &fixed(-3 * 3_600)), Ok(NOW));
        assert_eq!(
            parse_until("2026-11-06 14:00:00", &fixed(2 * 3_600)),
            Ok(NOW)
        );
        assert_eq!(parse_until("2026-11-06 12:00", &fixed(0)), Ok(NOW));
        // 30 minutes east: +05:30.
        assert_eq!(parse_until("2026-11-06 17:30", &fixed(19_800)), Ok(NOW));
        // An explicit zone ignores the local offset.
        assert_eq!(parse_until("2026-11-06T12:00:00Z", &fixed(7_200)), Ok(NOW));
    }

    #[test]
    fn bad_datetimes() {
        for bad in [
            "",
            "tomorrow",
            "2026-13-01 10:00",
            "2026-02-30 10:00",
            "2026-11-06 24:00",
            "2026-11-06 10:60",
            "2026-11-06 10",
            "2026-11-06T10:00:00+0200",
            "2026-11-06T10:00:00+25:00",
            "26-11-06 10:00",
        ] {
            assert!(parse_until(bad, &fixed(0)).is_err(), "{bad:?}");
        }
        assert!(parse_until("2028-02-29 10:00", &fixed(0)).is_ok());
    }

    #[test]
    fn expiry_window() {
        assert!(check_expiry(NOW, NOW + 1).is_ok());
        assert!(check_expiry(NOW, NOW + MAX_SECS).is_ok());
        assert!(check_expiry(NOW, NOW).is_err());
        assert!(check_expiry(NOW, NOW - 60).unwrap_err().contains("past"));
        assert!(
            check_expiry(NOW, NOW + MAX_SECS + 1)
                .unwrap_err()
                .contains("30 days")
        );
    }

    #[test]
    fn actions_and_aliases() {
        assert_eq!(parse_actions("push,mr"), Ok(vec!["push", "pr"]));
        assert_eq!(
            parse_actions("workitems, MR merge push"),
            Ok(vec!["push", "pr", "merge", "issues"])
        );
        assert_eq!(parse_actions("pr,mr,pr"), Ok(vec!["pr"]));
        assert!(parse_actions("push,deploy").unwrap_err().contains("deploy"));
        assert!(parse_actions("").is_err());
    }

    #[test]
    fn protected_is_normalised() {
        for b in [
            "Main",
            "origin/main",
            "refs/heads/main",
            "refs/heads/Origin/MASTER",
            "STAGING",
            "origin/homolog",
        ] {
            assert!(is_protected(b), "{b}");
        }
        for b in ["develop", "dev", "mainline", "feat/main", "origin/develop"] {
            assert!(!is_protected(b), "{b}");
        }
        let b = vec![
            "Main".to_string(),
            "dev".to_string(),
            "origin/staging".to_string(),
        ];
        assert_eq!(protected_in(&b), ["Main", "origin/staging"]);
    }

    #[test]
    fn branches_and_protected() {
        let b = parse_branches("develop, dev").unwrap();
        assert_eq!(b, ["develop", "dev"]);
        assert!(protected_in(&b).is_empty());
        let b = parse_branches("dev,main,staging,feat/x").unwrap();
        assert_eq!(protected_in(&b), ["main", "staging"]);
        for bad in ["-x", "a;b", "a$b", "a'b"] {
            assert!(parse_branches(bad).is_err(), "{bad}");
        }
        assert!(parse_branches("").is_err());
    }

    #[test]
    fn every_written_file_parses_as_active_with_the_same_content() {
        let combos: [(&[&str], &[&str]); 4] = [
            (&["push"], &["develop", "dev"]),
            (&["push", "pr", "merge", "issues"], &["release/1.x", "dev"]),
            (&["issues"], &["trunk"]),
            (&["pr", "merge"], &["develop"]),
        ];
        for (actions, branches) in combos {
            let branches: Vec<String> = branches.iter().map(ToString::to_string).collect();
            let text = render(NOW + 3_600, actions, &branches);
            let g = grant::parse(&text, &grant::format_utc(NOW)).expect("active");
            assert_eq!(g.branches(), branches.as_slice(), "{text}");
            assert_eq!(g.actions(), actions);
            match inspect_text(&text, NOW) {
                Status::Active {
                    expires,
                    actions: a,
                    branches: b,
                } => {
                    assert_eq!(expires, NOW + 3_600);
                    assert_eq!(a, actions);
                    assert_eq!(b, branches);
                }
                other => panic!("{other:?}"),
            }
        }
    }

    #[test]
    fn the_text_is_the_documented_format() {
        let text = render(
            NOW + 6 * 3_600,
            &["push", "pr"],
            &["develop".into(), "dev".into()],
        );
        assert_eq!(
            text,
            "expires: 2026-11-06T18:00:00Z\nbranches: develop dev\nactions: push pr\n"
        );
    }

    #[test]
    fn inspecting_text() {
        let t = |s: &str| inspect_text(s, NOW);
        assert!(matches!(
            t("expires: 2026-11-06T18:00:00Z\nactions: push\n"),
            Status::Active { .. }
        ));
        assert_eq!(
            t("expires: 2026-11-06T11:00:00Z\nactions: push\n"),
            Status::Expired(NOW - 3_600)
        );
        assert!(matches!(t("actions: push\n"), Status::Unreadable(_)));
        assert!(matches!(
            t("expires: tomorrow\nactions: push\n"),
            Status::Unreadable(_)
        ));
    }

    #[test]
    fn display_helpers() {
        assert_eq!(human_left(45), "45s");
        assert_eq!(human_left(3_599), "59m");
        assert_eq!(human_left(4 * 3_600 + 59 * 60), "4h 59m");
        assert_eq!(human_left(2 * 86_400 + 3 * 3_600), "2d 3h");
        assert_eq!(fmt_offset(-3 * 3_600), "-03:00");
        assert_eq!(fmt_offset(19_800), "+05:30");
        assert_eq!(fmt_local(NOW, -3 * 3_600), "2026-11-06 09:00 -03:00");
        assert_eq!(parse_zone("-0330"), Some(-12_600));
        assert_eq!(parse_zone("+0000"), Some(0));
        assert_eq!(parse_zone("x"), None);
    }

    fn repo() -> tempfile::TempDir {
        let d = tempfile::tempdir().unwrap();
        let ok = Command::new("git")
            .args(["init", "-q", "-b", "dev"])
            .arg(d.path())
            .status()
            .unwrap()
            .success();
        assert!(ok);
        d
    }

    #[test]
    fn the_write_is_atomic_private_and_replaces() {
        let d = repo();
        let root = repo_root(d.path()).unwrap();
        let p = write_atomic(&root, "expires: a\n").unwrap();
        write_atomic(&root, "expires: b\n").unwrap();
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "expires: b\n");
        let names: Vec<_> = std::fs::read_dir(p.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, ["autonomy"], "no temp file is left");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&p).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
        }
    }

    #[test]
    fn the_exclude_line_is_added_once() {
        let d = repo();
        let root = repo_root(d.path()).unwrap();
        assert_eq!(ensure_excluded(&root), Ok(Some(FILE.to_string())));
        assert_eq!(ensure_excluded(&root), Ok(None));
        let ex = std::fs::read_to_string(root.join(".git/info/exclude")).unwrap();
        assert_eq!(ex.matches(FILE).count(), 1);
    }

    #[test]
    fn an_already_ignored_file_is_left_alone() {
        let d = repo();
        let root = repo_root(d.path()).unwrap();
        std::fs::write(root.join(".gitignore"), ".ai-gent/\n").unwrap();
        assert_eq!(ensure_excluded(&root), Ok(None));
    }

    #[test]
    fn inspect_reads_the_file() {
        let d = repo();
        let root = repo_root(d.path()).unwrap();
        assert_eq!(inspect(&root, NOW), Status::Missing);
        write_atomic(&root, &render(NOW + 60, &["push"], &["dev".into()])).unwrap();
        assert!(matches!(inspect(&root, NOW), Status::Active { .. }));
        assert_eq!(inspect(&root, NOW + 120), Status::Expired(NOW + 60));
    }
}
