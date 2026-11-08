//! The shared redactor (spec 043 FR-008, T004): host, user and password of the profile in use are
//! replaced with `<host>`, `<user>` and `<password>` in every error and log line, for every connector.
//!
//! Stricter than the Python `redact()` on purpose, because it sits on a credential path and a driver
//! quotes values back in many shapes (each strictness is listed in `specs/043-dbrun-port/differences.md`):
//!
//! - the password is replaced wherever it occurs (no word boundary), in the forms a driver quotes it:
//!   as given, trimmed, JSON-escaped, with quotes doubled or backslash-escaped, and percent-encoded.
//!   A password that is all digits is just text here, so it is covered like any other (the review
//!   finding on `oracle_thin.rs` was a profile that gave the password as a number);
//! - host and user keep Python's whole-word rule (so `db` is not cut out of `database`), also in
//!   their percent-encoded form, and a value of any length counts: a one-character user is redacted,
//!   not skipped;
//! - one pass over the original text: overlapping or touching hits merge into one label, so a
//!   replacement can never expose a piece of another secret or create a new match.
//!
//! An empty value is never a needle; the loopback names are not secrets and stay readable.

/// The loopback names that are not secrets.
const OPEN_HOSTS: &[&str] = &["127.0.0.1", "localhost", "::1"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// Replaced wherever it occurs.
    Anywhere,
    /// Replaced only as a whole word (Python's rule).
    Word,
}

#[derive(Clone)]
struct Needle {
    text: String,
    label: &'static str,
    kind: Kind,
}

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

use std::fmt::Write as _;

fn percent_encode(value: &str) -> String {
    let mut out = String::new();
    for b in value.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(b));
        } else {
            let _ = write!(out, "%{b:02X}");
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

    fn push(&mut self, text: String, label: &'static str, kind: Kind) {
        if !text.is_empty()
            && !self
                .needles
                .iter()
                .any(|n| n.text == text && n.label == label)
        {
            self.needles.push(Needle { text, label, kind });
        }
    }

    /// Adds a password, in every form a driver is likely to quote it.
    pub fn add_password(&mut self, value: &str) {
        let trimmed = value.trim();
        let mut forms = vec![value.to_string(), trimmed.to_string(), json_inner(value)];
        for base in [value, trimmed] {
            forms.push(base.replace('\'', "''"));
            forms.push(base.replace('"', "\"\""));
            forms.push(base.replace('\\', "\\\\"));
            forms.push(base.replace('\'', "\\'"));
            forms.push(base.replace('"', "\\\""));
            forms.push(percent_encode(base));
            // Every special character escaped the same way at once, as one driver quotes it.
            forms.push(
                base.replace('\\', "\\\\")
                    .replace('\'', "\\'")
                    .replace('"', "\\\""),
            );
            forms.push(base.replace('\'', "''").replace('"', "\"\""));
            forms.push(
                base.replace('\\', "\\\\")
                    .replace('\'', "''")
                    .replace('"', "\"\""),
            );
        }
        for form in forms {
            self.push(form, "<password>", Kind::Anywhere);
        }
    }

    /// Adds a host (whole word); loopback names are not secrets.
    pub fn add_host(&mut self, value: &str) {
        self.add_word(value, "<host>", true);
    }

    /// Adds a user (whole word), of any length.
    pub fn add_user(&mut self, value: &str) {
        self.add_word(value, "<user>", false);
    }

    fn add_word(&mut self, value: &str, label: &'static str, host: bool) {
        let v = value.trim();
        if host && OPEN_HOSTS.contains(&v) {
            return;
        }
        for form in [v.to_string(), json_inner(v), percent_encode(v)] {
            self.push(form, label, Kind::Word);
        }
    }

    /// `text` with every known secret replaced.
    #[must_use]
    pub fn redact(&self, text: &str) -> String {
        // (start, end, rank): rank 0 password, 1 host, 2 user; the lowest rank labels a merged hit.
        let mut hits: Vec<(usize, usize, u8)> = Vec::new();
        for needle in &self.needles {
            let rank = match needle.label {
                "<password>" => 0,
                "<host>" => 1,
                _ => 2,
            };
            let mut from = 0;
            while let Some(p) = text[from..].find(&needle.text) {
                let (s, e) = (from + p, from + p + needle.text.len());
                let whole = needle.kind == Kind::Anywhere || {
                    let before = text[..s].chars().next_back();
                    let after = text[e..].chars().next();
                    before.is_none_or(|c| !(word_char(c) || c == '.' || c == '-'))
                        && after.is_none_or(|c| !(word_char(c) || c == '-'))
                };
                if whole {
                    hits.push((s, e, rank));
                }
                from = s + text[s..].chars().next().map_or(1, char::len_utf8);
            }
        }
        hits.sort_unstable();
        let labels = ["<password>", "<host>", "<user>"];
        let (mut out, mut at) = (String::with_capacity(text.len()), 0);
        let mut i = 0;
        while i < hits.len() {
            let (start, mut end, mut rank) = hits[i];
            i += 1;
            while i < hits.len() && hits[i].0 <= end {
                end = end.max(hits[i].1);
                rank = rank.min(hits[i].2);
                i += 1;
            }
            out.push_str(&text[at..start]);
            out.push_str(labels[usize::from(rank)]);
            at = end;
        }
        out.push_str(&text[at..]);
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

    #[test]
    fn a_password_inside_a_quoted_error_message_is_redacted() {
        let r = conn("pa'ss\"wo\\rd", "h.example", "u_app");
        for text in [
            r#"syntax error near 'pa'ss"wo\rd'"#,
            r#"DETAIL: Key (password)=(pa'ss"wo\rd) is wrong"#,
            r#"value 'pa''ss"wo\rd' rejected"#,
            r#"value "pa'ss""wo\rd" rejected"#,
            r#"value 'pa\'ss\"wo\\rd' rejected"#,
            "url postgres://u:pa%27ss%22wo%5Crd@h/db",
            r#"json {"password":"pa'ss\"wo\\rd"}"#,
        ] {
            let out = r.redact(text);
            for leak in [
                "pa'ss", "pa''ss", "pa\\'ss", "pa%27ss", "wo\\rd", "wo\\\\rd", "wo%5Crd",
            ] {
                assert!(
                    !out.contains(leak),
                    "{leak:?} left in {out:?} (from {text:?})"
                );
            }
        }
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
        let started = std::time::Instant::now();
        let out = r.redact(&text);
        assert!(out.ends_with("<password>"));
        assert!(started.elapsed() < std::time::Duration::from_secs(10));
    }
}
