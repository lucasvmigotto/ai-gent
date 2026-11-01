//! PowerShell and `cmd.exe` segmentation (spec 033, FR-004).
//!
//! The same job as `sh`: blank quoted strings by the shell's own quoting rules and break the line
//! into simple commands. Differences, from the shells' documentation:
//!
//! - PowerShell: `'..'` with `''` as an escaped quote, `".."` with a backtick escape and `""`; a
//!   backtick outside quotes escapes the next character (and `` `<newline> `` continues the line);
//!   separators are `; & | ( )` and newline.
//! - cmd: `".."` only; `^` escapes the next character outside quotes (`^&` is not a separator,
//!   `^<newline>` continues the line); separators are `& && || |`, newline, and (like the sh
//!   segmenter, so a grouped command is not hidden) `( )`.
//!
//! Windows paths are normalized for the rules: `\` becomes `/` and a trailing `.exe` on a word is
//! dropped, so `C:\pg\bin\psql.exe` is the client `psql` and `git -C C:\repo commit` names `C:/repo`.
use super::Shell;

/// The segments and the prose-blanked words of a command.
pub struct Scanned {
    /// One simple command per element, quoted strings blanked.
    pub segments: Vec<String>,
    /// The whole command with quoted prose blanked and quoted words kept.
    pub words: String,
}

fn has_space(t: &str) -> bool {
    t.chars().any(char::is_whitespace)
}

/// `\` to `/`, and a `.exe` at the end of a word dropped.
fn winpath(s: &str) -> String {
    let swapped = s.replace('\\', "/");
    let lower = swapped.to_ascii_lowercase();
    let mut out = String::with_capacity(swapped.len());
    let mut last = 0;
    let mut from = 0;
    while let Some(pos) = lower[from..].find(".exe") {
        let at = from + pos;
        let end = at + 4;
        let boundary = lower[end..].chars().next().is_none_or(char::is_whitespace);
        if boundary {
            out.push_str(&swapped[last..at]);
            last = end;
        }
        from = end;
    }
    out.push_str(&swapped[last..]);
    out
}

/// The string opened at `chars[start]`: its text (escapes kept) and the index after its closer.
fn read_quoted(chars: &[char], start: usize, ps: bool) -> (String, usize) {
    let c = chars[start];
    let mut text = String::new();
    let mut j = start + 1;
    while j < chars.len() {
        let q = chars[j];
        if ps && c == '"' && q == '`' {
            text.push(q);
            if let Some(n) = chars.get(j + 1) {
                text.push(*n);
            }
            j += 2;
        } else if q == c && ps && chars.get(j + 1) == Some(&c) {
            text.push(q);
            text.push(q);
            j += 2;
        } else if q == c {
            break;
        } else {
            text.push(q);
            j += 1;
        }
    }
    (text, j + 1)
}

