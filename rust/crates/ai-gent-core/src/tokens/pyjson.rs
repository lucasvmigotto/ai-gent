//! A small JSON value that keeps key order and prints like Python's `json.dumps`.
//!
//! The token tools write files that the Python tool wrote byte for byte (`", "` and `": "`
//! separators, `ensure_ascii`, `indent=1`), so `serde_json` (sorted keys, compact) cannot be used
//! for them.

use std::fmt::Write as _;

/// A JSON value with ordered objects.
#[derive(Debug, Clone, PartialEq)]
pub enum J {
    /// `null`
    Null,
    /// `true` / `false`
    Bool(bool),
    /// An integer.
    Int(i64),
    /// A float.
    Float(f64),
    /// A string.
    Str(String),
    /// An array.
    Arr(Vec<J>),
    /// An object, in insertion order.
    Obj(Vec<(String, J)>),
}

impl J {
    /// A string value.
    pub fn s(v: impl Into<String>) -> J {
        J::Str(v.into())
    }

    /// An optional string (`null` when absent).
    pub fn opt_s(v: Option<&str>) -> J {
        v.map_or(J::Null, J::s)
    }

    /// An optional integer (`null` when absent).
    pub fn opt_i(v: Option<i64>) -> J {
        v.map_or(J::Null, J::Int)
    }

    /// An array of strings.
    pub fn strs<S: AsRef<str>>(v: &[S]) -> J {
        J::Arr(v.iter().map(|x| J::s(x.as_ref())).collect())
    }

    /// The member `key` of an object.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&J> {
        match self {
            J::Obj(m) => m.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// Set a member: replaces in place (Python dict semantics) or appends.
    pub fn set(&mut self, key: &str, val: J) {
        if let J::Obj(m) = self {
            if let Some(slot) = m.iter_mut().find(|(k, _)| k == key) {
                slot.1 = val;
            } else {
                m.push((key.to_string(), val));
            }
        }
    }

    /// The string, if this is one.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        if let J::Str(s) = self { Some(s) } else { None }
    }

    /// The number as `f64` (integers and floats).
    #[must_use]
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            J::Int(i) => Some(*i as f64),
            J::Float(f) => Some(*f),
            _ => None,
        }
    }

    /// The number as an integer (floats truncate, like Python's `int()`).
    #[must_use]
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            J::Int(i) => Some(*i),
            J::Float(f) => Some(*f as i64),
            _ => None,
        }
    }

    /// Python truthiness.
    #[must_use]
    pub fn truthy(&self) -> bool {
        match self {
            J::Null => false,
            J::Bool(b) => *b,
            J::Int(i) => *i != 0,
            J::Float(f) => *f != 0.0,
            J::Str(s) => !s.is_empty(),
            J::Arr(a) => !a.is_empty(),
            J::Obj(o) => !o.is_empty(),
        }
    }
}

