//! Tree-sitter leaves as the token stream.

use tree_sitter::{Node, Parser};

/// Languages of the spike (the D29 scope; `Scss` is here for the hazard port).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    /// Rust, `free` join family.
    Rust,
    /// Java, `free` join family.
    Java,
    /// C, `free` join family, preprocessor lines.
    C,
    /// C++, `free` join family, preprocessor lines.
    Cpp,
    /// C#, `free` join family, preprocessor lines, raw-string protection.
    CSharp,
    /// Go, `go` join family (semicolons re-derived).
    Go,
    /// JavaScript and JSX, `js` join family.
    JavaScript,
    /// TypeScript, `js` join family.
    TypeScript,
    /// TSX, `js` join family with JSX.
    Tsx,
    /// Kotlin, `kotlin` join family.
    Kotlin,
    /// CSS, `css` join family.
    Css,
    /// SCSS, `css` join family; `//` makes it level 2 (hazard).
    Scss,
}

/// Join family (`join` of `langs.py`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    /// Newlines are plain whitespace.
    Free,
    /// Re-insert the semicolons the Go lexer would insert.
    Go,
    /// Join only where ASI cannot fire.
    Js,
    /// Join only where the grammar allows a newline.
    Kotlin,
    /// Whitespace is a descendant combinator.
    Css,
}

impl Lang {
    /// Language from a file extension.
    #[must_use]
    pub fn from_ext(ext: &str) -> Option<Lang> {
        Some(match ext {
            "rs" => Lang::Rust,
            "java" => Lang::Java,
            "c" | "h" => Lang::C,
            "cc" | "cpp" | "cxx" | "hpp" | "hh" | "hxx" => Lang::Cpp,
            "cs" => Lang::CSharp,
            "go" => Lang::Go,
            "js" | "mjs" | "cjs" | "jsx" => Lang::JavaScript,
            "ts" | "mts" | "cts" => Lang::TypeScript,
            "tsx" => Lang::Tsx,
            "kt" | "kts" => Lang::Kotlin,
            "css" => Lang::Css,
            "scss" => Lang::Scss,
            _ => return None,
        })
    }

    /// The join family.
    #[must_use]
    pub fn family(self) -> Family {
        match self {
            Lang::Go => Family::Go,
            Lang::JavaScript | Lang::TypeScript | Lang::Tsx => Family::Js,
            Lang::Kotlin => Family::Kotlin,
            Lang::Css | Lang::Scss => Family::Css,
            _ => Family::Free,
        }
    }

    /// Whether newlines are significant (ASI): the `js` family.
    #[must_use]
    pub fn is_js(self) -> bool {
        self.family() == Family::Js
    }

    /// Does the language have JSX?
    #[must_use]
    pub fn has_jsx(self) -> bool {
        matches!(self, Lang::Tsx | Lang::JavaScript)
    }

    /// Preprocessor lines are kept verbatim.
    #[must_use]
    pub fn preproc(self) -> bool {
        matches!(self, Lang::C | Lang::Cpp | Lang::CSharp)
    }

    /// Grammars that reject some valid one-line code: parse errors are noise, newlines are
    /// checked lexically (`ts_verify=False` in `langs.py`).
    #[must_use]
    pub fn lexical_only(self) -> bool {
        matches!(self, Lang::Go | Lang::Kotlin)
    }

    /// Highest level the source allows (the `hazard` of `langs.py`).
    #[must_use]
    pub fn max_level(self, src: &str) -> u8 {
        if self == Lang::Scss && crate::lookaround::has_line_comment_hazard(src) {
            2
        } else {
            4
        }
    }

    fn language(self) -> tree_sitter::Language {
        match self {
            Lang::Rust => tree_sitter_rust::LANGUAGE.into(),
            Lang::Java => tree_sitter_java::LANGUAGE.into(),
            Lang::C => tree_sitter_c::LANGUAGE.into(),
            Lang::Cpp => tree_sitter_cpp::LANGUAGE.into(),
            Lang::CSharp => tree_sitter_c_sharp::LANGUAGE.into(),
            Lang::Go => tree_sitter_go::LANGUAGE.into(),
            Lang::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            Lang::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Lang::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
            Lang::Kotlin => tree_sitter_kotlin_ng::LANGUAGE.into(),
            Lang::Css => tree_sitter_css::LANGUAGE.into(),
            Lang::Scss => tree_sitter_scss::language(),
        }
    }
}

