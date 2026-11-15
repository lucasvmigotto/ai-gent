//! Rewrite the gaps between tokens; token text is never touched.

use crate::tokenize::{Family, Kind, Lang, Tok};

/// Which transforms are on (`blank`, `indent`, `join`, `spaces`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Flags(u8);

const BLANK: u8 = 1;
const INDENT: u8 = 2;
const JOIN: u8 = 4;
const SPACES: u8 = 8;
const PARA: u8 = 16;

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
        match self.0 & !PARA {
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
        self.0 & !PARA == 0
    }
    /// The same flags, also keeping one blank line where the source had blank lines.
    #[must_use]
    pub fn with_paragraphs(self) -> Flags {
        if self.0 & (BLANK | INDENT | JOIN) != 0 {
            Flags(self.0 | PARA)
        } else {
            self
        }
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
                if bit == BLANK {
                    self.0 &= !PARA;
                }
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

fn css_sep(p: &str, n: &str) -> bool {
    let (a, b) = (
        p.chars().last().unwrap_or(' '),
        n.chars().next().unwrap_or(' '),
    );
    !("{};,:".contains(a) || "{};,".contains(b))
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
const AFTER_CLOSE: &[&str] = &["else", "catch", "finally", "while"];
const OBJ_BEFORE: &[&str] = &[
    "(", ",", "=", ":", "[", "?", "||", "&&", "??", "return", "...", "<", "default",
];
const HEADER_WORDS: &[&str] = &[
    "if", "for", "while", "when", "catch", "switch", "with", "foreach",
];
const JS_NO_STMT_START: &[&str] = &[
    "in",
    "instanceof",
    "of",
    "as",
    "satisfies",
    "from",
    "extends",
    "implements",
    "else",
    "catch",
    "finally",
    "keyof",
    "is",
];
const JS_CONTEXTUAL: &[&str] = &[
    "static",
    "get",
    "set",
    "async",
    "abstract",
    "declare",
    "readonly",
    "public",
    "private",
    "protected",
    "override",
    "accessor",
    "export",
    "default",
    "let",
    "type",
    "namespace",
    "module",
    "interface",
    "enum",
    "as",
    "satisfies",
    "of",
    "in",
    "from",
    "new",
    "typeof",
    "void",
    "delete",
    "await",
    "case",
    "var",
    "const",
    "function",
    "class",
    "extends",
    "implements",
    "import",
    "else",
    "do",
    "try",
];
const KT_MODIFIERS: &[&str] = &[
    "private",
    "public",
    "internal",
    "protected",
    "override",
    "open",
    "final",
    "abstract",
    "lateinit",
    "const",
    "inline",
    "suspend",
    "operator",
    "infix",
    "tailrec",
    "external",
    "actual",
    "expect",
    "inner",
    "data",
    "sealed",
    "enum",
    "annotation",
    "companion",
    "value",
    "vararg",
    "noinline",
    "crossinline",
    "reified",
    "fun",
    "val",
    "var",
    "class",
    "object",
    "interface",
    "typealias",
    "import",
    "package",
    "in",
    "out",
    "is",
    "as",
    "by",
    "where",
    "constructor",
    "get",
    "set",
    "init",
    "else",
    "if",
    "when",
    "try",
    "do",
    "for",
    "while",
];
const KT_NO_STMT_START: &[&str] = &[
    "get",
    "set",
    "by",
    "where",
    "constructor",
    "as",
    "is",
    "in",
    "else",
    "catch",
    "finally",
    "out",
];
const KT_END: &[&str] = &[
    "{", "(", "[", ",", ";", "=", "+=", "-=", "*=", "/=", "%=", "->", "&&", "||", "?:", "+", "-",
    "*", "/", "%", "..", "..<", "<", "==", "!=", "===", "!==", "<=", ">=", ":", "?.", "::",
];
const KT_END_WORDS: &[&str] = &["else", "in", "is", "as", "try", "finally", "do"];
const KT_START: &[&str] = &[
    ")", "]", "}", ",", ".", "?.", "?:", "&&", "||", "->", ":", "=", ";",
];
const KT_START_WORDS: &[&str] = &["catch", "finally"];
const GO_KEYWORDS: &[&str] = &[
    "break",
    "case",
    "chan",
    "const",
    "continue",
    "default",
    "defer",
    "else",
    "fallthrough",
    "for",
    "func",
    "go",
    "goto",
    "if",
    "import",
    "interface",
    "map",
    "package",
    "range",
    "return",
    "select",
    "struct",
    "switch",
    "type",
    "var",
];
/// Keywords that are values: the `Keyword.Constant` and `Keyword.Type` of the Pygments lexers.
const VALUE_WORDS: &[&str] = &[
    "true",
    "false",
    "null",
    "undefined",
    "this",
    "super",
    "nil",
    "void",
    "number",
    "string",
    "boolean",
    "any",
    "unknown",
    "never",
    "object",
    "symbol",
    "bigint",
];

/// Would the Go lexer insert a semicolon after token `p` at a newline?
#[must_use]
pub fn go_needs_semi(p: &Tok, src: &str) -> bool {
    let v = p.text(src);
    match p.kind {
        Kind::Str => true,
        Kind::Code => {
            if matches!(
                v,
                "break" | "continue" | "fallthrough" | "return" | "++" | "--" | ")" | "]" | "}"
            ) {
                return true;
            }
            if GO_KEYWORDS.contains(&v) {
                return false;
            }
            (p.named || VALUE_WORDS.contains(&v)) && v.chars().last().is_some_and(word)
        }
        _ => false,
    }
}

struct Ctx<'a> {
    src: &'a str,
    toks: &'a [Tok],
    gaps: &'a [String],
    lang: Lang,
    fam: Family,
    js_semi: bool,
    no_semi: bool,
    mate: Vec<Option<usize>>,
    go_imports: Vec<bool>,
}

impl<'a> Ctx<'a> {
    fn new(src: &'a str, toks: &'a [Tok], gaps: &'a [String], lang: Lang) -> Self {
        let mut cx = Ctx {
            src,
            toks,
            gaps,
            lang,
            fam: lang.family(),
            js_semi: false,
            no_semi: false,
            mate: vec![None; toks.len()],
            go_imports: vec![false; toks.len()],
        };
        let mut stack = Vec::new();
        for i in 0..toks.len() {
            if cx.toks[i].kind != Kind::Code {
                continue;
            }
            match cx.text(i) {
                "(" | "[" | "{" => stack.push(i),
                ")" | "]" | "}" => {
                    if let Some(o) = stack.pop() {
                        cx.mate[i] = Some(o);
                        cx.mate[o] = Some(i);
                    }
                }
                _ => {}
            }
        }
        if cx.fam == Family::Go {
            for i in 0..toks.len().saturating_sub(1) {
                if cx.text(i) == "import" && cx.text(i + 1) == "(" {
                    if let Some(c) = cx.mate[i + 1] {
                        for j in i + 2..c {
                            cx.go_imports[j] = true;
                        }
                    }
                }
            }
        }
        cx.js_semi = cx.fam == Family::Js && cx.semicolon_style();
        cx
    }

    fn text(&self, k: usize) -> &str {
        self.toks[k].text(self.src)
    }

    fn punct(&self, k: usize) -> bool {
        self.toks[k].kind == Kind::Code && !self.text(k).chars().next().is_some_and(word)
    }

    /// An identifier, number or value keyword (the `Name`, `Literal`, `Keyword.Constant` and
    /// `Keyword.Type` token classes of the Pygments lexers).
    fn value_token(&self, k: usize) -> bool {
        let t = &self.toks[k];
        let v = self.text(k);
        t.kind == Kind::Code
            && v.chars().next().is_some_and(word)
            && (t.named || t.parent == "predefined_type" || VALUE_WORDS.contains(&v))
    }

    fn name(&self, k: usize) -> bool {
        let t = &self.toks[k];
        t.kind == Kind::Code && t.named && t.node.contains("identifier")
    }

    fn number(&self, k: usize) -> bool {
        let n = self.toks[k].node;
        self.toks[k].kind == Kind::Code
            && (n.contains("number")
                || n.contains("integer")
                || n.contains("float")
                || n.contains("decimal"))
    }

    /// The `js` family writes `;` for the statement end; only code written that way may join at `}`.
    fn semicolon_style(&self) -> bool {
        let (mut semi, mut bare) = (0, 0);
        for k in 1..self.toks.len() {
            let pk = self.toks[k - 1].kind;
            if !self.gaps[k].contains('\n') || !matches!(pk, Kind::Code | Kind::Str) {
                continue;
            }
            let p = self.text(k - 1);
            if p == ";" && pk == Kind::Code {
                semi += 1;
            } else if (pk == Kind::Str || matches!(p, ")" | "]") || self.value_token(k - 1))
                && !(self.punct(k) && JS_START.contains(&self.text(k)))
                && !p.starts_with('@')
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

    fn annotation_end(&self, i: usize) -> bool {
        let mut j = i;
        loop {
            if i - j > 8 {
                return false;
            }
            let v = self.text(j);
            if v.starts_with('@') {
                return true;
            }
            if !(matches!(v, "." | ":") || self.name(j)) || (j < i && !self.gaps[j + 1].is_empty())
            {
                return false;
            }
            if j == 0 {
                return false;
            }
            j -= 1;
        }
    }

    fn kt_infix(&self, i: usize) -> bool {
        if i < 1 || !self.name(i) || self.gaps[i].is_empty() || self.gaps[i].contains('\n') {
            return false;
        }
        let q = i - 1;
        let qv = self.text(q);
        if self.toks[q].kind == Kind::Str || matches!(qv, ")" | "]" | "}" | "!!") {
            return true;
        }
        self.toks[q].kind == Kind::Code
            && (self.name(q) || self.number(q) || matches!(qv, "true" | "false" | "null" | "this"))
            && !KT_MODIFIERS.contains(&qv)
            && !matches!(qv, "return" | "throw")
            && !self.annotation_end(q)
    }

    /// Is the `}` at index `i` the end of a block or declaration (not an object literal, arrow body
    /// or function expression, which end an expression that ASI terminates)?
    fn js_block_close(&self, i: usize) -> bool {
        let Some(o) = self.mate[i] else { return false };
        if o == 0 {
            return true;
        }
        let pv = self.text(o - 1);
        if OBJ_BEFORE.contains(&pv) || pv == "=>" {
            return false;
        }
        if pv == ")" {
            let Some(po) = self.mate[o - 1] else {
                return true;
            };
            if po == 0 {
                return true;
            }
            let mut j = po - 1;
            if HEADER_WORDS.contains(&self.text(j)) {
                return true;
            }
            if self.name(j) && j > 0 {
                j -= 1;
            }
            if self.text(j) == "*" && j > 0 {
                j -= 1;
            }
            if self.text(j) == "function" {
                let mut q = if j > 0 { Some(j - 1) } else { None };
                if q.is_some_and(|x| self.text(x) == "async") {
                    q = if j > 1 { Some(j - 2) } else { None };
                }
                return !q
                    .is_some_and(|x| OBJ_BEFORE.contains(&self.text(x)) || self.text(x) == "=>");
            }
            return true;
        }
        if pv == "class" || (o >= 2 && self.text(o - 2) == "class") {
            let q = if pv != "class" && o >= 3 {
                Some(o - 3)
            } else if o >= 2 {
                Some(o - 2)
            } else {
                None
            };
            return !q.is_some_and(|x| OBJ_BEFORE.contains(&self.text(x)));
        }
        true
    }

    /// Does token `i` end an expression or statement?
    fn value_end(&self, i: usize) -> bool {
        match self.toks[i].kind {
            Kind::Str => return true,
            Kind::Code => {}
            _ => return false,
        }
        let v = self.text(i);
        if v.starts_with('@') {
            return false;
        }
        let kotlin = self.fam == Family::Kotlin;
        if kotlin && matches!(v, "return" | "throw" | "break" | "continue") {
            return false;
        }
        if v == ")" {
            let Some(o) = self.mate[i] else { return false };
            if o == 0 {
                return false;
            }
            return !HEADER_WORDS.contains(&self.text(o - 1)) && !self.annotation_end(o - 1);
        }
        if matches!(v, "++" | "--") {
            // postfix only if glued to its operand on the same line (`i\n++\nj` is `i; ++j`)
            return i > 0
                && !self.gaps[i].contains('\n')
                && (matches!(self.text(i - 1), ")" | "]") || self.name(i - 1));
        }
        if matches!(v, "]" | "}") || (kotlin && matches!(v, "!!" | "?")) {
            return true;
        }
        if !kotlin
            && matches!(
                v,
                "return"
                    | "break"
                    | "continue"
                    | "throw"
                    | "yield"
                    | "debugger"
                    | "this"
                    | "super"
                    | "null"
                    | "true"
                    | "false"
            )
        {
            return true;
        }
        if v == "void" && matches!(self.lang, Lang::TypeScript | Lang::Tsx) {
            return true;
        }
        if (!kotlin && JS_CONTEXTUAL.contains(&v)) || (kotlin && KT_MODIFIERS.contains(&v)) {
            return false;
        }
        if self.annotation_end(i) || (kotlin && self.kt_infix(i)) {
            return false;
        }
        self.value_token(i) && v.chars().last().is_some_and(word)
    }

    /// Can this newline be written as `;`? Only where the language would end the statement there
    /// anyway (JS ASI, Kotlin newline-as-separator): never inside a control header, before a
    /// continuation, after an annotation, or in JSX text.
    fn semi_join(&self, k: usize) -> bool {
        let (p, n) = (&self.toks[k - 1], &self.toks[k]);
        if !matches!(self.fam, Family::Js | Family::Kotlin)
            || (self.no_semi && self.fam == Family::Js)
            || p.jsx_text
            || n.jsx_text
            || p.jsx_tag
            || n.jsx_tag
            || !self.value_end(k - 1)
        {
            return false;
        }
        let kotlin = self.fam == Family::Kotlin;
        let v = self.text(k);
        if n.kind == Kind::Str {
            let c = v.chars().next();
            return if kotlin {
                c == Some('"')
            } else {
                matches!(c, Some('\'' | '"'))
            };
        }
        if n.kind != Kind::Code {
            return false;
        }
        if !kotlin {
            if JS_NO_STMT_START.contains(&v) {
                return false;
            }
            if matches!(v, "!" | "~" | "++" | "--") || v.starts_with(['@', '#']) {
                return true;
            }
            if self.number(k) {
                return !v.starts_with('.');
            }
            return v.chars().next().is_some_and(word);
        }
        if KT_NO_STMT_START.contains(&v) {
            return false;
        }
        let mut j = k;
        while j < self.toks.len() && j < k + 12 {
            let t = self.text(j);
            if t.starts_with('@')
                || (matches!(t, "." | ":") && self.gaps[j].is_empty())
                || (self.name(j) && j > k && matches!(self.text(j - 1), "." | ":" | "@"))
            {
                j += 1;
                if j < self.toks.len() && self.text(j) == "(" {
                    if let Some(c) = self.mate[j] {
                        j = c + 1;
                    }
                }
                continue;
            }
            if KT_MODIFIERS.contains(&t) && !KT_NO_STMT_START.contains(&t) {
                j += 1;
                continue;
            }
            break;
        }
        if j < self.toks.len() && KT_NO_STMT_START.contains(&self.text(j)) {
            return false;
        }
        if matches!(v, "!" | "++" | "--") || v.starts_with('@') {
            return true;
        }
        v.chars().next().is_some_and(word)
    }

    /// May the newline before token `k` be removed?
    fn can_join(&self, k: usize) -> bool {
        let (p, n) = (&self.toks[k - 1], &self.toks[k]);
        if matches!(p.kind, Kind::Line | Kind::Pre)
            || matches!(n.kind, Kind::Line | Kind::Pre)
            || self.comment_pinned(k)
        {
            return false;
        }
        match self.fam {
            Family::Free | Family::Go | Family::Css => return true,
            _ => {}
        }
        let (pv, nv) = (self.text(k - 1), self.text(k));
        if self.fam == Family::Js {
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
            if (self.punct(k - 1) && JS_END.contains(&pv))
                || (p.kind == Kind::Code && JS_END_WORDS.contains(&pv))
            {
                return true;
            }
            if (self.punct(k) && JS_START.contains(&nv))
                || (n.kind == Kind::Code && nv == "instanceof")
            {
                return true;
            }
            return pv == "}"
                && p.kind == Kind::Code
                && (self.js_semi || AFTER_CLOSE.contains(&nv))
                && self.js_block_close(k - 1);
        }
        // kotlin
        if matches!(pv, "++" | "--" | "!!") || matches!(nv, "++" | "--") {
            return false;
        }
        if self.kt_infix(k - 1) {
            return true;
        }
        if (self.punct(k - 1) && KT_END.contains(&pv))
            || (p.kind == Kind::Code && KT_END_WORDS.contains(&pv))
        {
            return true;
        }
        if (self.punct(k) && KT_START.contains(&nv))
            || (n.kind == Kind::Code && KT_START_WORDS.contains(&nv))
        {
            return true;
        }
        pv == "}" && AFTER_CLOSE.contains(&nv) && nv != "while"
    }

    fn sep(&self, k: usize) -> &'static str {
        let (pv, nv) = (self.text(k - 1), self.text(k));
        match self.fam {
            Family::Css => return if css_sep(pv, nv) { " " } else { "" },
            Family::Js if pv.ends_with('>') && nv.starts_with('<') => return "",
            _ => {}
        }
        if need_space(pv, nv, self.toks[k].kind) {
            " "
        } else {
            ""
        }
    }

    /// The text that replaces a joined newline (Go re-inserts the semicolon its lexer would).
    fn join_sep(&self, k: usize) -> String {
        if self.fam == Family::Go {
            let p = &self.toks[k - 1];
            let last = if p.kind == Kind::Com {
                (0..k)
                    .rev()
                    .find(|&i| !matches!(self.toks[i].kind, Kind::Com | Kind::Line))
            } else {
                Some(k - 1)
            };
            if let Some(q) = last {
                if go_needs_semi(&self.toks[q], self.src)
                    && !matches!(self.text(k), ")" | "}" | ";")
                {
                    return if p.kind == Kind::Com { " ;" } else { ";" }.into();
                }
            }
        }
        self.sep(k).into()
    }

    /// A blank line that formatters treat as intent: between comment groups and import groups.
    fn keeps_paragraph(&self, k: usize, gap: &str, flags: Flags) -> bool {
        if gap.matches('\n').count() < 2 {
            return false;
        }
        if flags.has(PARA) {
            return true;
        }
        let (p, n) = (&self.toks[k - 1], &self.toks[k]);
        if matches!(p.kind, Kind::Com | Kind::Line) || matches!(n.kind, Kind::Com | Kind::Line) {
            return true;
        }
        self.import_group_break(k)
    }

    fn import_group_break(&self, k: usize) -> bool {
        let (p, n) = (&self.toks[k - 1], &self.toks[k]);
        let includes = |t: &Tok| {
            let v = t.text(self.src).trim_start_matches(['#', ' ', '\t']);
            v.starts_with("include") || v.starts_with("import")
        };
        if n.kind == Kind::Pre && p.kind == Kind::Pre {
            return includes(n) && includes(p);
        }
        let mut j = k;
        if matches!(self.text(j), "pub" | "export") && j + 1 < self.toks.len() {
            j += 1;
        }
        let word_ok = matches!(self.text(j), "use" | "import" | "from" | "using");
        if (word_ok && p.kind == Kind::Code && matches!(self.text(k - 1), ";" | "}"))
            || (self.fam == Family::Kotlin && self.text(j) == "import")
        {
            return true;
        }
        if self.lang == Lang::Rust {
            // skip #[attributes] and visibility to the item keyword
            let mut j = k;
            while j < self.toks.len() {
                if self.text(j) == "#" && j + 1 < self.toks.len() && self.text(j + 1) == "[" {
                    j = self.mate[j + 1].map_or(j + 2, |c| c + 1);
                    continue;
                }
                if matches!(self.text(j), "pub" | "(" | ")" | "crate" | "super" | "in") {
                    j += 1;
                    continue;
                }
                break;
            }
            return j < self.toks.len() && matches!(self.text(j), "use" | "extern" | "mod");
        }
        self.go_imports[k]
    }

    /// JSX whitespace rules (`_gap` of `compact.py`): `Some(gap)` when they decide.
    fn jsx_gap(&self, k: usize, flags: Flags) -> Option<String> {
        if !self.lang.has_jsx() {
            return None;
        }
        let (p, n) = (&self.toks[k - 1], &self.toks[k]);
        let gap = &self.gaps[k];
        let nl = gap.contains('\n');
        if matches!(p.kind, Kind::Line | Kind::Pre)
            || matches!(n.kind, Kind::Line | Kind::Pre)
            || self.comment_pinned(k)
        {
            return None;
        }
        let (pv, nv) = (self.text(k - 1), self.text(k));
        let text = n.jsx_text || p.jsx_text;
        let tag = n.jsx_tag || p.jsx_tag;
        if nl
            && flags.has(JOIN)
            && tag
            && !matches!(pv, "<" | "</")
            && !matches!(nv, ">" | "/>" | "/")
        {
            return Some(" ".into()); // between JSX attributes a newline is just a separator
        }
        if nl && flags.has(JOIN) && text && n.jsx_text != p.jsx_text {
            return Some(String::new()); // JSX drops whitespace-with-newline between text and a tag
        }
        if !gap.is_empty() && !nl && (text || n.jsx_gap) {
            return Some(gap.clone()); // a space between JSX children is text
        }
        if !gap.is_empty() && n.jsx_text && p.jsx_text {
            let indent = if flags.has(INDENT) {
                ""
            } else {
                &gap[gap.rfind('\n').map_or(0, |i| i + 1)..]
            };
            return Some(format!("\n{indent}"));
        }
        None
    }
}

fn gap_for(cx: &Ctx, k: usize, flags: Flags, hold: bool) -> String {
    let gap = &cx.gaps[k];
    if let Some(g) = cx.jsx_gap(k, flags) {
        return g;
    }
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
    let para = cx.keeps_paragraph(k, gap, flags);
    if flags.has(JOIN) && !para && !hold && cx.can_join(k) {
        return cx.join_sep(k);
    }
    if flags.has(JOIN) && !para && !hold && cx.semi_join(k) {
        return ";".into(); // the newline ended a statement: say so explicitly
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

/// Output options of [`render`] beyond the flags.
#[derive(Debug, Clone, Copy, Default)]
pub struct Opts {
    /// Keep line breaks at brace depth `<= n` (0 = one top-level item per line).
    pub outline: Option<usize>,
    /// Soft-wrap a line at an original line break once it holds this many bytes (0 = never).
    pub wrap: usize,
    /// Prefix each output line with its original line number.
    pub numbers: bool,
    /// Never write a joined newline as `;` (the grammar cannot vouch for the file).
    pub no_semi: bool,
}

/// Render with `flags`. Level 0 is the source itself.
#[must_use]
pub fn render(
    src: &str,
    toks: &[Tok],
    gaps: &[String],
    lang: Lang,
    flags: Flags,
    o: Opts,
) -> String {
    if toks.is_empty() {
        return if o.numbers {
            number_lines(src, &line_numbers_identity(src))
        } else {
            src.to_string()
        };
    }
    let mut cx = Ctx::new(src, toks, gaps, lang);
    cx.no_semi = o.no_semi;
    let nl_offsets: Vec<usize> = src.match_indices('\n').map(|(i, _)| i).collect();
    let lineno = |off: usize| nl_offsets.partition_point(|&n| n < off) + 1;
    let mut out = String::with_capacity(src.len());
    let mut src_line: Vec<Option<usize>> = vec![None];
    let mut line_len = 0usize;
    let mut depth = 0usize;
    let g0 = &gaps[0];
    if !flags.has(BLANK) && !flags.has(INDENT) {
        out.push_str(g0);
    } else if !flags.has(INDENT) {
        out.push_str(g0.rfind('\n').map_or(g0.as_str(), |i| &g0[i + 1..]));
    }
    src_line.extend(std::iter::repeat_n(None, out.matches('\n').count()));
    line_len += out.len() - out.rfind('\n').map_or(0, |i| i + 1);
    for (k, tok) in toks.iter().enumerate() {
        if k > 0 {
            let hold = o.outline.is_some_and(|d| depth <= d);
            let mut s = gap_for(&cx, k, flags, hold);
            if o.wrap > 0 && line_len >= o.wrap && gaps[k].contains('\n') && !s.contains('\n') {
                s = format!("{}\n", s.trim_end_matches(' ')); // restoring an original line break is always safe
            }
            let nls = s.matches('\n').count();
            if nls > 0 {
                line_len = s.len() - s.rfind('\n').map_or(0, |i| i + 1);
                src_line.extend(std::iter::repeat_n(None, nls));
            } else {
                line_len += s.len();
            }
            out.push_str(&s);
        }
        let first = lineno(tok.start);
        if let Some(last) = src_line.last_mut() {
            if last.is_none() {
                *last = Some(first);
            }
        }
        let text = tok.text(src);
        out.push_str(text);
        let inner = text.matches('\n').count();
        if inner > 0 {
            line_len = text.len() - text.rfind('\n').map_or(0, |i| i + 1);
            src_line.extend((1..=inner).map(|j| Some(first + j)));
        } else {
            line_len += text.len();
        }
        if tok.kind == Kind::Code {
            match text {
                "{" => depth += 1,
                "}" => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
    }
    let tail = &gaps[toks.len()];
    out.push_str(
        if flags.has(BLANK) || flags.has(INDENT) || tail.is_empty() {
            "\n"
        } else {
            tail
        },
    );
    if o.numbers {
        number_lines(&out, &src_line)
    } else {
        out
    }
}

fn line_numbers_identity(src: &str) -> Vec<Option<usize>> {
    (1..=src.matches('\n').count() + 1).map(Some).collect()
}

/// `N|line` for every non-empty output line (`width` of the largest number); lines with no known
/// source line get blanks in place of the number.
fn number_lines(text: &str, src_line: &[Option<usize>]) -> String {
    let width = src_line
        .iter()
        .flatten()
        .max()
        .map_or(1, |m| m.to_string().len());
    text.split('\n')
        .enumerate()
        .map(
            |(i, ln)| match (ln.is_empty(), src_line.get(i).copied().flatten()) {
                (true, _) => String::new(),
                (false, Some(n)) => format!("{n:>width$}|{ln}"),
                (false, None) => format!("{:width$}|{ln}", ""),
            },
        )
        .collect::<Vec<_>>()
        .join("\n")
}
