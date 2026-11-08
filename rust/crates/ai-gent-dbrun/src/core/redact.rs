//! The shared redactor (spec 043 FR-008, T004): host, user and password of the profile in use are
//! replaced with `<host>`, `<user>` and `<password>` in every error and log line, for every connector.
//!
//! Stricter than the Python `redact()` on purpose, because it sits on a credential path and a driver
//! quotes values back in many shapes (each strictness is listed in `specs/043-dbrun-port/differences.md`):
//!
//! - every secret is also matched in the forms a driver quotes it: as given, trimmed, JSON-escaped,
//!   quotes doubled, backslash-escaped (all special characters, or only the ones one library escapes,
//!   as libpq and the Python repr do), and percent-encoded in every common dialect (which characters
//!   are kept, upper or lower case hex, space as `%20` or `+`);
//! - the password is replaced wherever it occurs (no word boundary), so a password that is all digits
//!   is just text (the review finding on `oracle_thin.rs` was a profile that gave it as a number);
//! - host and user keep Python's whole-word rule (so `db` is not cut out of `database`) but match
//!   ASCII letters in any case, and a value of any length counts: a one-character user is redacted,
//!   not skipped;
//! - one pass over the original text: overlapping or touching hits merge into one label, so a
//!   replacement can never expose a piece of another secret or create a new match. The pass marks
//!   bytes of the text, so time is linear in the text length times the number of patterns and memory
//!   is one byte per text byte, however many hits there are.
//!
//! An empty value is never a needle; the loopback names are not secrets and stay readable.
use std::fmt::Write as _;

/// The loopback names that are not secrets.
const OPEN_HOSTS: &[&str] = &["127.0.0.1", "localhost", "::1"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// Replaced wherever it occurs.
    Anywhere,
    /// Replaced only as a whole word (Python's rule), ASCII case ignored.
    Word,
}

#[derive(Clone)]
struct Needle {
    text: String,
    rank: u8,
    kind: Kind,
}

/// Rank of each label; the lowest rank labels a merged hit.
const LABELS: [&str; 3] = ["<password>", "<host>", "<user>"];
const NONE: u8 = u8::MAX;

/// Replaces a profile's secrets in text. `Debug` never prints a value.
#[derive(Clone, Default)]
pub struct Redactor {
    needles: Vec<Needle>,
}

impl std::fmt::Debug for Redactor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Redactor({} patterns)", self.needles.len())
    }
}

