//! Hand-ported look-around patterns of `langs.py`. The `regex` crate has no look-around or
//! back-references, and a heavier engine (`fancy-regex`, `onig`) is not worth its size for three
//! patterns, so each is a small scanner that finds exactly what the Python regex finds (the tests
//! hold cases computed with Python's `re`).

use crate::tokenize::{Kind, Tok};
use std::ops::Range;

/// `(?<![:/])//` (flag `re.M`): a `//` not preceded by `:` or `/` (scss and less `hazard`: the
/// lexer may miss `//` comments inside rules, so lines must stay where they are).
#[must_use]
pub fn has_line_comment_hazard(src: &str) -> bool {
    let b = src.as_bytes();
    (0..b.len().saturating_sub(1))
        .any(|i| b[i] == b'/' && b[i + 1] == b'/' && (i == 0 || !matches!(b[i - 1], b':' | b'/')))
}

/// `\$*("{3,})[\s\S]*?\1(?!")` over the whole source, like `finditer`: C# 11 raw string literals
/// (`protect`). Returns the matched byte ranges, left to right, not overlapping.
#[must_use]
pub fn csharp_raw_strings(src: &str) -> Vec<Range<usize>> {
    let b = src.as_bytes();
    let n = b.len();
    let mut out = Vec::new();
    let mut i = 0;
    while i < n {
        // `\$*` is greedy and may only give back `$`, which can never be followed by `"`
        let mut d = i;
        while d < n && b[d] == b'$' {
            d += 1;
        }
        let mut run = 0;
        while d + run < n && b[d + run] == b'"' {
            run += 1;
        }
        let mut hit = None;
        if run >= 3 {
            // `"{3,}` is greedy and backtracks one quote at a time; `[\s\S]*?` is lazy
            for g in (3..=run).rev() {
                let from = d + g;
                let mut j = from;
                while j + g <= n {
                    if b[j..j + g].iter().all(|&c| c == b'"') && (j + g == n || b[j + g] != b'"') {
                        hit = Some(i..j + g);
                        break;
                    }
                    j += 1;
                }
                if hit.is_some() {
                    break;
                }
            }
        }
        match hit {
            Some(r) => {
                i = r.end.max(i + 1);
                out.push(r);
            }
            None => i += 1,
        }
    }
    out
}

/// Every protected region must come out of the tokenizer as strings only (never split by a
/// punctuation or word token), or the lexer mis-read a raw string.
///
/// # Errors
/// A code token sits inside a raw string.
pub fn check_raw_strings(src: &str, toks: &[Tok]) -> Result<(), String> {
    for r in csharp_raw_strings(src) {
        if let Some(t) = toks
            .iter()
            .find(|t| t.start >= r.start && t.end <= r.end && t.kind != Kind::Str)
        {
            return Err(format!("raw string split: token {:?}", t.text(src)));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hazard_matches_python() {
        // computed with re.compile(r"(?<![:/])//", re.M).search
        for (s, want) in [
            ("a { b: c } // x", true),
            ("url(http://x)", false),
            ("///", true),
            ("a ///", true),
            ("http:///x", false),
            ("a /* c */ b", false),
            ("//", true),
            ("x//y", true),
            ("/", false),
            ("", false),
        ] {
            assert_eq!(has_line_comment_hazard(s), want, "{s:?}");
        }
    }

    #[test]
    fn raw_strings_match_python() {
        // computed with [m.span() for m in re.finditer(r'\$*("{3,})[\s\S]*?\1(?!")', s)]
        let cases: &[(&str, &[(usize, usize)])] = &[
            ("var s = \"\"\"a \"b\" c\"\"\";", &[(8, 21)]),
            ("var s = $$\"\"\"{x}\"\"\" + 1;", &[(8, 19)]),
            (
                "x = \"\"\"\"a\"\"\"\" + \"\"\"b\"\"\";",
                &[(4, 13), (16, 23)],
            ),
            ("\"\" \"\"", &[]),
            ("\"\"\"never closed", &[]),
            ("\"\"\"\"\"\"", &[(0, 6)]),
            ("\"\"\"a\"\"\"\"", &[(0, 8)]),
        ];
        for (s, want) in cases {
            let got: Vec<(usize, usize)> = csharp_raw_strings(s)
                .iter()
                .map(|r| (r.start, r.end))
                .collect();
            assert_eq!(&got, want, "{s:?}");
        }
    }
}