/// Token class: what a gap next to it may do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Ordinary token.
    Code,
    /// String, char, template or JSX text word: content is verbatim.
    Str,
    /// Block comment.
    Com,
    /// Line comment: the newline after it is mandatory.
    Line,
    /// Preprocessor directive, verbatim to the end of its line.
    Pre,
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
    /// The leaf is a named node (identifiers, numbers, ...), not a keyword or punctuation.
    pub named: bool,
    /// A word of JSX text (where a `;` would become visible text).
    pub jsx_text: bool,
    /// Inside a JSX tag (names, attributes), not the angle brackets or an attribute expression.
    pub jsx_tag: bool,
    /// The gap before this token lies between JSX children.
    pub jsx_gap: bool,
}

impl Tok {
    /// Token text.
    #[must_use]
    pub fn text<'a>(&self, src: &'a str) -> &'a str {
        &src[self.start..self.end]
    }
}

fn atomic(kind: &str) -> bool {
    (kind.contains("string") && kind != "concatenated_string")
        || kind.contains("comment")
        || kind.contains("char")
        || kind.contains("rune")
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

const JSX_TAGS: &[&str] = &[
    "jsx_opening_element",
    "jsx_self_closing_element",
    "jsx_closing_element",
];

fn in_jsx_tag(leaf: Node, text: &str) -> bool {
    if matches!(text, "<" | ">" | "</" | "/>" | "/") {
        return false;
    }
    let mut n = leaf.parent();
    while let Some(p) = n {
        if p.kind() == "jsx_expression" {
            return false;
        }
        if JSX_TAGS.contains(&p.kind()) {
            return true;
        }
        n = p.parent();
    }
    false
}

/// End (exclusive, before the newline) of the preprocessor directive that starts at `start`: a
/// backslash continues the line and a `/* */` opened on it may span lines (`_directive_end`).
fn directive_end(src: &str, start: usize) -> usize {
    let b = src.as_bytes();
    let mut i = start;
    loop {
        let Some(off) = src[i..].find('\n') else {
            return src.len();
        };
        let j = i + off;
        if j > 0 && b[j - 1] == b'\\' {
            i = j + 1;
            continue;
        }
        let seg = &src[start..j];
        if seg.matches("/*").count() > seg.matches("*/").count() {
            if let Some(k) = src[j..].find("*/") {
                i = j + k + 2;
                continue;
            }
        }
        return j;
    }
}

fn at_line_start(src: &str, pos: usize) -> bool {
    src[..pos]
        .rsplit('\n')
        .next()
        .is_some_and(|l| l.chars().all(char::is_whitespace))
}

struct Walker<'a> {
    src: &'a str,
    lang: Lang,
    out: Vec<Tok>,
    skip_to: usize,
}

impl Walker<'_> {
    fn push_leaf(&mut self, node: Node) {
        let (s, e) = (node.start_byte(), node.end_byte());
        let text = &self.src[s..e];
        if text.trim().is_empty() {
            return;
        }
        if self.lang.preproc() && text.starts_with('#') && at_line_start(self.src, s) {
            let end = directive_end(self.src, s);
            let end = s + self.src[s..end].trim_end().len();
            self.out.push(Tok {
                kind: Kind::Pre,
                start: s,
                end,
                node: "preproc",
                parent: "preproc",
                named: false,
                jsx_text: false,
                jsx_tag: false,
                jsx_gap: false,
            });
            self.skip_to = end;
            return;
        }
        let kind = classify(node, self.src);
        let mut end = e;
        if matches!(kind, Kind::Line | Kind::Com) {
            while end > s && self.src.as_bytes()[end - 1].is_ascii_whitespace() {
                end -= 1;
            }
        }
        let parent = node.parent().map_or("", |p| p.kind());
        if node.kind() == "jsx_text" {
            // words: whitespace between them is the renderer's business (JSX trims lines)
            let mut pos = s;
            for w in text.split_whitespace() {
                let ws = pos + self.src[pos..].find(w).unwrap_or(0);
                self.out.push(Tok {
                    kind: Kind::Str,
                    start: ws,
                    end: ws + w.len(),
                    node: "jsx_text",
                    parent,
                    named: true,
                    jsx_text: true,
                    jsx_tag: false,
                    jsx_gap: false,
                });
                pos = ws + w.len();
            }
            return;
        }
        self.out.push(Tok {
            kind,
            start: s,
            end,
            node: node.kind(),
            parent,
            named: node.is_named(),
            jsx_text: false,
            jsx_tag: self.lang.has_jsx() && in_jsx_tag(node, text),
            jsx_gap: false,
        });
    }

    /// A node whose text is not all in its children (CSS `1px` is an `integer_value` holding the
    /// digits and a `unit` child): it is one token.
    fn has_own_text(&self, node: Node) -> bool {
        let mut pos = node.start_byte();
        let mut c = node.walk();
        for ch in node.children(&mut c) {
            if !self.src[pos..ch.start_byte().max(pos)].trim().is_empty() {
                return true;
            }
            pos = ch.end_byte();
        }
        !self.src[pos..node.end_byte().max(pos)].trim().is_empty()
    }

    fn walk(&mut self, node: Node) {
        if node.is_missing() || node.end_byte() <= self.skip_to && node.start_byte() < self.skip_to
        {
            return;
        }
        if node.child_count() == 0
            || atomic(node.kind())
            || (node.kind() != "ERROR" && self.has_own_text(node))
        {
            if node.start_byte() >= self.skip_to && node.end_byte() > node.start_byte() {
                self.push_leaf(node);
            }
            return;
        }
        let mut c = node.walk();
        for ch in node.children(&mut c) {
            self.walk(ch);
        }
    }
}