/// An object from key/value pairs.
#[must_use]
pub fn obj(pairs: Vec<(&str, J)>) -> J {
    J::Obj(pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

// ------------------------------------------------------------------ parse

struct P<'a> {
    b: &'a [u8],
    i: usize,
}

/// Parse one JSON document; `None` on any error (like a `JSONDecodeError`).
#[must_use]
pub fn parse(text: &str) -> Option<J> {
    let mut p = P {
        b: text.as_bytes(),
        i: 0,
    };
    p.ws();
    let v = p.value(0)?;
    p.ws();
    (p.i == p.b.len()).then_some(v)
}

impl P<'_> {
    fn ws(&mut self) {
        while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b'\t' | b'\n' | b'\r') {
            self.i += 1;
        }
    }

    fn eat(&mut self, lit: &str) -> bool {
        if self.b[self.i..].starts_with(lit.as_bytes()) {
            self.i += lit.len();
            true
        } else {
            false
        }
    }

    fn value(&mut self, depth: usize) -> Option<J> {
        if depth > 200 {
            return None;
        }
        match *self.b.get(self.i)? {
            b'{' => {
                self.i += 1;
                let mut v = J::Obj(Vec::new());
                self.ws();
                if self.eat("}") {
                    return Some(v);
                }
                loop {
                    self.ws();
                    let k = self.string()?;
                    self.ws();
                    if !self.eat(":") {
                        return None;
                    }
                    self.ws();
                    let x = self.value(depth + 1)?;
                    v.set(&k, x);
                    self.ws();
                    if self.eat(",") {
                        continue;
                    }
                    return self.eat("}").then_some(v);
                }
            }
            b'[' => {
                self.i += 1;
                let mut v = Vec::new();
                self.ws();
                if self.eat("]") {
                    return Some(J::Arr(v));
                }
                loop {
                    self.ws();
                    v.push(self.value(depth + 1)?);
                    self.ws();
                    if self.eat(",") {
                        continue;
                    }
                    return self.eat("]").then_some(J::Arr(v));
                }
            }
            b'"' => self.string().map(J::Str),
            b't' => self.eat("true").then_some(J::Bool(true)),
            b'f' => self.eat("false").then_some(J::Bool(false)),
            b'n' => self.eat("null").then_some(J::Null),
            _ => self.number(),
        }
    }

    fn number(&mut self) -> Option<J> {
        let start = self.i;
        let mut float = false;
        if self.b.get(self.i) == Some(&b'-') {
            self.i += 1;
        }
        let digits = |p: &mut Self| {
            let s = p.i;
            while p.i < p.b.len() && p.b[p.i].is_ascii_digit() {
                p.i += 1;
            }
            p.i > s
        };
        if !digits(self) {
            return None;
        }
        if self.b.get(self.i) == Some(&b'.') {
            float = true;
            self.i += 1;
            if !digits(self) {
                return None;
            }
        }
        if matches!(self.b.get(self.i), Some(b'e' | b'E')) {
            float = true;
            self.i += 1;
            if matches!(self.b.get(self.i), Some(b'+' | b'-')) {
                self.i += 1;
            }
            if !digits(self) {
                return None;
            }
        }
        let t = std::str::from_utf8(&self.b[start..self.i]).ok()?;
        if !float {
            if let Ok(i) = t.parse::<i64>() {
                return Some(J::Int(i));
            }
        }
        t.parse::<f64>().ok().map(J::Float)
    }

    fn hex4(&mut self) -> Option<u32> {
        let h = self.b.get(self.i..self.i + 4)?;
        self.i += 4;
        u32::from_str_radix(std::str::from_utf8(h).ok()?, 16).ok()
    }

    fn string(&mut self) -> Option<String> {
        if self.b.get(self.i) != Some(&b'"') {
            return None;
        }
        self.i += 1;
        let mut out: Vec<u8> = Vec::new();
        loop {
            let c = *self.b.get(self.i)?;
            self.i += 1;
            match c {
                b'"' => return String::from_utf8(out).ok(),
                b'\\' => {
                    let e = *self.b.get(self.i)?;
                    self.i += 1;
                    let ch = match e {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => {
                            let mut cp = self.hex4()?;
                            if (0xD800..0xDC00).contains(&cp)
                                && self.b[self.i..].starts_with(b"\\u")
                            {
                                let save = self.i;
                                self.i += 2;
                                match self.hex4() {
                                    Some(lo) if (0xDC00..0xE000).contains(&lo) => {
                                        cp = 0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00);
                                    }
                                    _ => self.i = save,
                                }
                            }
                            char::from_u32(cp).unwrap_or('\u{FFFD}')
                        }
                        _ => return None,
                    };
                    let mut buf = [0u8; 4];
                    out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                }
                0..=0x1f => return None,
                _ => out.push(c),
            }
        }
    }
}

// -------------------------------------------------------------------- dump

/// Python's `repr(float)`.
#[must_use]
pub fn float_repr(f: f64) -> String {
    if f.is_nan() {
        return "NaN".into();
    }
    if f.is_infinite() {
        return if f > 0.0 { "Infinity" } else { "-Infinity" }.into();
    }
    if f == 0.0 {
        return if f.is_sign_negative() { "-0.0" } else { "0.0" }.into();
    }
    let e = format!("{f:e}"); // shortest round-trip digits: d.ddde-x
    let (mant, exp) = e.split_once('e').unwrap_or((&e, "0"));
    let exp: i32 = exp.parse().unwrap_or(0);
    let neg = mant.starts_with('-');
    let digits: String = mant.chars().filter(char::is_ascii_digit).collect();
    let sign = if neg { "-" } else { "" };
    if (-4..16).contains(&exp) {
        if exp >= 0 {
            let e = exp as usize;
            let (int, frac) = if digits.len() > e + 1 {
                (digits[..=e].to_string(), digits[e + 1..].to_string())
            } else {
                (
                    format!("{digits}{}", "0".repeat(e + 1 - digits.len())),
                    "0".into(),
                )
            };
            format!("{sign}{int}.{frac}")
        } else {
            format!("{sign}0.{}{digits}", "0".repeat((-exp - 1) as usize))
        }
    } else {
        let m = if digits.len() > 1 {
            format!("{}.{}", &digits[..1], &digits[1..])
        } else {
            digits
        };
        format!(
            "{sign}{m}e{}{:02}",
            if exp < 0 { '-' } else { '+' },
            exp.abs()
        )
    }
}

