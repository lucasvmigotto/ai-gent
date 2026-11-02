//! Small Python-compatible helpers the `repo` port needs: `str()` of JSON values, percent-encoding,
//! and `urlparse` pieces.
use crate::tokens::pyjson::{J, dumps, float_repr};

/// Python `str(value)` for a parsed JSON value.
#[must_use]
pub fn py_str(v: &J) -> String {
    match v {
        J::Str(s) => s.clone(),
        J::Int(i) => i.to_string(),
        J::Bool(b) => (if *b { "True" } else { "False" }).to_string(),
        J::Float(f) => float_repr(*f),
        J::Null => "None".to_string(),
        other => dumps(other, false, None),
    }
}

fn unreserved(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-' | b'~')
}

/// `urllib.parse.quote(s, safe="")`.
#[must_use]
pub fn quote(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if unreserved(b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// `urllib.parse.quote_plus(s)`.
#[must_use]
pub fn quote_plus(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if unreserved(b) {
            out.push(b as char);
        } else if b == b' ' {
            out.push('+');
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// `urllib.parse.urlencode(pairs)`.
#[must_use]
pub fn urlencode(pairs: &[(String, String)]) -> String {
    pairs
        .iter()
        .map(|(k, v)| format!("{}={}", quote_plus(k), quote_plus(v)))
        .collect::<Vec<_>>()
        .join("&")
}

/// `urllib.parse.unquote(s)` (UTF-8, invalid bytes replaced).
#[must_use]
pub fn unquote(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let hi = (b[i + 1] as char).to_digit(16);
            let lo = (b[i + 2] as char).to_digit(16);
            if let (Some(h), Some(l)) = (hi, lo) {
                out.push(u8::try_from(h * 16 + l).unwrap_or(b'?'));
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

/// The pieces of `urlparse(url)` the port uses.
#[derive(Debug, Default, Clone)]
pub struct Parsed {
    /// `scheme` (lowercase).
    pub scheme: String,
    /// `netloc`, as written (userinfo and port included).
    pub netloc: String,
    /// `hostname`, lowercase, without userinfo, port or brackets.
    pub hostname: String,
    /// `path`, without query or fragment.
    pub path: String,
}

/// A minimal `urlparse`.
#[must_use]
pub fn urlparse(url: &str) -> Parsed {
    let mut rest = url;
    let mut p = Parsed::default();
    if let Some(i) = rest.find(':') {
        let sch = &rest[..i];
        if !sch.is_empty()
            && sch.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
            && sch
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
        {
            p.scheme = sch.to_lowercase();
            rest = &rest[i + 1..];
        }
    }
    if let Some(r) = rest.strip_prefix("//") {
        let end = r.find(['/', '?', '#']).unwrap_or(r.len());
        p.netloc = r[..end].to_string();
        rest = &r[end..];
    }
    let end = rest.find(['?', '#']).unwrap_or(rest.len());
    p.path = rest[..end].to_string();
    let hostport = p.netloc.rsplit('@').next().unwrap_or("");
    let host = if let Some(r) = hostport.strip_prefix('[') {
        r.split(']').next().unwrap_or("")
    } else {
        hostport.split(':').next().unwrap_or("")
    };
    p.hostname = host.to_lowercase();
    p
}
