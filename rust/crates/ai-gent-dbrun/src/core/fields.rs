//! The connection-field validator (spec 043 FR-009, T004).
//!
//! An engine that builds a connect string or descriptor from profile fields (Oracle's
//! `(DESCRIPTION=...)` and Easy Connect) must call [`validate_conn`] first: a value holding `(`, `)`,
//! `=`, a quote, a backslash or whitespace could add or end a descriptor part (the review finding on
//! `oracle_thin.rs`). The rule is an allowlist, so anything not named here is refused:
//!
//! - `host`: ASCII letters, digits, `.`, `-`, `_`, up to 255 bytes; or an IPv6 address literal
//!   (no brackets: the descriptor builder adds them);
//! - `port`: 1 to 5 ASCII digits, value 1 to 65535;
//! - `service`, `database`, `sid`: letters and digits of any script, `_ - . $ #`, up to 255 bytes.
//!
//! An empty value is refused (a profile with a blank field must say `None`). The error names the
//! field and the rule and never carries the value, which may be a mistyped secret.
use crate::connector::ConnFacts;
use std::fmt;
use std::net::Ipv6Addr;

/// Longest accepted host, service, database or SID.
pub const MAX_FIELD_LEN: usize = 255;

/// A refused connection field. `Display` has the field name and a fixed reason, never the value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldError {
    /// `host`, `port`, `service`, `database` or `sid`.
    pub field: &'static str,
    /// A fixed explanation.
    pub reason: &'static str,
}

impl fmt::Display for FieldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "connection field `{}` {}", self.field, self.reason)
    }
}

impl std::error::Error for FieldError {}

const fn refuse(field: &'static str, reason: &'static str) -> FieldError {
    FieldError { field, reason }
}

/// Validates the `host` value.
///
/// # Errors
/// [`FieldError`] when it is empty, too long, or holds anything but a name or an IPv6 literal.
pub fn validate_host(value: &str) -> Result<(), FieldError> {
    const F: &str = "host";
    if value.is_empty() {
        return Err(refuse(F, "is empty"));
    }
    if value.len() > MAX_FIELD_LEN {
        return Err(refuse(F, "is too long"));
    }
    if value.contains(':') {
        return value
            .parse::<Ipv6Addr>()
            .map(|_| ())
            .map_err(|_| refuse(F, "holds characters a host name cannot have"));
    }
    if value
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
    {
        Ok(())
    } else {
        Err(refuse(F, "holds characters a host name cannot have"))
    }
}

/// Validates the `port` value.
///
/// # Errors
/// [`FieldError`] unless it is 1 to 5 ASCII digits naming a port from 1 to 65535.
pub fn validate_port(value: &str) -> Result<(), FieldError> {
    const F: &str = "port";
    if value.is_empty() || value.len() > 5 || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err(refuse(F, "is not a port number"));
    }
    match value.parse::<u32>() {
        Ok(1..=65535) => Ok(()),
        _ => Err(refuse(F, "is not a port number")),
    }
}

fn validate_name(field: &'static str, value: &str) -> Result<(), FieldError> {
    if value.is_empty() {
        return Err(refuse(field, "is empty"));
    }
    if value.len() > MAX_FIELD_LEN {
        return Err(refuse(field, "is too long"));
    }
    if value
        .chars()
        .all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.' | '$' | '#'))
    {
        Ok(())
    } else {
        Err(refuse(
            field,
            "holds characters that could change a connect string (parentheses, =, quotes, backslash, whitespace or others)",
        ))
    }
}

/// Validates a `service`, `database` or `sid` value; `field` names it in the error.
///
/// # Errors
/// [`FieldError`] when it is empty, too long, or holds a character outside the allowlist.
pub fn validate_service(value: &str) -> Result<(), FieldError> {
    validate_name("service", value)
}

/// Validates a `database` value.
///
/// # Errors
/// As [`validate_service`].
pub fn validate_database(value: &str) -> Result<(), FieldError> {
    validate_name("database", value)
}

/// Validates a `sid` value.
///
/// # Errors
/// As [`validate_service`].
pub fn validate_sid(value: &str) -> Result<(), FieldError> {
    validate_name("sid", value)
}

