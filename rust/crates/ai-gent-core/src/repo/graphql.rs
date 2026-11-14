//! Whether a GraphQL request body only reads: the JSON body's `query` document, lexed by the GraphQL
//! grammar, runs a `query` operation (the one `operationName` names when there are several). Anything
//! else — a mutation or subscription, a body that isn't a JSON object, a missing or non-string
//! `query`, a batched array, a repeated key, a token outside the grammar, several operations with no
//! name to pick one — is not a read. Mirrors `graphql_is_read` in `plugins/repo/scripts/reporun.py`.
use crate::tokens::pyjson::{J, parse_unique};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Query,
    Mutation,
    Subscription,
}

#[derive(Debug, PartialEq, Eq)]
enum Tok {
    Name(String),
    Punct(char),
    /// A string, number or `...`: never decides an operation's type.
    Other,
}

/// Whether the body of a POST to a GraphQL endpoint only reads (fail closed).
#[must_use]
pub fn is_read_body(body: Option<&[u8]>) -> bool {
    let Some(text) = body.and_then(|b| std::str::from_utf8(b).ok()) else {
        return false;
    };
    let Some(v @ J::Obj(_)) = parse_unique(text) else {
        return false;
    };
    let Some(J::Str(doc)) = v.get("query") else {
        return false;
    };
    let name = match v.get("operationName") {
        None | Some(J::Null) => None,
        Some(J::Str(s)) if !s.is_empty() => Some(s.as_str()),
        _ => return false,
    };
    let Some(ops) = operations(doc) else {
        return false;
    };
    if ops.len() > 1 && ops.iter().any(|(_, n)| n.is_none()) {
        return false; // an anonymous operation must stand alone
    }
    let chosen: Vec<&(Kind, Option<String>)> = match name {
        None => ops.iter().collect(),
        Some(n) => ops
            .iter()
            .filter(|(_, x)| x.as_deref() == Some(n))
            .collect(),
    };
    chosen.len() == 1 && chosen[0].0 == Kind::Query
}

/// The executable document's operations (type and name); `None` when it isn't one.
fn operations(doc: &str) -> Option<Vec<(Kind, Option<String>)>> {
    let t = tokens(doc)?;
    if t.is_empty() {
        return None;
    }
    let mut ops = Vec::new();
    let mut i = 0;
    while i < t.len() {
        match &t[i] {
            Tok::Punct('{') => {
                i = skip_set(&t, i)?;
                ops.push((Kind::Query, None));
            }
            Tok::Name(w) if w == "query" || w == "mutation" || w == "subscription" => {
                let kind = match w.as_str() {
                    "query" => Kind::Query,
                    "mutation" => Kind::Mutation,
                    _ => Kind::Subscription,
                };
                i += 1;
                let mut name = None;
                if let Some(Tok::Name(n)) = t.get(i) {
                    name = Some(n.clone());
                    i += 1;
                }
                i = skip_set(&t, to_set(&t, i)?)?;
                ops.push((kind, name));
            }
            Tok::Name(w) if w == "fragment" => {
                i = skip_set(&t, to_set(&t, i + 1)?)?;
            }
            _ => return None, // type-system definitions and anything else
        }
    }
    Some(ops)
}

/// The index of the selection set's `{`, past variable definitions and directives.
fn to_set(t: &[Tok], mut i: usize) -> Option<usize> {
    let mut depth = 0usize;
    loop {
        match t.get(i)? {
            Tok::Punct('(' | '[') => depth += 1,
            Tok::Punct(')' | ']') => depth = depth.checked_sub(1)?,
            Tok::Punct('{') if depth == 0 => return Some(i),
            Tok::Punct('}') if depth == 0 => return None,
            _ => {}
        }
        i += 1;
    }
}

/// The index after the selection set that opens at `i`.
fn skip_set(t: &[Tok], mut i: usize) -> Option<usize> {
    let mut depth = 0usize;
    loop {
        match t.get(i)? {
            Tok::Punct('{') => depth += 1,
            Tok::Punct('}') => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(i + 1);
                }
            }
            _ => {}
        }
        i += 1;
    }
}

fn is_name_start(c: char) -> bool {
    c == '_' || c.is_ascii_alphabetic()
}