/// Scan `cmd` as PowerShell or cmd. `keep_engine` keeps `"$CONTAINER_ENGINE"` readable.
///
/// # Panics
/// When `shell` is `Sh` (it has its own segmenter).
#[must_use]
pub fn scan(shell: Shell, cmd: &str, keep_engine: bool) -> Scanned {
    assert!(shell != Shell::Sh, "sh has its own segmenter");
    let ps = shell == Shell::PowerShell;
    let chars: Vec<char> = cmd.chars().collect();
    let mut segments: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut words = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        match c {
            '`' if ps => {
                match next {
                    Some('\n') => i += 1,
                    Some('\r') => {
                        i += if chars.get(i + 2) == Some(&'\n') {
                            2
                        } else {
                            1
                        }
                    }
                    Some(n) => {
                        cur.push(n);
                        words.push(n);
                        i += 1;
                    }
                    None => {}
                }
                i += 1;
            }
            '^' if !ps => {
                match next {
                    Some('\n') => i += 1,
                    Some('\r') => {
                        i += if chars.get(i + 2) == Some(&'\n') {
                            2
                        } else {
                            1
                        }
                    }
                    Some(n) => {
                        cur.push(n);
                        words.push(n);
                        i += 1;
                    }
                    None => {}
                }
                i += 1;
            }
            '"' | '\'' if c == '"' || ps => {
                let (text, after) = read_quoted(&chars, i, ps);
                let engine = keep_engine
                    && c == '"'
                    && (text == "$CONTAINER_ENGINE" || text == "${CONTAINER_ENGINE}");
                if engine {
                    cur.push_str(&text);
                } else {
                    cur.push(c);
                    cur.push(c);
                }
                if has_space(&text) {
                    words.push(c);
                    words.push(c);
                } else {
                    words.push_str(&text);
                }
                i = after;
            }
            '&' | '|' | '(' | ')' | '\n' | '\r' => {
                segments.push(std::mem::take(&mut cur));
                words.push(c);
                i += 1;
            }
            ';' if ps => {
                segments.push(std::mem::take(&mut cur));
                words.push(c);
                i += 1;
            }
            _ => {
                cur.push(c);
                words.push(c);
                i += 1;
            }
        }
    }
    segments.push(cur);
    Scanned {
        segments: segments.iter().map(|s| winpath(s)).collect(),
        words: winpath(&words),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ps(c: &str) -> Vec<String> {
        scan(Shell::PowerShell, c, false).segments
    }
    fn cmd(c: &str) -> Vec<String> {
        scan(Shell::Cmd, c, false).segments
    }

    #[test]
    fn powershell_doubled_single_quote_is_an_escaped_quote() {
        // 'it''s fine' is one string; the option after it stays visible.
        assert_eq!(
            ps("git commit -m 'it''s fine' --no-verify"),
            vec!["git commit -m '' --no-verify"]
        );
    }

    #[test]
    fn powershell_text_in_quotes_is_blanked() {
        assert_eq!(
            ps(r#"git commit -m "fix --no-verify""#),
            vec![r#"git commit -m """#]
        );
        assert_eq!(
            ps("git commit -m 'fix --no-verify'"),
            vec!["git commit -m ''"]
        );
    }

    #[test]
    fn powershell_backtick_escapes_in_double_quotes_and_outside() {
        assert_eq!(
            ps("echo \"a `\" ; b\""),
            vec![r#"echo "" "#.trim_end().to_string() + ""]
        );
        // an escaped semicolon is not a separator
        assert_eq!(ps("echo a`; git push"), vec!["echo a; git push"]);
        // a trailing backtick continues the line
        assert_eq!(ps("git `\npush"), vec!["git push"]);
    }

    #[test]
    fn powershell_separators() {
        assert_eq!(
            ps("git push; Write-Output hi"),
            vec!["git push", " Write-Output hi"]
        );
        assert_eq!(ps("a | b & c"), vec!["a ", " b ", " c"]);
    }

    #[test]
    fn cmd_separators_and_caret() {
        assert_eq!(cmd("git push & echo hi"), vec!["git push ", " echo hi"]);
        assert_eq!(cmd("a && b || c"), vec!["a ", "", " b ", "", " c"]);
        assert_eq!(cmd("git push ^& echo hi"), vec!["git push & echo hi"]);
        assert_eq!(cmd("git ^\npush"), vec!["git push"]);
    }

    #[test]
    fn cmd_has_only_double_quotes() {
        assert_eq!(
            cmd(r#"git commit -m "a & b" --no-verify"#),
            vec![r#"git commit -m "" --no-verify"#]
        );
        // a single quote is an ordinary character: the ampersand after it still separates
        assert_eq!(cmd("echo 'a & b'"), vec!["echo 'a ", " b'"]);
        // inside quotes the caret is literal
        assert_eq!(cmd(r#"echo "a ^ b" & ls"#), vec![r#"echo "" "#, " ls"]);
    }

    #[test]
    fn windows_paths_and_exe_are_normalized() {
        assert_eq!(cmd(r"C:\pg\bin\psql.exe -l"), vec!["C:/pg/bin/psql -l"]);
        assert_eq!(ps(r"git -C C:\repo commit"), vec!["git -C C:/repo commit"]);
        assert_eq!(cmd("psql.exe.bak x"), vec!["psql.exe.bak x"]);
    }

    #[test]
    fn words_keep_quoted_words_and_blank_prose() {
        let w = scan(Shell::Cmd, r#"type "oneword" "two words""#, false).words;
        assert_eq!(w, r#"type oneword """#);
    }
}
