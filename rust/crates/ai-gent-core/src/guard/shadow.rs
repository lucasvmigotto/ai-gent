//! Shadow mode (spec 033, FR-009): record how the evaluator compares with the shell guard that
//! decided, without ever storing the command text.
//!
//! The record holds the decisions, a short hash of the command, its length and, when its first
//! words are plain tokens, the verb (`git push`). `AI_GENT_SHADOW_CAPTURE` (a redacted copy of the
//! command on disagreement) is not implemented yet: `capture` is always `null`.
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;
use std::io::Write;
use std::path::{Path, PathBuf};

/// One line of `guard-shadow.jsonl` (`contracts/shadow-log.md`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Record {
    /// Shape version.
    pub schema: u32,
    /// UTC time, `YYYY-MM-DDTHH:MM:SSZ`.
    pub ts: String,
    /// Release of the binary.
    pub release: String,
    /// Target triple of the binary.
    pub target: String,
    /// Guard id (or `all`).
    pub guard: String,
    /// Tool name of the event.
    pub tool: String,
    /// Shell syntax used.
    pub shell: String,
    /// What the shell guard decided.
    pub shell_decision: String,
    /// What the evaluator decided.
    pub rust_decision: String,
    /// The decisions are equal.
    pub agree: bool,
    /// The reasons are equal; `null` when the shell reason was not passed.
    pub reason_equal: Option<bool>,
    /// A divergence known in advance (never set by the evaluator itself).
    pub expected_divergence: bool,
    /// Reason id of the evaluator's decision, `null` for none.
    pub rule: Option<String>,
    /// First 16 hex digits of the SHA-256 of the command, `null` for non-shell tools.
    pub cmd_sha: Option<String>,
    /// Length of the command in bytes.
    pub cmd_len: usize,
    /// `git push`, `psql`: only when the first words are plain tokens.
    pub verb: Option<String>,
    /// Time the evaluation took.
    pub latency_us: u64,
    /// Redacted command on disagreement (not implemented: always `null`).
    pub capture: Option<String>,
}

impl Record {
    /// A disagreement the evaluator was not expected to have.
    #[must_use]
    pub fn unexplained(&self) -> bool {
        (!self.agree || self.reason_equal == Some(false)) && !self.expected_divergence
    }
}

/// SHA-256 of `data` (FIPS 180-4), as lowercase hex.
#[must_use]
#[allow(clippy::too_many_lines)] // the round constants
pub fn sha256_hex(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a_2f98,
        0x7137_4491,
        0xb5c0_fbcf,
        0xe9b5_dba5,
        0x3956_c25b,
        0x59f1_11f1,
        0x923f_82a4,
        0xab1c_5ed5,
        0xd807_aa98,
        0x1283_5b01,
        0x2431_85be,
        0x550c_7dc3,
        0x72be_5d74,
        0x80de_b1fe,
        0x9bdc_06a7,
        0xc19b_f174,
        0xe49b_69c1,
        0xefbe_4786,
        0x0fc1_9dc6,
        0x240c_a1cc,
        0x2de9_2c6f,
        0x4a74_84aa,
        0x5cb0_a9dc,
        0x76f9_88da,
        0x983e_5152,
        0xa831_c66d,
        0xb003_27c8,
        0xbf59_7fc7,
        0xc6e0_0bf3,
        0xd5a7_9147,
        0x06ca_6351,
        0x1429_2967,
        0x27b7_0a85,
        0x2e1b_2138,
        0x4d2c_6dfc,
        0x5338_0d13,
        0x650a_7354,
        0x766a_0abb,
        0x81c2_c92e,
        0x9272_2c85,
        0xa2bf_e8a1,
        0xa81a_664b,
        0xc24b_8b70,
        0xc76c_51a3,
        0xd192_e819,
        0xd699_0624,
        0xf40e_3585,
        0x106a_a070,
        0x19a4_c116,
        0x1e37_6c08,
        0x2748_774c,
        0x34b0_bcb5,
        0x391c_0cb3,
        0x4ed8_aa4a,
        0x5b9c_ca4f,
        0x682e_6ff3,
        0x748f_82ee,
        0x78a5_636f,
        0x84c8_7814,
        0x8cc7_0208,
        0x90be_fffa,
        0xa450_6ceb,
        0xbef9_a3f7,
        0xc671_78f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09_e667,
        0xbb67_ae85,
        0x3c6e_f372,
        0xa54f_f53a,
        0x510e_527f,
        0x9b05_688c,
        0x1f83_d9ab,
        0x5be0_cd19,
    ];
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&((data.len() as u64) * 8).to_be_bytes());
    for block in msg.chunks(64) {
        let mut w = [0u32; 64];
        for (i, word) in block.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let mut v = h;
        for i in 0..64 {
            let s1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
            let ch = (v[4] & v[5]) ^ (!v[4] & v[6]);
            let t1 = v[7]
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
            let maj = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
            let t2 = s0.wrapping_add(maj);
            v = [
                t1.wrapping_add(t2),
                v[0],
                v[1],
                v[2],
                v[3].wrapping_add(t1),
                v[4],
                v[5],
                v[6],
            ];
        }
        for (x, y) in h.iter_mut().zip(v) {
            *x = x.wrapping_add(y);
        }
    }
    h.iter().fold(String::new(), |mut out, x| {
        let _ = write!(out, "{x:08x}");
        out
    })
}

