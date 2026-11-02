//! Tree-sitter leaves as the token stream.

use tree_sitter::{Node, Parser};

/// Languages of the prototype.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    /// Rust, `free` join family.
    Rust,
    /// Java, `free` join family.
    Java,
    /// TypeScript, `js` join family.
    TypeScript,
    /// TSX, `js` join family with JSX.
    Tsx,
}

impl Lang {
    /// Language from a file extension.
    #[must_use]
    pub fn from_ext(ext: &str) -> Option<Lang> {
        Some(match ext {
            "rs" => Lang::Rust,
            "java" => Lang::Java,
            "ts" | "mts" | "cts" => Lang::TypeScript,
            "tsx" => Lang::Tsx,
            _ => return None,
        })
    }

    /// Whether newlines are significant (ASI): the `js` family.
    #[must_use]
    pub fn is_js(self) -> bool {
        matches!(self, Lang::TypeScript | Lang::Tsx)
    }

    fn language(self) -> tree_sitter::Language {
        match self {
            Lang::Rust => tree_sitter_rust::LANGUAGE.into(),
            Lang::Java => tree_sitter_java::LANGUAGE.into(),
            Lang::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Lang::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
        }
    }
}

/// Token class: what a gap next to it may do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Ordinary token.
    Code,
    /// String, char, template or JSX text: content is verbatim.
    Str,
    /// Block comment.
    Com,
    /// Line comment: the newline after it is mandatory.
    Line,
}

/// A significant token.
#[derive(Debug, Clone)]
pub struct Tok {
    /// Class.
    pub kind: Kind,
    /// Start byte.
    pub start: usize,
    /// End byte (exclusive).
    pub end: usize,
    /// Syntax-tree kind of the leaf.
    pub node: &'static str,
    /// Syntax-tree kind of its parent.
    pub parent: &'static str,
}

impl Tok {
    /// Token text.
    #[must_use]
    pub fn text<'a>(&self, src: &'a str) -> &'a str {
        &src[self.start..self.end]
    }
}

fn atomic(kind: &str) -> bool {
    kind.contains("string")
        || kind.contains("comment")
        || kind.contains("char")
        || kind.contains("regex")
        || kind.contains("text_block")
        || kind == "jsx_text"
        || kind == "shebang"
        || kind.contains("raw")
}

fn classify(node: Node, src: &str) -> Kind {
    let k = node.kind();
    if k.contains("comment") {
        let t = &src[node.start_byte()..node.end_byte()];
        return if t.starts_with("/*") {
            Kind::Com
        } else {
            Kind::Line
        };
    }
    if atomic(k) {
        Kind::Str
    } else {
        Kind::Code
    }
}

fn walk<'t>(node: Node<'t>, src: &str, out: &mut Vec<Tok>) {
    if node.is_missing() {
        return;
    }
    if node.child_count() == 0 || atomic(node.kind()) {
        if node.end_byte() > node.start_byte() {
            let kind = classify(node, src);
            let mut end = node.end_byte();
            if matches!(kind, Kind::Line | Kind::Com) {
                while end > node.start_byte() && src.as_bytes()[end - 1].is_ascii_whitespace() {
                    end -= 1;
                }
            }
            out.push(Tok {
                kind,
                start: node.start_byte(),
                end,
                node: node.kind(),
                parent: node.parent().map_or("", |p| p.kind()),
            });
        }
        return;
    }
    let mut c = node.walk();
    for ch in node.children(&mut c) {
        walk(ch, src, out);
    }
}

/// Parse `src`; error when the tree has an error node (the file is then left at level 1).
pub fn parse(src: &str, lang: Lang) -> Result<tree_sitter::Tree, String> {
    let mut p = Parser::new();
    p.set_language(&lang.language())
        .map_err(|e| e.to_string())?;
    let tree = p.parse(src, None).ok_or("parse failed")?;
    if tree.root_node().has_error() {
        return Err("parse: syntax error".into());
    }
    Ok(tree)
}

/// Tokens and the gaps around them: `gaps.len() == toks.len() + 1`, `gaps[i]` precedes `toks[i]`.
///
/// # Errors
/// The source does not parse, or a gap holds something that is not whitespace (a token the tree hides).
pub fn tokenize(src: &str, lang: Lang) -> Result<(Vec<Tok>, Vec<String>), String> {
    let tree = parse(src, lang)?;
    let mut toks = Vec::new();
    walk(tree.root_node(), src, &mut toks);
    let mut gaps = Vec::with_capacity(toks.len() + 1);
    let mut pos = 0;
    for t in &toks {
        if t.start < pos {
            return Err("overlapping tokens".into());
        }
        let g = &src[pos..t.start];
        if !g.chars().all(char::is_whitespace) {
            return Err(format!("non-whitespace between tokens: {g:?}"));
        }
        gaps.push(g.to_string());
        pos = t.end;
    }
    let tail = &src[pos..];
    if !tail.chars().all(char::is_whitespace) {
        return Err("non-whitespace after the last token".into());
    }
    gaps.push(tail.to_string());
    Ok((toks, gaps))
}