/// The GraphQL lexical grammar, strictly: `None` on any character it doesn't define.
fn tokens(src: &str) -> Option<Vec<Tok>> {
    let s: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < s.len() {
        let c = s[i];
        match c {
            '\u{FEFF}' | ' ' | '\t' | ',' | '\n' | '\r' => i += 1,
            '#' => {
                i += 1;
                while i < s.len() && s[i] != '\n' && s[i] != '\r' {
                    // printable ASCII only: no character another lexer could read as a line end
                    if s[i] != '\t' && !(' '..='~').contains(&s[i]) {
                        return None;
                    }
                    i += 1;
                }
            }
            '!' | '$' | '&' | '(' | ')' | ':' | '=' | '@' | '[' | ']' | '{' | '|' | '}' => {
                out.push(Tok::Punct(c));
                i += 1;
            }
            '.' => {
                if s.get(i + 1) != Some(&'.') || s.get(i + 2) != Some(&'.') {
                    return None;
                }
                out.push(Tok::Other);
                i += 3;
            }
            '"' => {
                i = string(&s, i)?;
                out.push(Tok::Other);
            }
            '-' | '0'..='9' => {
                i = number(&s, i)?;
                out.push(Tok::Other);
            }
            c if is_name_start(c) => {
                let start = i;
                while i < s.len() && (s[i] == '_' || s[i].is_ascii_alphanumeric()) {
                    i += 1;
                }
                out.push(Tok::Name(s[start..i].iter().collect()));
            }
            _ => return None,
        }
    }
    Some(out)
}

/// The index after the number that starts at `i`.
fn number(s: &[char], mut i: usize) -> Option<usize> {
    let digits = |i: &mut usize| {
        let start = *i;
        while *i < s.len() && s[*i].is_ascii_digit() {
            *i += 1;
        }
        *i > start
    };
    if s[i] == '-' {
        i += 1;
    }
    let int_start = i;
    if !digits(&mut i) {
        return None;
    }
    if s[int_start] == '0' && i - int_start > 1 {
        return None;
    }
    if s.get(i) == Some(&'.') {
        i += 1;
        if !digits(&mut i) {
            return None;
        }
    }
    if matches!(s.get(i), Some('e' | 'E')) {
        i += 1;
        if matches!(s.get(i), Some('+' | '-')) {
            i += 1;
        }
        if !digits(&mut i) {
            return None;
        }
    }
    match s.get(i) {
        Some(&c) if c == '.' || is_name_start(c) => None,
        _ => Some(i),
    }
}

/// The index after the string (or block string) that starts at `i`.
fn string(s: &[char], mut i: usize) -> Option<usize> {
    if s.get(i + 1) == Some(&'"') && s.get(i + 2) == Some(&'"') {
        i += 3;
        loop {
            let c = *s.get(i)?;
            if c == '"' && s.get(i + 1) == Some(&'"') && s.get(i + 2) == Some(&'"') {
                return Some(i + 3);
            }
            if c == '\\' && s[i + 1..].starts_with(&['"', '"', '"']) {
                i += 4;
                continue;
            }
            if c < ' ' && !matches!(c, '\t' | '\n' | '\r') {
                return None;
            }
            i += 1;
        }
    }
    i += 1;
    loop {
        let c = *s.get(i)?;
        i += 1;
        match c {
            '"' => return Some(i),
            '\\' => {
                let e = *s.get(i)?;
                i += 1;
                match e {
                    '"' | '\\' | '/' | 'b' | 'f' | 'n' | 'r' | 't' => {}
                    'u' if s.get(i) == Some(&'{') => {
                        i += 1;
                        let start = i;
                        while i < s.len() && s[i].is_ascii_hexdigit() {
                            i += 1;
                        }
                        if i == start || s.get(i) != Some(&'}') {
                            return None;
                        }
                        i += 1;
                    }
                    'u' => {
                        if s.len() < i + 4 || !s[i..i + 4].iter().all(char::is_ascii_hexdigit) {
                            return None;
                        }
                        i += 4;
                    }
                    _ => return None,
                }
            }
            c if c < ' ' && c != '\t' => return None,
            _ => {}
        }
    }
}

#[cfg(test)]
#[allow(clippy::needless_raw_string_hashes)] // one hash style for every case, some need two
mod tests {
    use super::is_read_body;

    fn read(body: &str) -> bool {
        is_read_body(Some(body.as_bytes()))
    }

