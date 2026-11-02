//! Spec 037 D0 spike: a tree-sitter token stream, a renderer for levels 1-4 and a verifier, for rust,
//! java, typescript and tsx. This is a prototype to answer "does the Pygments-free design hold", not
//! the shipped `ai-gent compact` (it has no preprocessor lines, no Go, Kotlin or CSS families, no
//! outline and no CLI).
//!
//! Flow: [`tokenize`] turns the source into significant tokens (leaves of the syntax tree; strings,
//! comments and JSX text are atomic) and the whitespace gaps between them; [`render`] rewrites only the
//! gaps; [`verify`] re-parses the result and demands the same token texts and the same node-kind
//! sequence. [`compact`] steps down one flag at a time when a check fails, like `compact.py`.
#![forbid(unsafe_code)]

pub mod render;
pub mod tokenize;
pub mod verify;

pub use render::{render, Flags};
pub use tokenize::{tokenize, Kind, Lang, Tok};
pub use verify::verify;

/// What `compact` did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Info {
    /// Flags actually applied (empty = text returned as is).
    pub flags: Flags,
    /// Flags that were tried and failed verification, in the order they were dropped.
    pub fallback: Vec<&'static str>,
}

impl Info {
    /// Level reached (0..=4), the `level_of` of `compact.py`.
    #[must_use]
    pub fn level(&self) -> u8 {
        self.flags.level()
    }
}

/// Compact `src` to at most `level` (1..=4), verified; falls back flag by flag.
///
/// # Errors
/// The source does not parse (the caller keeps level 1 at most, `fallback[parse]`).
pub fn compact(src: &str, lang: Lang, level: u8) -> Result<(String, Info), String> {
    let (toks, gaps) = tokenize(src, lang)?;
    let mut flags = Flags::from_level(level.min(4));
    let mut fallback = Vec::new();
    loop {
        if flags.is_empty() {
            return Ok((src.to_string(), Info { flags, fallback }));
        }
        let out = render(src, &toks, &gaps, lang, flags);
        match verify(src, &out, lang) {
            Ok(()) => return Ok((out, Info { flags, fallback })),
            Err(why) => {
                let dropped = flags.drop_one();
                fallback.push(dropped);
                if std::env::var_os("COMPACT_DEBUG").is_some() {
                    eprintln!("fallback[{dropped}]: {why}");
                    if std::env::var("COMPACT_DEBUG").as_deref() == Ok("dump") {
                        eprintln!("{out}");
                    }
                }
            }
        }
    }
}