fn parse_any(src: &str, lang: Lang) -> Result<tree_sitter::Tree, String> {
    let mut p = Parser::new();
    p.set_language(&lang.language())
        .map_err(|e| e.to_string())?;
    p.parse(src, None).ok_or_else(|| "parse failed".into())
}

/// Parse `src`; error when the tree has an error node and the language's grammar is trusted.
pub fn parse(src: &str, lang: Lang) -> Result<tree_sitter::Tree, String> {
    let mut p = Parser::new();
    p.set_language(&lang.language())
        .map_err(|e| e.to_string())?;
    let tree = p.parse(src, None).ok_or("parse failed")?;
    if tree.root_node().has_error() && !lang.lexical_only() {
        return Err("parse: syntax error".into());
    }
    Ok(tree)
}

/// Tokens and the gaps around them: `gaps.len() == toks.len() + 1`, `gaps[i]` precedes `toks[i]`.
///
/// # Errors
/// The source does not parse, or a gap holds something that is not whitespace (a token the tree hides).
pub fn tokenize(src: &str, lang: Lang) -> Result<(Vec<Tok>, Vec<String>), String> {
    let tree = parse_any(src, lang)?;
    let mut w = Walker {
        src,
        lang,
        out: Vec::new(),
        skip_to: 0,
    };
    w.walk(tree.root_node());
    let mut toks = Vec::new();
    let mut gaps = Vec::with_capacity(w.out.len() + 1);
    let mut pos = 0;
    for t in w.out {
        if t.start < pos {
            return Err("overlapping tokens".into());
        }
        let g = &src[pos..t.start];
        if g.chars().all(char::is_whitespace) {
            gaps.push(g.to_string());
        } else {
            // error recovery dropped text from the tree (a macro the grammar does not know): the
            // pieces become plain tokens, so the file is still compacted and checked lexically
            let mut at = pos;
            for piece in g.split_whitespace() {
                let ps = at + src[at..].find(piece).unwrap_or(0);
                gaps.push(src[at..ps].to_string());
                toks.push(Tok {
                    kind: Kind::Code,
                    start: ps,
                    end: ps + piece.len(),
                    node: "ERROR",
                    parent: "ERROR",
                    named: false,
                    jsx_text: false,
                    jsx_tag: false,
                    jsx_gap: false,
                });
                at = ps + piece.len();
            }
            gaps.push(src[at..t.start].to_string());
        }
        pos = t.end;
        toks.push(t);
    }
    let tail = &src[pos..];
    if tail.chars().all(char::is_whitespace) {
        gaps.push(tail.to_string());
    } else {
        let mut at = pos;
        for piece in tail.split_whitespace() {
            let ps = at + src[at..].find(piece).unwrap_or(0);
            gaps.push(src[at..ps].to_string());
            toks.push(Tok {
                kind: Kind::Code,
                start: ps,
                end: ps + piece.len(),
                node: "ERROR",
                parent: "ERROR",
                named: false,
                jsx_text: false,
                jsx_tag: false,
                jsx_gap: false,
            });
            at = ps + piece.len();
        }
        gaps.push(src[at..].to_string());
    }
    if lang.has_jsx() {
        let root = tree.root_node();
        for k in 1..toks.len() {
            let (a, b) = (toks[k - 1].end, toks[k].start);
            if a < b {
                toks[k].jsx_gap = root
                    .descendant_for_byte_range(a, b)
                    .is_some_and(|n| matches!(n.kind(), "jsx_element" | "jsx_fragment"));
            }
        }
    }
    if lang == Lang::CSharp {
        crate::lookaround::check_raw_strings(src, &toks)?;
    }
    Ok((toks, gaps))
}