/// The verb of a command: its first word, plus the second when both are plain tokens.
#[must_use]
pub fn verb(cmd: &str) -> Option<String> {
    let plain = |w: &str| {
        let mut c = w.chars();
        c.next().is_some_and(|f| f.is_ascii_lowercase())
            && c.all(|x| x.is_ascii_lowercase() || x.is_ascii_digit() || x == '-')
    };
    let mut words = cmd.split_whitespace();
    let first = words.next().filter(|w| plain(w))?;
    match words.next().filter(|w| plain(w)) {
        Some(second) => Some(format!("{first} {second}")),
        None => Some(first.to_string()),
    }
}

/// Seconds since the Unix epoch to `YYYY-MM-DDTHH:MM:SSZ`.
#[must_use]
pub fn iso_utc(secs: u64) -> String {
    let (days, rem) = (secs / 86_400, secs % 86_400);
    // Howard Hinnant's civil_from_days
    let z = i64::try_from(days).unwrap_or(0) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// `YYYY-MM-DDTHH:MM:SSZ` to seconds since the Unix epoch.
#[must_use]
#[allow(clippy::many_single_char_names)] // date arithmetic
pub fn parse_iso_utc(s: &str) -> Option<u64> {
    let b = s.as_bytes();
    if b.len() != 20 || b[4] != b'-' || b[7] != b'-' || b[10] != b'T' || b[19] != b'Z' {
        return None;
    }
    let n = |r: std::ops::Range<usize>| s.get(r)?.parse::<i64>().ok();
    let (y, m, d) = (n(0..4)?, n(5..7)?, n(8..10)?);
    let (hh, mm, ss) = (n(11..13)?, n(14..16)?, n(17..19)?);
    let y = y - i64::from(m <= 2);
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    u64::try_from(days * 86_400 + hh * 3600 + mm * 60 + ss).ok()
}

/// Parse `90s`, `15m`, `24h` or `7d` into seconds.
#[must_use]
pub fn parse_duration(s: &str) -> Option<u64> {
    let (num, unit) = s.split_at(s.len().checked_sub(1)?);
    let n: u64 = num.parse().ok()?;
    let mult = match unit {
        "s" => 1,
        "m" => 60,
        "h" => 3600,
        "d" => 86_400,
        _ => return None,
    };
    n.checked_mul(mult)
}

/// The directory holding the shadow log: `${XDG_STATE_HOME:-~/.local/state}/ai-gent`, on Windows
/// `%LOCALAPPDATA%\ai-gent\state`.
#[must_use]
pub fn log_dir(get: impl Fn(&str) -> Option<PathBuf>) -> Option<PathBuf> {
    if cfg!(windows) {
        return get("LOCALAPPDATA").map(|p| p.join("ai-gent").join("state"));
    }
    if let Some(p) = get("XDG_STATE_HOME") {
        return Some(p.join("ai-gent"));
    }
    get("HOME").map(|p| p.join(".local/state/ai-gent"))
}

/// The log directory from the real environment.
#[must_use]
pub fn default_log_dir() -> Option<PathBuf> {
    log_dir(|k| {
        std::env::var_os(k)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    })
}

/// Log file name.
pub const LOG_NAME: &str = "guard-shadow.jsonl";
const ROTATED_NAME: &str = "guard-shadow.1.jsonl";
const ROTATE_AT: u64 = 5 * 1024 * 1024;

/// Append one record (one `write` per line), rotating at 5 MiB. The directory is 0700 and the file
/// 0600 on Unix.
///
/// # Errors
/// When the directory or file cannot be created or written.
pub fn append(dir: &Path, rec: &Record) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)?;
    }
    #[cfg(not(unix))]
    std::fs::create_dir_all(dir)?;
    let path = dir.join(LOG_NAME);
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > ROTATE_AT) {
        std::fs::rename(&path, dir.join(ROTATED_NAME))?;
    }
    let mut opts = std::fs::OpenOptions::new();
    opts.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut line = serde_json::to_string(rec).map_err(std::io::Error::other)?;
    line.push('\n');
    opts.open(path)?.write_all(line.as_bytes())
}

/// Every parsable record of the log (the rotated file first); lines that do not parse are skipped.
#[must_use]
pub fn read_all(dir: &Path) -> Vec<Record> {
    let mut out = Vec::new();
    for name in [ROTATED_NAME, LOG_NAME] {
        if let Ok(text) = std::fs::read_to_string(dir.join(name)) {
            out.extend(
                text.lines()
                    .filter_map(|l| serde_json::from_str::<Record>(l).ok()),
            );
        }
    }
    out
}

/// Counts for one guard.
#[derive(Debug, Default, Clone, Serialize, PartialEq, Eq)]
pub struct GuardCounts {
    /// Records.
    pub evaluations: usize,
    /// Decisions (and known reasons) equal.
    pub agree: usize,
    /// Unexplained disagreements.
    pub disagree: usize,
    /// Disagreements flagged as expected in advance.
    pub expected_divergence: usize,
}

