//! POSIX `sh` segmentation: a port of the `awk` pipeline in the five `guard.sh` scripts.
//!
//! The shell guards see the command JSON-escaped (`\n` and `\"` are two characters), and their
//! quote blanking depends on that. So this module escapes the decoded command the same way and
//! runs the same steps: blank `\"..\"`, then `'..'`, turn the two characters `\n` into a line
//! break, then break on `; & | ( )` (and a backtick for the containers guard).
use regex::{NoExpand, Regex};
use std::fmt::Write;
use std::sync::OnceLock;

fn dq() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r#"\\"(?:[^\\]|\\[^"])*\\""#).expect("static pattern"))
}

fn sq() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"'[^']*'").expect("static pattern"))
}

fn engine() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r#"\\"\$\{?CONTAINER_ENGINE\}?\\""#).expect("static pattern"))
}

/// The command as the shell guards read it from the event JSON: the content of a JSON string.
#[must_use]
pub fn escape(cmd: &str) -> String {
    let mut out = String::with_capacity(cmd.len() + 8);
    for c in cmd.chars() {
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
            c => out.push(c),
        }
    }
    out
}

/// One simple command per element. `raw` is the escaped command; `containers` keeps
/// `"$CONTAINER_ENGINE"` as a word and also breaks on a backtick.
#[must_use]
pub fn segments(raw: &str, containers: bool) -> Vec<String> {
    let mut s = raw.to_string();
    if containers {
        s = engine()
            .replace_all(&s, NoExpand("$CONTAINER_ENGINE"))
            .into_owned();
    }
    s = dq().replace_all(&s, NoExpand("\"\"")).into_owned();
    s = sq().replace_all(&s, NoExpand("''")).into_owned();
    s = s.replace("\\n", "\n");
    let seps: &[char] = if containers {
        &[';', '&', '|', '(', ')', '`']
    } else {
        &[';', '&', '|', '(', ')']
    };
    s.chars()
        .map(|c| if seps.contains(&c) { '\n' } else { c })
        .collect::<String>()
        .split('\n')
        .map(str::to_string)
        .collect()
}

fn has_space(t: &str) -> bool {
    t.chars()
        .any(|c| matches!(c, ' ' | '\t' | '\n' | '\u{b}' | '\u{c}' | '\r'))
}

/// Quoted text with spaces in it is prose, not a path: blank it, pairing each quote with its own
/// closer; a quoted word without spaces keeps its text (the db guard's `words` scanner).
#[must_use]
#[allow(clippy::many_single_char_names)] // a transliteration of the awk scanner
pub fn blank_prose(raw: &str) -> String {
    let s: Vec<char> = raw.chars().collect();
    let n = s.len();
    let at = |i: usize| s.get(i).copied();
    let mut out = String::new();
    let mut i = 0;
    while i < n {
        let c = s[i];
        if c == '\\' && at(i + 1) == Some('"') {
            let mut j = i + 2;
            let mut t = String::new();
            while j < n && !(s[j] == '\\' && at(j + 1) == Some('"')) {
                if s[j] == '\\' {
                    t.push(s[j]);
                    if let Some(x) = at(j + 1) {
                        t.push(x);
                    }
                    j += 2;
                } else {
                    t.push(s[j]);
                    j += 1;
                }
            }
            out.push_str(if has_space(&t) { "\"\"" } else { &t });
            i = j + 2;
            continue;
        }
        if c == '\'' {
            let mut j = i + 1;
            let mut t = String::new();
            while j < n && s[j] != '\'' {
                t.push(s[j]);
                j += 1;
            }
            out.push_str(if has_space(&t) { "''" } else { &t });
            i = j + 1;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn segs(cmd: &str) -> Vec<String> {
        segments(&escape(cmd), false)
    }

    #[test]
    fn quoted_text_is_blanked() {
        assert_eq!(
            segs(r#"git commit -m "fix --no-verify""#),
            vec![r#"git commit -m """#]
        );
        assert_eq!(segs("echo 'a; b' && ls"), vec!["echo '' ", "", " ls"]);
    }

    #[test]
    fn newline_and_separators_split() {
        assert_eq!(segs("a\nb; c | d"), vec!["a", "b", " c ", " d"]);
        assert_eq!(segs("(x)"), vec!["", "x", ""]);
    }

    #[test]
    fn containers_keeps_the_engine_word_and_splits_on_backtick() {
        let raw = escape("\"$CONTAINER_ENGINE\" rmi `podman images -q`");
        let s = segments(&raw, true);
        assert_eq!(s[0], "$CONTAINER_ENGINE rmi ");
        assert_eq!(s[1], "podman images -q");
    }

    #[test]
    fn prose_with_spaces_is_blanked_and_words_are_kept() {
        let raw = escape(r#"git commit -m "a long note" && cat "oneword""#);
        let w = blank_prose(&raw);
        assert!(w.contains("-m \"\" "), "{w}");
        assert!(w.contains("cat oneword"), "{w}");
        assert_eq!(blank_prose("echo 'two words'"), "echo ''");
        assert_eq!(blank_prose("echo 'oneword'"), "echo oneword");
    }

    #[test]
    fn escape_matches_json_strings() {
        for s in ["a\"b", "x\\y", "l1\nl2\tt", "ctl\u{1}"] {
            let j = serde_json::to_string(s).unwrap();
            assert_eq!(format!("\"{}\"", escape(s)), j);
        }
    }
}