    #[test]
    fn queries_read() {
        assert!(read(r##"{"query":"query { viewer { login } }"}"##));
        assert!(read(r##"{"query":"{ viewer { login } }"}"##));
        assert!(read(
            r##"{"query":"query Q($n: Int = 3, $o: In = {a: [1, 2]}) @d(x: \"}\") { a(n: $n) { ...F } } fragment F on A { b }","variables":{"n":1}}"##
        ));
        // the word "mutation" inside a string, a comment or a field name no longer reads as one
        assert!(read(
            r##"{"query":"# mutation\nquery { search(query: \"mutation testing\") { mutation } }"}"##
        ));
        assert!(read(
            r##"{"query":"query A { a } mutation B { b }","operationName":"A"}"##
        ));
    }

    #[test]
    fn writes_and_unknowns_do_not_read() {
        for body in [
            r##"{"query":"mutation { x }"}"##,
            r##"{"query":"subscription { x }"}"##,
            r##"{"query":"subscription S { x }"}"##,
            r##"{"query":"query A { a } mutation B { b }","operationName":"B"}"##,
            r##"{"query":"query A { a } \u006dutation B { b }","operationName":"B"}"##,
            r##"{"query":"query A { a } mutation B { b }"}"##,
            r##"{"query":"query A { a } subscription B { b }","operationName":"C"}"##,
            r##"{"query":"query A { a } query A { b }","operationName":"A"}"##,
            r##"{"query":"{ a } query B { b }","operationName":"B"}"##,
            r##"{"query":"# query\n\u006dutation { x }"}"##,
            r##"{"query":"\u006dutation { x }"}"##,
            r##"{"query":"query { a }","operationName":""}"##,
            r##"{"query":"query { a }","operationName":3}"##,
            r##"{"query":"query { a }","query":"\u006dutation { x }"}"##,
            r##"[{"query":"query { a }"},{"query":"\u006dutation { x }"}]"##,
            r##"[{"query":"query { a }"}]"##,
            r##"{"query":"query { a }"} x"##,
            r##"{"variables":{}}"##,
            r##"{"query":null}"##,
            r##"{"query":["query { a }"]}"##,
            r##"{"query":""}"##,
            r##"{"query":"   "}"##,
            r##"{"query":"query { a"}"##,
            r##"{"query":"query { a }}"}"##,
            r##"{"query":"Query { a }"}"##,
            r##"{"query":"MUTATION { a }"}"##,
            r##"{"query":"type Query { a: Int }"}"##,
            r##"{"query":"query { a } \u00a0mutation { b }"}"##,
            r##"{"query":"# x\u2028\u006dutation { b }"}"##,
            r##"{"query":"query { a } \u200bmutation { b }"}"##,
            r##"{"query":"query { a(x: \"\\q\") }"}"##,
            r##"{"query":"query { a(x: 01) }"}"##,
            r##"{"query":"query { a(x: 1x) }"}"##,
            r##"{"query":"query { a(x: \"unterminated) }"}"##,
            r##"{"query":"query { a(x: \"\"\"open) }"}"##,
            "{\"query\":\"query { a }\", \"x\": NaN}",
            "\u{feff}{\"query\":\"query { a }\"}",
            "not json",
            "",
        ] {
            assert!(!read(body), "read: {body}");
        }
        assert!(!is_read_body(None));
        assert!(!is_read_body(Some(b"{\"query\":\"query { a \xff }\"}")));
    }

    #[test]
    fn api_is_write_uses_the_body() {
        use crate::repo::api::api_is_write;
        let q: &[u8] = br##"{"query":"query { viewer { login } }"}"##;
        let s: &[u8] = br##"{"query":"subscription { x }"}"##;
        for (plat, path) in [
            ("github", "/graphql"),
            ("gitlab", "/api/graphql"),
            ("gitlab", "/graphql"),
        ] {
            assert!(!api_is_write(plat, "POST", path, Some(q)));
            assert!(api_is_write(plat, "POST", path, Some(s)));
            assert!(api_is_write(plat, "POST", path, None));
            assert!(api_is_write(plat, "POST", path, Some(b"")));
        }
        assert!(api_is_write("bitbucket", "POST", "/graphql", Some(q)));
        assert!(api_is_write("github", "PATCH", "/graphql", Some(q)));
        // WIQL and Jira search keep reading whatever the body
        assert!(!api_is_write(
            "azure-devops",
            "POST",
            "/p/_apis/wit/wiql",
            None
        ));
        assert!(!api_is_write(
            "jira",
            "POST",
            "/rest/api/3/search/jql",
            Some(b"{}")
        ));
    }
}