/// Validates every present field of `conn` that structures a connect string: `host`, `port`,
/// `service`, `database`, `sid` (a `None` field is simply absent). The first refused field is returned.
///
/// # Errors
/// [`FieldError`] naming the first field that fails.
pub fn validate_conn(conn: &ConnFacts) -> Result<(), FieldError> {
    if let Some(v) = &conn.host {
        validate_host(v)?;
    }
    if let Some(v) = &conn.port {
        validate_port(v)?;
    }
    if let Some(v) = &conn.service {
        validate_service(v)?;
    }
    if let Some(v) = &conn.database {
        validate_database(v)?;
    }
    if let Some(v) = &conn.sid {
        validate_sid(v)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    type Setter = fn(&mut ConnFacts, String);

    const PASSWORD: &str = "pw-Sup3rS3cret";

    fn with(f: impl FnOnce(&mut ConnFacts)) -> ConnFacts {
        let mut c = ConnFacts {
            host: Some("db.internal".into()),
            port: Some("1521".into()),
            service: Some("FREEPDB1".into()),
            password: Some(PASSWORD.into()),
            ..ConnFacts::default()
        };
        f(&mut c);
        c
    }

    #[test]
    fn a_normal_profile_passes() {
        validate_conn(&with(|_| {})).unwrap();
        validate_conn(&with(|c| c.host = Some("::1".into()))).unwrap();
        validate_conn(&with(|c| c.host = Some("2001:db8::7".into()))).unwrap();
        validate_conn(&with(|c| c.sid = Some("XE".into()))).unwrap();
        validate_conn(&with(|c| c.database = Some("shop_v2-test.db$#".into()))).unwrap();
        validate_conn(&with(|c| c.port = Some("00080".into()))).unwrap();
    }

    #[test]
    fn descriptor_characters_are_refused_in_every_field() {
        // Each hostile value must be refused in each of the fields, and the message never echoes it.
        let hostile = [
            "a(b",
            "a)b",
            "a=b",
            "a'b",
            "a\"b",
            "a\\b",
            "a b",
            "a\tb",
            "a\nb",
            "a\rb",
            "a\u{0}b",
            "x)(ADDRESS=(HOST=evil)",
            "x/y",
            "x;y",
            "x,y",
            "x?y=1",
            "x@y",
            "x%41",
            "x&y",
            "x`y",
            "\u{ff08}x",
            "a\u{a0}b",
            "a\u{2028}b",
        ];
        let setters: [(&str, Setter); 5] = [
            ("host", |c, v| c.host = Some(v)),
            ("port", |c, v| c.port = Some(v)),
            ("service", |c, v| c.service = Some(v)),
            ("database", |c, v| c.database = Some(v)),
            ("sid", |c, v| c.sid = Some(v)),
        ];
        for (name, set) in setters {
            for bad in hostile {
                let conn = with(|c| set(c, bad.to_string()));
                let err = validate_conn(&conn).expect_err(&format!("{name} accepted {bad:?}"));
                assert_eq!(err.field, name, "{bad:?}");
                let shown = err.to_string();
                assert!(shown.contains(name), "{shown}");
                assert!(
                    !shown.contains(bad) || bad.len() < 2,
                    "value echoed: {shown}"
                );
                assert!(!shown.contains(PASSWORD), "{shown}");
            }
        }
    }

    #[test]
    fn a_secret_typed_into_a_field_is_never_echoed() {
        let conn = with(|c| c.service = Some(format!("{PASSWORD} extra)")));
        let shown = validate_conn(&conn).unwrap_err().to_string();
        assert!(
            !shown.contains("Sup3r") && !shown.contains(PASSWORD),
            "{shown}"
        );
        let conn = with(|c| c.host = Some(format!("{PASSWORD}=x")));
        let shown = format!("{:?}", validate_conn(&conn).unwrap_err());
        assert!(!shown.contains("Sup3r"), "{shown}");
    }

    #[test]
    fn empty_oversized_and_bad_ports_are_refused() {
        for (f, c) in [
            ("host", with(|c| c.host = Some(String::new()))),
            ("service", with(|c| c.service = Some(String::new()))),
            ("host", with(|c| c.host = Some("h".repeat(256)))),
            ("sid", with(|c| c.sid = Some("S".repeat(256)))),
            ("port", with(|c| c.port = Some(String::new()))),
            ("port", with(|c| c.port = Some("0".into()))),
            ("port", with(|c| c.port = Some("65536".into()))),
            ("port", with(|c| c.port = Some("-1".into()))),
            ("port", with(|c| c.port = Some("+80".into()))),
            ("port", with(|c| c.port = Some("1e3".into()))),
            ("port", with(|c| c.port = Some("123456".into()))),
            ("port", with(|c| c.port = Some("\u{661}\u{662}".into()))),
            ("host", with(|c| c.host = Some("[::1]".into()))),
            ("host", with(|c| c.host = Some("fe80::1%eth0".into()))),
            ("host", with(|c| c.host = Some("not:ipv6".into()))),
            ("host", with(|c| c.host = Some("h\u{f6}st".into()))),
        ] {
            let err = validate_conn(&c).expect_err(f);
            assert_eq!(err.field, f, "{err}");
        }
        // The maximum length itself is accepted.
        validate_conn(&with(|c| c.sid = Some("S".repeat(255)))).unwrap();
        validate_conn(&with(|c| c.port = Some("65535".into()))).unwrap();
    }

    #[test]
    fn a_numeric_user_and_password_are_not_connection_fields() {
        // They are not validated here (they are driver arguments, never part of a descriptor); the
        // redactor covers them. A numeric service name is fine.
        let conn = with(|c| {
            c.user = Some("424242".into());
            c.password = Some("12345".into());
            c.service = Some("1234".into());
        });
        validate_conn(&conn).unwrap();
    }
}
