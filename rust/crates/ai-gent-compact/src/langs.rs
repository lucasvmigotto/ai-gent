//! The language table (generated from `langs.py`, `scripts/gen-compact-langs.py`) and what this
//! crate does with each language: D29 compacts java, csharp, c, cpp, go, rust, javascript, kotlin,
//! css, typescript and tsx (and scss to level 2); every other language is outline only.

use crate::tokenize::Lang;

/// One row of the generated table. Field meanings are in the header of `data/compact-langs.rs`.
#[derive(Debug)]
#[allow(dead_code, missing_docs)]
pub struct LangSpec {
    pub key: &'static str,
    pub exts: &'static [&'static str],
    pub names: &'static [&'static str],
    pub allowed: u8,
    pub join: Option<&'static str>,
    pub preproc: bool,
    pub ts: Option<&'static str>,
    pub ts_verify: Option<&'static str>,
    pub protect: Option<&'static str>,
    pub hazard: Option<&'static str>,
    pub hazard_allowed: u8,
    pub indent_sensitive: bool,
    pub needs_lookaround: bool,
}

/// Every language `compact.py` knows, sorted by key.
pub static LANGS: &[LangSpec] = include!("data/compact-langs.rs");

/// The row of a language key.
#[must_use]
pub fn find(key: &str) -> Option<&'static LangSpec> {
    LANGS.iter().find(|l| l.key == key)
}

/// The tokenizer of a language key: `None` means "outline only, no compaction".
#[must_use]
pub fn tokenizer(key: &str) -> Option<Lang> {
    Some(match key {
        "rust" => Lang::Rust,
        "java" => Lang::Java,
        "c" => Lang::C,
        "cpp" => Lang::Cpp,
        "csharp" => Lang::CSharp,
        "go" => Lang::Go,
        "javascript" | "jsx" => Lang::JavaScript,
        "typescript" => Lang::TypeScript,
        "tsx" => Lang::Tsx,
        "kotlin" => Lang::Kotlin,
        "css" => Lang::Css,
        "scss" => Lang::Scss,
        _ => return None,
    })
}

/// The language key of a path (by exact file name, then `Dockerfile*`, then extension).
#[must_use]
pub fn detect(path: &str) -> Option<&'static str> {
    let base = path.rsplit(['/', '\\']).next().unwrap_or(path);
    if let Some(l) = LANGS.iter().find(|l| l.names.contains(&base)) {
        return Some(l.key);
    }
    if base.starts_with("Dockerfile") || base.ends_with(".dockerfile") {
        return Some("dockerfile");
    }
    let ext = base.rfind('.').map(|i| &base[i..])?;
    let by = |e: &str| LANGS.iter().find(|l| l.exts.contains(&e)).map(|l| l.key);
    by(ext).or_else(|| by(&ext.to_lowercase()))
}

/// Resolve a `-l` value: a language key or an extension (`.rs`, `rs`).
#[must_use]
pub fn resolve(name: &str) -> Option<&'static str> {
    let n = name.to_lowercase();
    find(&n)
        .map(|l| l.key)
        .or_else(|| detect(&format!("x.{}", n.trim_start_matches('.'))))
}

fn level_of_bits(bits: u8) -> u8 {
    [(15, 4), (7, 3), (3, 2), (1, 1)]
        .iter()
        .find(|(m, _)| bits & m == *m)
        .map_or(0, |&(_, l)| l)
}

/// The `--list-languages` text: the lines of `compact.py` for the compacted languages, and for the
/// others level 0 and the note "outline only, no compaction"; each line ends with the tokenizer.
#[must_use]
pub fn list_languages() -> String {
    const NAMES: [&str; 5] = ["as-is", "blank", "lines", "flat", "dense"];
    let mut out = String::new();
    for l in LANGS {
        let supported = tokenizer(l.key).is_some();
        let max = if supported {
            level_of_bits(l.allowed)
        } else {
            0
        };
        let mut extra = String::new();
        if l.indent_sensitive {
            extra.push_str("  indent-sensitive");
        }
        let all: Vec<&str> = l.exts.iter().chain(l.names.iter()).copied().collect();
        let tok = match (supported, l.ts) {
            (true, Some(t)) => format!("tree-sitter:{t}"),
            (true, None) => "tree-sitter".to_string(),
            _ => "none (outline only, no compaction)".to_string(),
        };
        out.push_str(&format!(
            "{:<12} max L{} ({}){}  {} | {}\n",
            l.key,
            max,
            NAMES[usize::from(max)],
            extra,
            all.join(" "),
            tok
        ));
    }
    out
}
