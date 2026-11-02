//! Rewrite the gaps between tokens; token text is never touched.

use crate::tokenize::{Kind, Lang, Tok};

/// Which transforms are on (`blank`, `indent`, `join`, `spaces`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Flags(u8);

const BLANK: u8 = 1;
const INDENT: u8 = 2;
const JOIN: u8 = 4;
const SPACES: u8 = 8;

impl Flags {
    /// Flags of a level.
    #[must_use]
    pub fn from_level(l: u8) -> Flags {
        Flags(match l {
            0 => 0,
            1 => BLANK,
            2 => BLANK | INDENT,
            3 => BLANK | INDENT | JOIN,
            _ => BLANK | INDENT | JOIN | SPACES,
        })
    }
    /// Level that these flags make up.
    #[must_use]
    pub fn level(self) -> u8 {
        match self.0 {
            0 => 0,
            BLANK => 1,
            x if x == BLANK | INDENT => 2,
            x if x == BLANK | INDENT | JOIN => 3,
            _ => 4,
        }
    }
    /// No transform left.
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.0 == 0
    }
    /// Drop one flag in the fallback order spaces, join, indent, blank; returns its name.
    pub fn drop_one(&mut self) -> &'static str {
        for (bit, name) in [
            (SPACES, "spaces"),
            (JOIN, "join"),
            (INDENT, "indent"),
            (BLANK, "blank"),
        ] {
            if self.0 & bit != 0 {
                self.0 &= !bit;
                return name;
            }
        }
        "none"
    }
    fn has(self, bit: u8) -> bool {
        self.0 & bit != 0
    }
}

fn word(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$' || (c as u32) > 127
}
const OPCH: &str = "+-*/%=<>!&|^~?:.@#\\";

/// Does removing the gap between `p` and `n` change how a lexer reads them?
fn need_space(p: &str, n: &str, n_kind: Kind) -> bool {
    let (a, b) = (
        p.chars().last().unwrap_or(' '),
        n.chars().next().unwrap_or(' '),
    );
    (word(a) && word(b))
        || (OPCH.contains(a) && OPCH.contains(b))
        || (b == '.' && a.is_ascii_digit())
        || (a == '.' && b.is_ascii_digit())
        || (word(a) && "\"'`".contains(b))
        || ("\"'`".contains(a) && word(b))
        || (word(a) && b == '#')
        || matches!(n_kind, Kind::Com | Kind::Line)
}

const JS_END: &[&str] = &[
    "{", "(", "[", ",", ";", ".", "?.", "...", "=>", "=", "+=", "-=", "*=", "/=", "%=", "**=",
    "<<=", ">>=", ">>>=", "&=", "|=", "^=", "&&=", "||=", "??=", "&&", "||", "??", "?", ":", "+",
    "-", "*", "/", "%", "**", "&", "|", "^", "!", "~", "<", "==", "===", "!=", "!==", "<=", ">=",
    "<<",
];
const JS_END_WORDS: &[&str] = &[
    "instanceof",
    "in",
    "typeof",
    "delete",
    "new",
    "else",
    "extends",
    "implements",
    "case",
    "do",
    "try",
    "finally",
];
const JS_START: &[&str] = &[
    ")", "]", "}", ",", ";", ".", "?.", ":", "?", "&&", "||", "??", "==", "===", "!=", "!==", "=",
    "+=", "-=", "*=", "/=", "%=", "**=", "<<=", ">>=", ">>>=", "&=", "|=", "^=", "&&=", "||=",
    "??=", "|", "&", "*", "/", "%", "<", ">", ">=", "<=", "<<", ">>", ">>>", "**", "^",
];
const JS_RESTRICTED: &[&str] = &[
    "return",
    "throw",
    "break",
    "continue",
    "yield",
    "async",
    "await",
    "get",
    "set",
    "static",
    "let",
    "abstract",
    "declare",
    "type",
    "namespace",
    "module",
    "readonly",
    "override",
    "accessor",
];
const AFTER_CLOSE: &[&str] = &["else", "catch", "finally"];
const BLOCK_PARENTS: &[&str] = &[
    "statement_block",
    "class_body",
    "enum_body",
    "switch_body",
    "interface_body",
    "object_type",
    "internal_module",
    "module",
];

struct Ctx<'a> {
    src: &'a str,
    toks: &'a [Tok],
    gaps: &'a [String],
    lang: Lang,
    js_semi: bool,
}