/// Percent-encodes `value`: bytes in `keep` (plus ASCII letters and digits) stay, a space becomes `+`
/// when `plus`, everything else is `%XX` in the chosen case.
fn percent_form(value: &str, keep: &str, lower: bool, plus: bool) -> String {
    let mut out = String::new();
    for b in value.bytes() {
        if b.is_ascii_alphanumeric() || keep.as_bytes().contains(&b) {
            out.push(char::from(b));
        } else if b == b' ' && plus {
            out.push('+');
        } else if lower {
            let _ = write!(out, "%{b:02x}");
        } else {
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}

/// The percent-encodings in use: which characters stay (nothing, RFC 3986 unreserved, `-_.` only,
/// JavaScript's `encodeURIComponent` set), upper or lower hex, space as `%20` or `+`.
fn percent_forms(value: &str) -> Vec<String> {
    let mut out = Vec::new();
    for keep in ["", "-_.~", "-_.", "-_.!~*'()"] {
        for lower in [false, true] {
            for plus in [false, true] {
                out.push(percent_form(value, keep, lower, plus));
            }
        }
    }
    out
}

/// The text of a JSON string for `value`, without the surrounding quotes.
fn json_inner(value: &str) -> String {
    let quoted = serde_json::to_string(value).unwrap_or_default();
    quoted
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or("")
        .to_string()
}

fn word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Every quoting a driver is likely to apply to `base`.
fn quoted_forms(base: &str) -> Vec<String> {
    let bs = |s: &str| s.replace('\\', "\\\\");
    vec![
        base.to_string(),
        json_inner(base),
        base.replace('\'', "''"),
        base.replace('"', "\"\""),
        base.replace('\'', "''").replace('"', "\"\""),
        bs(base),
        base.replace('\'', "\\'"),
        base.replace('"', "\\\""),
        // Escaping only what one library escapes: libpq and the Python repr of a single-quoted
        // string (backslash and `'`), a double-quoted repr (backslash and `"`).
        bs(base).replace('\'', "\\'"),
        bs(base).replace('"', "\\\""),
        // Everything escaped at once, backslash style and SQL style.
        bs(base).replace('\'', "\\'").replace('"', "\\\""),
        bs(base).replace('\'', "''").replace('"', "\"\""),
    ]
}

impl Redactor {
    /// A redactor that knows no secret yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The redactor for one profile's connection facts; a value that is `None` or empty adds nothing.
    #[must_use]
    pub fn from_parts(password: Option<&str>, host: Option<&str>, user: Option<&str>) -> Self {
        let mut r = Self::new();
        if let Some(p) = password {
            r.add_password(p);
        }
        if let Some(h) = host {
            r.add_host(h);
        }
        if let Some(u) = user {
            r.add_user(u);
        }
        r
    }

    fn push(&mut self, text: String, rank: u8, kind: Kind) {
        if !text.is_empty()
            && !self
                .needles
                .iter()
                .any(|n| n.text == text && n.rank == rank)
        {
            self.needles.push(Needle { text, rank, kind });
        }
    }

    /// Adds a password, in every form a driver is likely to quote it.
    pub fn add_password(&mut self, value: &str) {
        for base in [value, value.trim()] {
            for form in quoted_forms(base).into_iter().chain(percent_forms(base)) {
                self.push(form, 0, Kind::Anywhere);
            }
        }
    }

    /// Adds a host (whole word); loopback names are not secrets.
    pub fn add_host(&mut self, value: &str) {
        self.add_word(value, 1, true);
    }

    /// Adds a user (whole word), of any length.
    pub fn add_user(&mut self, value: &str) {
        self.add_word(value, 2, false);
    }

    fn add_word(&mut self, value: &str, rank: u8, host: bool) {
        let v = value.trim();
        if host && OPEN_HOSTS.contains(&v) {
            return;
        }
        for form in quoted_forms(v).into_iter().chain(percent_forms(v)) {
            self.push(form.to_ascii_lowercase(), rank, Kind::Word);
        }
    }

    /// `text` with every known secret replaced.
    #[must_use]
    pub fn redact(&self, text: &str) -> String {
        if self.needles.is_empty() {
            return text.to_string();
        }
        let lower = text.to_ascii_lowercase();
        // One byte per text byte: the lowest rank that covers it.
        let mut marks = vec![NONE; text.len()];
        for needle in &self.needles {
            let haystack = if needle.kind == Kind::Word {
                &lower
            } else {
                text
            };
            let mut from = 0;
            while let Some(p) = haystack[from..].find(&needle.text) {
                let (s, e) = (from + p, from + p + needle.text.len());
                let whole = needle.kind == Kind::Anywhere || {
                    let before = text[..s].chars().next_back();
                    let after = text[e..].chars().next();
                    before.is_none_or(|c| !(word_char(c) || c == '.' || c == '-'))
                        && after.is_none_or(|c| !(word_char(c) || c == '-'))
                };
                if whole {
                    for m in &mut marks[s..e] {
                        *m = (*m).min(needle.rank);
                    }
                }
                from = s + haystack[s..].chars().next().map_or(1, char::len_utf8);
            }
        }
        let mut out = String::with_capacity(text.len());
        let mut i = 0;
        while i < text.len() {
            if marks[i] == NONE {
                let c = text[i..].chars().next().map_or(1, char::len_utf8);
                out.push_str(&text[i..i + c]);
                i += c;
            } else {
                let mut rank = NONE;
                while i < text.len() && marks[i] != NONE {
                    rank = rank.min(marks[i]);
                    i += 1;
                }
                out.push_str(LABELS[usize::from(rank)]);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn(password: &str, host: &str, user: &str) -> Redactor {
        Redactor::from_parts(Some(password), Some(host), Some(user))
    }

    /// Every output below must be free of the secret it was built from.
    fn clean(r: &Redactor, input: &str, secret: &str) -> String {
        let out = r.redact(input);
        assert!(
            !out.contains(secret),
            "{secret:?} left in {out:?} (from {input:?})"
        );
        out
    }

    #[test]
    fn a_numeric_password_and_user_are_redacted() {
        let r = conn("12345678", "db.internal", "424242");
        let out = clean(
            &r,
            "FATAL: password authentication failed for user \"424242\" password 12345678 at db.internal",
            "12345678",
        );
        assert!(!out.contains("424242"), "{out}");
        assert_eq!(
            out,
            "FATAL: password authentication failed for user \"<user>\" password <password> at <host>"
        );
    }

    #[test]
    fn a_numeric_password_inside_a_larger_token_is_redacted() {
        // No word boundary for the password: a digit run glued to other text still goes.
        let r = conn("90210", "h.example", "u_app");
        clean(&r, "ORA-90210 code user90210x 1:90210:2", "90210");
    }

    /// An independent percent-encoder for the tests (not the one under test): bytes in `keep` and
    /// ASCII alphanumerics stay; a space is `+` when `plus`; the rest `%XX`.
    fn enc(value: &str, keep: &str, lower: bool, plus: bool) -> String {
        value
            .bytes()
            .map(|b| {
                if b.is_ascii_alphanumeric() || keep.as_bytes().contains(&b) {
                    char::from(b).to_string()
                } else if b == b' ' && plus {
                    "+".to_string()
                } else if lower {
                    format!("%{b:02x}")
                } else {
                    format!("%{b:02X}")
                }
            })
            .collect()
    }

    #[test]
    fn a_password_inside_a_quoted_error_message_is_redacted() {
        let password = "pa'ss\"wo\\rd";
        let r = conn(password, "h.example", "u_app");
        // Each text quotes the password the way one driver does; the quoted form must be gone.
        let quoted = [
            r#"syntax error near 'pa'ss"wo\rd'"#.to_string(),
            r#"DETAIL: Key (password)=(pa'ss"wo\rd) is wrong"#.to_string(),
            r#"value 'pa''ss"wo\rd' rejected"#.to_string(),
            r#"value "pa'ss""wo\rd" rejected"#.to_string(),
            r#"value 'pa\'ss\"wo\\rd' rejected"#.to_string(),
            r#"json {"password":"pa'ss\"wo\\rd"}"#.to_string(),
            // libpq and the Python repr: backslash and the quote escaped, the other quote bare.
            r#"repr 'pa\'ss"wo\\rd' end"#.to_string(),
            r#"repr "pa'ss\"wo\\rd" end"#.to_string(),
        ];
        for text in &quoted {
            let out = r.redact(text);
            assert!(
                out.contains("<password>"),
                "not redacted: {out:?} (from {text:?})"
            );
            for leak in [
                "ss\"wo", "pa'ss", "pa''ss", "pa\\'ss", "wo\\rd", "wo\\\\rd", "'ss",
            ] {
                assert!(
                    !out.contains(leak),
                    "{leak:?} left in {out:?} (from {text:?})"
                );
            }
        }
    }

    #[test]
    fn a_percent_encoded_password_is_redacted_in_every_dialect() {
        let password = "p@ss w'rd/\u{e9}+%x";
        let r = conn(password, "h.example", "u_app");
        for keep in ["", "-_.~", "-_.", "-_.!~*'()"] {
            for lower in [false, true] {
                for plus in [false, true] {
                    let encoded = enc(password, keep, lower, plus);
                    let text =
                        format!("connection to postgres://u_app:{encoded}@h.example/db failed");
                    let out = r.redact(&text);
                    assert!(
                        !out.contains(&encoded) && !out.contains("w%27rd") && !out.contains("w'rd"),
                        "{encoded:?} left in {out:?}"
                    );
                    assert!(out.contains("<password>"), "{out}");
                }
            }
        }
        // A password with no special character is the same text in every dialect.
        let plain = conn("hunter22", "h.example", "u_app");
        assert!(!plain.redact("x hunter22 y").contains("hunter22"));
    }

    #[test]
    fn a_percent_encoded_host_and_user_are_redacted() {
        let r = conn("pw-Sup3rS3cret", "my host.example", "ana@corp m");
        for keep in ["", "-_.~", "-_.!~*'()"] {
            for lower in [false, true] {
                for plus in [false, true] {
                    let user = enc("ana@corp m", keep, lower, plus);
                    let host = enc("my host.example", keep, lower, plus);
                    let text = format!("postgres://{user}:x@{host}/db");
                    let out = r.redact(&text);
                    assert!(!out.contains(&user) && !out.contains(&host), "{out:?}");
                    assert!(out.contains("<user>") && out.contains("<host>"), "{out}");
                }
            }
        }
    }

    #[test]
    fn user_and_host_match_in_any_ascii_case_and_with_doubled_quotes() {
        let r = conn("pw-Sup3rS3cret", "DB.Example", "App\"User");
        let out = r.redact("role app\"\"user at db.EXAMPLE, APP\"USER, \"App\"\"User\"");
        assert!(!out.to_lowercase().contains("app"), "{out}");
        assert!(!out.to_lowercase().contains("example"), "{out}");
        // Case folding does not turn a longer word into a hit.
        let r = conn("pw-Sup3rS3cret", "db", "app");
        assert_eq!(r.redact("Database APPLE"), "Database APPLE");
        assert_eq!(r.redact("host DB user App"), "host <host> user <user>");
    }

    #[test]
    fn host_and_user_are_whole_words_as_in_python() {
        let r = conn("pw-Sup3rS3cret", "db", "app");
        assert_eq!(
            r.redact("database db.example app_x app"),
            "database <host>.example app_x <user>"
        );
        assert_eq!(r.redact("host db user app"), "host <host> user <user>");
    }

    #[test]
    fn a_one_character_user_is_redacted_not_skipped() {
        let r = Redactor::from_parts(None, None, Some("x"));
        assert_eq!(r.redact("user x failed"), "user <user> failed");
    }

    #[test]
    fn loopback_names_stay_readable_and_empty_values_add_nothing() {
        let r = Redactor::from_parts(Some(""), Some("localhost"), Some(""));
        assert_eq!(
            r.redact("connect to localhost failed"),
            "connect to localhost failed"
        );
        let r = Redactor::from_parts(None, Some("127.0.0.1"), None);
        assert_eq!(r.redact("127.0.0.1 refused"), "127.0.0.1 refused");
        // An empty password must not turn every position into a hit.
        assert_eq!(
            Redactor::from_parts(Some(""), None, None).redact("abc"),
            "abc"
        );
    }

    #[test]
    fn overlapping_secrets_leave_no_fragment() {
        // The user is a piece of the password: one merged hit, labelled as the password.
        let r = conn("abc123", "h.example", "abc");
        let out = clean(&r, "x abc123 y", "abc");
        assert_eq!(out, "x <password> y");
        // Partial overlap: user `abc` (ends before a dot), password `c.x`, text `abc.x`: one hit.
        let r = conn("c.x", "h.example", "abc");
        assert_eq!(r.redact("abc.x"), "<password>");
    }

    #[test]
    fn a_replacement_never_creates_a_new_match() {
        // The user is named like a label; the single pass keeps the output stable.
        let r = conn("pw-Sup3rS3cret", "h.example", "password");
        assert_eq!(
            r.redact("pw-Sup3rS3cret for password"),
            "<password> for <user>"
        );
    }

    #[test]
    fn non_ascii_values_and_neighbours_are_safe() {
        let r = conn("sênha-ç", "höst.example", "usuário");
        let out = clean(&r, "falha: sênha-ç em höst.example por usuário.", "sênha");
        assert_eq!(out, "falha: <password> em <host> por <user>.");
    }

    #[test]
    fn debug_never_shows_a_value() {
        let r = conn("pw-Sup3rS3cret", "h.example", "u_app");
        let shown = format!("{r:?}");
        assert!(
            !shown.contains("Sup3r") && !shown.contains("u_app"),
            "{shown}"
        );
    }

    #[test]
    fn a_huge_text_stays_linear_enough() {
        let r = conn("pw-Sup3rS3cret", "h.example", "u_app");
        let text = "x".repeat(2_000_000) + "pw-Sup3rS3cret";
        let out = r.redact(&text);
        assert!(out.ends_with("<password>"));
        // One-character secrets over a 16 MiB text: every byte is a hit; the work stays linear.
        let r = conn("a", "h.example", "u");
        let text = "a".repeat(16 * 1024 * 1024);
        let started = std::time::Instant::now();
        assert_eq!(r.redact(&text), "<password>");
        let mixed = "ab-".repeat(5 * 1024 * 1024);
        let out = r.redact(&mixed);
        // Each `a` (1 byte) becomes `<password>` (10): the output is bounded by 4x here.
        assert_eq!(out.len(), 5 * 1024 * 1024 * 12);
        assert!(
            started.elapsed() < std::time::Duration::from_secs(8),
            "{:?}",
            started.elapsed()
        );
    }
}