/// What `guard shadow-report` shows.
#[derive(Debug, Serialize, PartialEq)]
pub struct Report {
    /// Start of the window, or `null` for all time.
    pub since: Option<String>,
    /// Counts by guard id.
    pub guards: std::collections::BTreeMap<String, GuardCounts>,
    /// The unexplained disagreements.
    pub disagreements: Vec<Record>,
}

/// Summarize `records`, keeping those at or after `since` (epoch seconds) and of `guard`.
#[must_use]
pub fn report(records: &[Record], since: Option<u64>, guard: Option<&str>) -> Report {
    let mut r = Report {
        since: since.map(iso_utc),
        guards: std::collections::BTreeMap::new(),
        disagreements: Vec::new(),
    };
    for rec in records {
        if guard.is_some_and(|g| g != rec.guard) {
            continue;
        }
        if since.is_some_and(|s| parse_iso_utc(&rec.ts).is_some_and(|t| t < s)) {
            continue;
        }
        let c = r.guards.entry(rec.guard.clone()).or_default();
        c.evaluations += 1;
        if rec.unexplained() {
            c.disagree += 1;
            r.disagreements.push(rec.clone());
        } else if !rec.agree || rec.reason_equal == Some(false) {
            c.expected_divergence += 1;
        } else {
            c.agree += 1;
        }
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(agree: bool, reason_equal: Option<bool>, expected: bool) -> Record {
        Record {
            schema: 1,
            ts: "2026-11-01T12:00:00Z".into(),
            release: "dev".into(),
            target: "t".into(),
            guard: "git".into(),
            tool: "Bash".into(),
            shell: "sh".into(),
            shell_decision: "ask".into(),
            rust_decision: if agree { "ask" } else { "none" }.into(),
            agree,
            reason_equal,
            expected_divergence: expected,
            rule: None,
            cmd_sha: None,
            cmd_len: 0,
            verb: None,
            latency_us: 1,
            capture: None,
        }
    }

    #[test]
    fn sha256_known_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let long = vec![b'a'; 1000];
        assert_eq!(
            sha256_hex(&long),
            "41edece42d63e8d9bf515a9ba6932e1c20cbc9f5a5d134645adb5db1b9737ea3"
        );
    }

    #[test]
    fn verb_is_plain_tokens_only() {
        assert_eq!(verb("git push origin x").as_deref(), Some("git push"));
        assert_eq!(verb("psql -h x").as_deref(), Some("psql"));
        assert_eq!(verb("ls").as_deref(), Some("ls"));
        assert_eq!(verb("TOKEN=abc git push"), None);
        assert_eq!(verb("ghp_abc123 push"), None);
        assert_eq!(verb("git Push").as_deref(), Some("git"));
    }

    #[test]
    fn time_round_trips() {
        for secs in [0, 86_399, 951_782_400, 1_793_534_400, 1_709_164_800] {
            assert_eq!(parse_iso_utc(&iso_utc(secs)), Some(secs), "{secs}");
        }
        assert_eq!(iso_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso_utc(1_709_164_800), "2024-02-29T00:00:00Z");
        assert_eq!(parse_duration("7d"), Some(604_800));
        assert_eq!(parse_duration("x"), None);
    }

    #[test]
    fn report_counts_and_explains() {
        let rs = vec![
            rec(true, Some(true), false),
            rec(true, None, false),
            rec(false, Some(false), false),
            rec(false, Some(false), true),
            rec(true, Some(false), false),
        ];
        let r = report(&rs, None, None);
        let c = &r.guards["git"];
        assert_eq!(
            (c.evaluations, c.agree, c.disagree, c.expected_divergence),
            (5, 2, 2, 1)
        );
        assert_eq!(r.disagreements.len(), 2);
        let none = report(
            &rs,
            Some(parse_iso_utc("2026-11-02T00:00:00Z").unwrap()),
            None,
        );
        assert!(none.guards.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn append_writes_one_private_line_and_rotates() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("state");
        append(&log, &rec(true, None, false)).unwrap();
        append(&log, &rec(false, None, false)).unwrap();
        let text = std::fs::read_to_string(log.join(LOG_NAME)).unwrap();
        assert_eq!(text.lines().count(), 2);
        assert!(text.ends_with('\n'));
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&log), 0o700);
        assert_eq!(mode(&log.join(LOG_NAME)), 0o600);
        // rotation
        std::fs::write(
            log.join(LOG_NAME),
            vec![b'x'; usize::try_from(ROTATE_AT + 1).unwrap()],
        )
        .unwrap();
        append(&log, &rec(true, None, false)).unwrap();
        assert!(log.join(ROTATED_NAME).is_file());
        assert_eq!(read_all(&log).len(), 1);
    }

    #[test]
    fn a_record_carries_no_command_text() {
        let json = serde_json::to_string(&rec(true, None, false)).unwrap();
        assert!(json.contains("\"capture\":null"));
    }
}