fn dump_str(s: &str, ascii: bool, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c if ascii && (c as u32) > 0x7e => {
                let mut b = [0u16; 2];
                for u in c.encode_utf16(&mut b) {
                    let _ = write!(out, "\\u{u:04x}");
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn dump_into(v: &J, ascii: bool, indent: Option<usize>, level: usize, out: &mut String) {
    let nl = |out: &mut String, lvl: usize| {
        if let Some(n) = indent {
            out.push('\n');
            out.push_str(&" ".repeat(n * lvl));
        }
    };
    match v {
        J::Null => out.push_str("null"),
        J::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        J::Int(i) => out.push_str(&i.to_string()),
        J::Float(f) => out.push_str(&float_repr(*f)),
        J::Str(s) => dump_str(s, ascii, out),
        J::Arr(a) => {
            if a.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push('[');
            for (i, x) in a.iter().enumerate() {
                if i > 0 {
                    out.push_str(if indent.is_some() { "," } else { ", " });
                }
                nl(out, level + 1);
                dump_into(x, ascii, indent, level + 1, out);
            }
            nl(out, level);
            out.push(']');
        }
        J::Obj(m) => {
            if m.is_empty() {
                out.push_str("{}");
                return;
            }
            out.push('{');
            for (i, (k, x)) in m.iter().enumerate() {
                if i > 0 {
                    out.push_str(if indent.is_some() { "," } else { ", " });
                }
                nl(out, level + 1);
                dump_str(k, ascii, out);
                out.push_str(": ");
                dump_into(x, ascii, indent, level + 1, out);
            }
            nl(out, level);
            out.push('}');
        }
    }
}

/// `json.dumps(v, ensure_ascii=ascii, indent=indent)`.
#[must_use]
pub fn dumps(v: &J, ascii: bool, indent: Option<usize>) -> String {
    let mut out = String::new();
    dump_into(v, ascii, indent, 0, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dumps_like_python() {
        let v = parse(r#"{"b": [1, 2.5, "é", null], "a": {"x": true}}"#).unwrap();
        assert_eq!(
            dumps(&v, true, None),
            "{\"b\": [1, 2.5, \"\\u00e9\", null], \"a\": {\"x\": true}}"
        );
        assert_eq!(
            dumps(&v, false, None),
            r#"{"b": [1, 2.5, "é", null], "a": {"x": true}}"#
        );
        assert_eq!(
            dumps(&parse(r#"[{"a":1}]"#).unwrap(), false, Some(1)),
            "[\n {\n  \"a\": 1\n }\n]"
        );
        assert_eq!(dumps(&J::Arr(vec![]), false, Some(1)), "[]");
    }

    #[test]
    fn float_repr_matches_python() {
        for (f, s) in [
            (1.0, "1.0"),
            (0.5, "0.5"),
            (1.3, "1.3"),
            (123456789012345.0, "123456789012345.0"),
            (1e16, "1e+16"),
            (0.0001, "0.0001"),
            (0.00001, "1e-05"),
            (-2.25, "-2.25"),
        ] {
            assert_eq!(float_repr(f), s);
        }
    }

    #[test]
    fn surrogate_pairs_and_duplicates() {
        let v = parse(r#"{"k": "😀", "k": 2}"#).unwrap();
        assert_eq!(v, J::Obj(vec![("k".into(), J::Int(2))]));
        assert_eq!(dumps(&J::s("😀"), true, None), "\"\\ud83d\\ude00\"");
        assert!(parse("{bad").is_none());
        assert!(parse("").is_none());
    }
}