impl Ctx<'_> {
    fn text(&self, k: usize) -> &str {
        self.toks[k].text(self.src)
    }

    /// The `js` family writes `;` for the statement end; only code written that way may join at `}`.
    fn semicolon_style(&self) -> bool {
        let (mut semi, mut bare) = (0, 0);
        for k in 1..self.toks.len() {
            if !self.gaps[k].contains('\n') || self.toks[k - 1].kind != Kind::Code {
                continue;
            }
            let p = self.text(k - 1);
            if p == ";" {
                semi += 1;
            } else if (matches!(p, ")" | "]") || p.chars().last().is_some_and(word))
                && !JS_START.contains(&self.text(k))
            {
                bare += 1;
            }
        }
        semi >= 3 && f64::from(bare) <= f64::from(semi) * 0.15
    }

    /// Own-line comments keep their line: they annotate what follows.
    fn comment_pinned(&self, k: usize) -> bool {
        let nl = |i: usize| i < self.gaps.len() && self.gaps[i].contains('\n');
        let own = |i: usize| self.toks[i].kind == Kind::Com && (i == 0 || nl(i)) && nl(i + 1);
        own(k) || own(k - 1) || (self.toks[k].kind == Kind::Com && nl(k))
    }

    /// May the newline before token `k` be removed?
    fn can_join(&self, k: usize) -> bool {
        let (p, n) = (&self.toks[k - 1], &self.toks[k]);
        if p.kind == Kind::Line || n.kind == Kind::Line || self.comment_pinned(k) {
            return false;
        }
        if !self.lang.is_js() {
            return true;
        }
        let (pv, nv) = (self.text(k - 1), self.text(k));
        if p.kind == Kind::Code && JS_RESTRICTED.contains(&pv)
            || matches!(nv, "++" | "--")
            || matches!(pv, "++" | "--")
        {
            return false;
        }
        // an expanded object literal stays expanded: the formatter would expand it again
        if pv == "{" && p.parent == "object" {
            return false;
        }
        if p.kind == Kind::Code && (JS_END.contains(&pv) || JS_END_WORDS.contains(&pv)) {
            return true;
        }
        if n.kind == Kind::Code && JS_START.contains(&nv) {
            return true;
        }
        pv == "}"
            && BLOCK_PARENTS.contains(&p.parent)
            && (self.js_semi || AFTER_CLOSE.contains(&nv))
    }

    fn sep(&self, k: usize) -> &'static str {
        let (pv, nv) = (self.text(k - 1), self.text(k));
        if self.lang == Lang::Tsx && pv.ends_with('>') && nv.starts_with('<') {
            return ""; // a space between two JSX tags would become a text node
        }
        if need_space(pv, nv, self.toks[k].kind) {
            " "
        } else {
            ""
        }
    }

    /// A blank line that formatters treat as intent: between comment groups and import groups.
    fn keeps_paragraph(&self, k: usize, gap: &str) -> bool {
        if gap.matches('\n').count() < 2 {
            return false;
        }
        let (p, n) = (&self.toks[k - 1], &self.toks[k]);
        if matches!(p.kind, Kind::Com | Kind::Line) || matches!(n.kind, Kind::Com | Kind::Line) {
            return true;
        }
        let mut j = k;
        if matches!(self.text(j), "pub" | "export") && j + 1 < self.toks.len() {
            j += 1;
        }
        matches!(self.text(j), "use" | "import" | "using") && matches!(self.text(k - 1), ";" | "}")
    }
}

fn gap_for(cx: &Ctx, k: usize, flags: Flags) -> String {
    let gap = &cx.gaps[k];
    let nl = gap.matches('\n').count();
    let tail = gap.rfind('\n').map_or("", |i| &gap[i + 1..]);
    if nl == 0 {
        return if gap.is_empty() {
            String::new()
        } else if flags.has(SPACES) {
            cx.sep(k).to_string()
        } else if flags.has(INDENT) {
            " ".into()
        } else {
            gap.clone()
        };
    }
    let para = cx.keeps_paragraph(k, gap);
    if flags.has(JOIN) && !para && cx.can_join(k) {
        return cx.sep(k).to_string();
    }
    let indent = if flags.has(INDENT) { "" } else { tail };
    if para {
        return format!("\n\n{indent}");
    }
    if flags.has(BLANK) {
        return format!("\n{indent}");
    }
    gap.clone()
}

/// Render with `flags`. Level 0 is the source itself.
#[must_use]
pub fn render(src: &str, toks: &[Tok], gaps: &[String], lang: Lang, flags: Flags) -> String {
    if toks.is_empty() || flags.is_empty() {
        return src.to_string();
    }
    let mut cx = Ctx {
        src,
        toks,
        gaps,
        lang,
        js_semi: false,
    };
    cx.js_semi = lang.is_js() && cx.semicolon_style();
    let mut out = String::with_capacity(src.len());
    let g0 = &gaps[0];
    if !flags.has(BLANK) && !flags.has(INDENT) {
        out.push_str(g0);
    } else if !flags.has(INDENT) {
        out.push_str(g0.rfind('\n').map_or(g0.as_str(), |i| &g0[i + 1..]));
    }
    for (k, tok) in toks.iter().enumerate() {
        if k > 0 {
            out.push_str(&gap_for(&cx, k, flags));
        }
        out.push_str(tok.text(src));
    }
    let tail = &gaps[toks.len()];
    out.push_str(
        if flags.has(BLANK) || flags.has(INDENT) || tail.is_empty() {
            "\n"
        } else {
            tail
        },
    );
    out
}
