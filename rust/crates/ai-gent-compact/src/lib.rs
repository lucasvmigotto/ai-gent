//! Token-lean source code reader of ai-gent (spec 037, D29): a compact view of a source file with
//! indentation, newlines and optional spaces removed (strings and comments kept), verified by
//! re-parsing, and an outline with the original line numbers.
//!
//! Flow: [`tokenize`] turns the source into the leaves of a tree-sitter tree and the whitespace gaps
//! between them; [`render`] rewrites only the gaps; [`verify`] re-parses the result and demands the
//! same tokens and the same statement-level tree shape (a lexical check for Go and Kotlin, whose
//! grammars reject some valid one-line code). [`compact_text`] steps down one flag at a time when a
//! check fails. Languages outside the D29 scope are returned as they are ("outline only, no
//! compaction").
#![forbid(unsafe_code)]
// The engine is a port of a rule-heavy Python tool: short names for tokens, one long rule table per
// family and flag structs are deliberate, as is indexing by token position.
#![allow(
    clippy::many_single_char_names,
    clippy::struct_excessive_bools,
    clippy::too_many_lines,
    clippy::collapsible_if,
    clippy::format_push_string,
    clippy::single_match_else,
    clippy::similar_names,
    clippy::needless_range_loop,
    clippy::cast_precision_loss
)]

pub mod langs;
pub mod lookaround;
pub mod render;
pub mod tokenize;
pub mod verify;
pub mod walk;

pub use render::{Flags, Opts, render};
pub use tokenize::{Family, Kind, Lang, Tok, tokenize};
pub use verify::{verify, verify_how};

/// The crate version, as `ai-gent-compact --version` prints it.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// What to do with a source.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    /// Requested level 0..=4 (capped by the language and the source).
    pub level: u8,
    /// Also drop comments (directive comments such as `//go:build` stay).
    pub strip_comments: bool,
    /// Keep line breaks at brace depth `<= n`.
    pub outline: Option<usize>,
    /// Soft-wrap length (0 = never).
    pub wrap: usize,
    /// Prefix lines with their original number.
    pub numbers: bool,
    /// Run the safety check (`false` is `--no-verify`).
    pub check: bool,
    /// Keep one blank line where the source had blank lines.
    pub paragraphs: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            level: 3,
            strip_comments: false,
            outline: None,
            wrap: 1500,
            numbers: false,
            check: true,
            paragraphs: false,
        }
    }
}

/// What `compact_text` did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Info {
    /// Language actually used (after refining `.h`).
    pub lang: String,
    /// Flags actually applied (empty = text returned as is).
    pub flags: Flags,
    /// `lex+ast`, `lex` or `-` (not verified).
    pub verified: &'static str,
    /// Flags that were tried and failed verification, as `flag:check`.
    pub fallback: Vec<String>,
    /// Why the text was kept as it is.
    pub error: Option<String>,
    /// A note for the summary, such as "outline only, no compaction".
    pub note: Option<String>,
}

impl Info {
    /// The summary of a file whose language is unknown (returned as it is).
    #[must_use]
    pub fn unknown() -> Info {
        Info {
            lang: "?".into(),
            flags: Flags::from_level(0),
            verified: "-",
            fallback: Vec::new(),
            error: None,
            note: None,
        }
    }

    /// Level reached (0..=4).
    #[must_use]
    pub fn level(&self) -> u8 {
        self.flags.level()
    }
}

/// `.h` files that are C++: the `CPP_HINT` of `langs.py`.
fn looks_cpp(code: &str) -> bool {
    code.contains("std::")
        || code.lines().any(|l| {
            let t = l.trim_start();
            t.starts_with("template<")
                || t.starts_with("template <")
                || t.starts_with("using namespace")
                || t.strip_prefix("namespace").is_some_and(|r| {
                    r.starts_with(char::is_whitespace)
                        && r.trim_start()
                            .starts_with(|c: char| c.is_alphanumeric() || c == '_')
                })
                || t.strip_prefix("class").is_some_and(|r| {
                    r.starts_with(char::is_whitespace) && r.contains('{') && !r.contains(';')
                })
        })
}

fn looks_objc(code: &str) -> bool {
    code.lines().any(|l| {
        let t = l.trim_start();
        ["@interface", "@protocol", "@class", "@implementation"]
            .iter()
            .any(|p| t.starts_with(p))
            || t.starts_with("#import ")
    })
}

fn normalize(code: &str) -> String {
    let mut s = code
        .trim_start_matches('\u{feff}')
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    if !s.is_empty() && !s.ends_with('\n') {
        s.push('\n'); // a final `// comment` is only a comment when a newline ends it
    }
    s
}

/// Compact `code` written in language `key` (a key of [`langs::LANGS`]).
#[must_use]
pub fn compact_text(code: &str, key: &str, o: &Options) -> (String, Info) {
    let code = normalize(code);
    let mut info = Info {
        lang: key.to_string(),
        flags: Flags::from_level(0),
        verified: "-",
        fallback: Vec::new(),
        error: None,
        note: None,
    };
    let mut key = key;
    if key == "c" && !looks_objc(&code) && looks_cpp(&code) {
        key = "cpp";
        info.lang = "cpp".into();
    }
    let Some(lang) = langs::tokenizer(key).filter(|_| !(key == "c" && looks_objc(&code))) else {
        info.note = Some("outline only, no compaction".into());
        let text = if o.numbers {
            render(
                &code,
                &[],
                &[],
                Lang::Rust,
                info.flags,
                Opts {
                    numbers: true,
                    ..Opts::default()
                },
            )
        } else {
            code
        };
        return (text, info);
    };
    let mut flags = Flags::from_level(o.level.min(lang.max_level(&code)));
    if o.paragraphs {
        flags = flags.with_paragraphs();
    }
    if flags.is_empty() && !o.strip_comments {
        return (code, info);
    }
    let (toks, gaps) = match tokenize::tokenize_view(&code, lang, o.strip_comments) {
        Ok(v) => v,
        Err(e) => {
            info.error = Some(e);
            return (code, info);
        }
    };
    let ropts = Opts {
        outline: o.outline,
        wrap: o.wrap,
        numbers: false,
        // ASI newlines become `;` only when the grammar vouches for the file
        no_semi: lang.is_js() && tokenize::parse(&code, lang).is_err(),
    };
    loop {
        let text = render(&code, &toks, &gaps, lang, flags, ropts);
        if !o.check {
            info.flags = flags;
            break;
        }
        match verify_how(&code, &text, lang, o.strip_comments) {
            Ok(how) => {
                info.flags = flags;
                info.verified = how;
                break;
            }
            Err(why) => {
                let how = if why.starts_with("syntax tree") {
                    "ast"
                } else {
                    "lex"
                };
                let dropped = flags.drop_one();
                if dropped == "none" {
                    return (code, info);
                }
                info.fallback.push(format!("{dropped}:{how}"));
                if std::env::var_os("COMPACT_DEBUG").is_some() {
                    eprintln!("fallback[{dropped}]: {why}");
                }
                if flags.is_empty() && !o.strip_comments {
                    return (code, info);
                }
            }
        }
    }
    let text = render(
        &code,
        &toks,
        &gaps,
        lang,
        info.flags,
        Opts {
            numbers: o.numbers,
            ..ropts
        },
    );
    (text, info)
}
