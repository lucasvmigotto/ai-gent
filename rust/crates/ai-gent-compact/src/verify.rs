//! The safety check: same tokens, same tree shape.

use crate::render::go_needs_semi;
use crate::tokenize::{Family, Kind, Lang, Tok, parse, tokenize_view};
use tree_sitter::Node;

/// Statement-level node kinds (the `STRUCT_RE` rule of `compact.py`): only structure that a missing or
/// extra newline could change is compared; expression-level labels are ambiguous in GLR grammars.
const STRUCT: &[&str] = &[
    "statement",
    "declaration",
    "definition",
    "block",
    "body",
    "clause",
    "program",
    "source_file",
    "module",
    "class",
    "function",
    "method",
    "lambda",
    "arrow",
    "jsx",
    "element",
    "case",
    "return",
    "export",
    "import",
    "namespace",
    "interface",
    "enum",
    "struct",
    "impl",
    "trait",
    "object",
];
const WRAPPERS: &[&str] = &["expression_statement"];
const SKIP: &[&str] = &["empty_statement"];

fn shape(src: &str, node: Node, semi_free: bool, strip: bool, out: &mut Vec<String>) {
    let k = node.kind();
    let open = node.is_named()
        && !node.is_missing()
        && !WRAPPERS.contains(&k)
        && !SKIP.contains(&k)
        && STRUCT.iter().any(|w| k.contains(w));
    if open {
        out.push(format!("<{k}"));
    }
    if node.child_count() == 0 {
        let text = &src[node.start_byte()..node.end_byte()];
        // a `;` may stand for a newline (js family); JSX whitespace-only text is not content
        let skip = (strip && k.contains("comment") && !crate::tokenize::is_directive(text))
            || (semi_free && text == ";")
            || (k == "jsx_text" && text.trim().is_empty());
        if !skip && !node.is_missing() && node.end_byte() > node.start_byte() {
            out.push("L".into()); // a token between two structure marks: a `;` that moved is caught here
        }
        return;
    }
    let mut c = node.walk();
    for ch in node.children(&mut c) {
        shape(src, ch, semi_free, strip, out);
    }
    if open {
        out.push(">".into());
    }
}

const OPCH: &str = "+-*/%=<>!&|^~?:.@#\\";

fn word(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '$' || (c as u32) > 127
}

/// The check for the languages whose grammars reject valid one-line code (Go, Kotlin): the compact
/// text cannot be re-parsed, so it is compared as a character stream (`lexical_diff` of
/// `compact.py`). Whitespace-free characters must be equal, except that a `;` may stand where the
/// original had a statement-ending newline; for Go that `;` must be exactly where its lexer inserts
/// one, and a place where it would insert one keeps a newline or a `;`. No two tokens that were
/// apart may touch as a word or operator, and strings, comments and preprocessor lines survive
/// verbatim and in order.
fn last_code(toks: &[Tok], k: usize) -> Option<&Tok> {
    toks[..k]
        .iter()
        .rev()
        .find(|t| !matches!(t.kind, Kind::Com | Kind::Line))
}

fn verify_lexical(
    orig: &str,
    new: &str,
    toks: &[Tok],
    gaps: &[String],
    lang: Lang,
) -> Result<(), String> {
    let go = lang == Lang::Go;
    // original: stream, token boundaries (stream index -> token), newline/gap flags
    let mut s0: Vec<char> = Vec::new();
    let mut tok_at = std::collections::HashMap::new();
    for (k, t) in toks.iter().enumerate() {
        tok_at.insert(s0.len(), k);
        s0.extend(t.text(orig).chars().filter(|c| !c.is_whitespace()));
    }
    // new: stream, whitespace and newline before each stream index
    let (mut s1, mut ws1, mut nl1) = (
        Vec::new(),
        std::collections::HashSet::new(),
        std::collections::HashSet::new(),
    );
    let (mut saw_ws, mut saw_nl) = (false, false);
    for c in new.chars() {
        if c.is_whitespace() {
            saw_ws = true;
            saw_nl |= c == '\n';
        } else {
            if saw_ws {
                ws1.insert(s1.len());
            }
            if saw_nl {
                nl1.insert(s1.len());
            }
            saw_ws = false;
            saw_nl = false;
            s1.push(c);
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut skipped_semi = false;
    while i < s0.len() || j < s1.len() {
        if let Some(&k) = tok_at.get(&i).filter(|_| i < s0.len() || j < s1.len()) {
            let nl = k > 0 && gaps[k].contains('\n');
            if j < s1.len()
                && s1[j] == ';'
                && !(i < s0.len() && s0[i] == ';')
                && nl
                && !skipped_semi
            {
                if go && !last_code(toks, k).is_some_and(|q| go_needs_semi(q, orig)) {
                    return Err("go: a `;` where the lexer inserts none".into());
                }
                j += 1;
                skipped_semi = true;
                continue;
            }
            // a boundary the original had, with a gap: the tokens must stay apart
            if k > 0 && !gaps[k].is_empty() && i > 0 && j > 0 && !ws1.contains(&j) && !skipped_semi
            {
                let (a, c) = (s0[i - 1], s0[i]);
                let fused = (word(a) && word(c))
                    || (OPCH.contains(a) && OPCH.contains(c) && !(a == '>' && c == '<'));
                if fused && toks[k - 1].kind == Kind::Code && toks[k].kind == Kind::Code {
                    return Err(format!(
                        "tokens fused near {:?}",
                        s0[i.saturating_sub(8)..(i + 8).min(s0.len())]
                            .iter()
                            .collect::<String>()
                    ));
                }
            }
            if go
                && nl
                && last_code(toks, k).is_some_and(|q| go_needs_semi(q, orig) && q.text(orig) != ";")
                && !matches!(toks[k].text(orig), ")" | "}" | ";")
                && !nl1.contains(&j)
                && !skipped_semi
            {
                return Err(format!(
                    "go: a newline that ends a statement was removed near {:?}",
                    s0[i.saturating_sub(8)..(i + 8).min(s0.len())]
                        .iter()
                        .collect::<String>()
                ));
            }
            skipped_semi = false;
        }
        if i < s0.len() && j < s1.len() && s0[i] == s1[j] {
            i += 1;
            j += 1;
        } else {
            return Err(format!(
                "chars differ near {:?}",
                s0[i.saturating_sub(15)..(i + 15).min(s0.len())]
                    .iter()
                    .collect::<String>()
            ));
        }
    }
    // strings, comments and preprocessor lines: verbatim, in order
    let mut pos = 0;
    for t in toks.iter().filter(|t| t.kind != Kind::Code) {
        let text = t.text(orig);
        match new[pos..].find(text) {
            Some(at) => pos += at + text.len(),
            None => {
                return Err(format!(
                    "literal differs: {:?}",
                    &text[..text.len().min(40)]
                ));
            }
        }
    }
    Ok(())
}

/// Compare the original and the compacted text. Token texts (strings and comments verbatim) must be
/// equal, except that the `js` and `kotlin` families may write `;` where the original had a
/// statement-ending newline and Go is compared by the semicolons its lexer inserts; then the
/// sequence of statement-level node kinds must be equal (not for Go and Kotlin, whose grammars
/// reject some valid one-line code: their newlines are checked lexically).
///
/// # Errors
/// What differs.
pub fn verify_how(orig: &str, new: &str, lang: Lang, strip: bool) -> Result<&'static str, String> {
    let (ta, ga) = tokenize_view(orig, lang, strip)?;
    // a grammar that rejects the original (or valid one-line code, Go and Kotlin) cannot vouch for
    // the output: the check is lexical (`bad_a` in `verify` of `compact.py`)
    if lang.lexical_only() || parse(orig, lang).is_err() {
        return verify_lexical(orig, new, &ta, &ga, lang).map(|()| "lex");
    }
    let (tb, _) = tokenize_view(new, lang, strip).map_err(|e| format!("compact output: {e}"))?;
    match lang.family() {
        Family::Js => {
            let (mut i, mut j) = (0, 0);
            while i < ta.len() || j < tb.len() {
                if i < ta.len() && j < tb.len() && ta[i].text(orig) == tb[j].text(new) {
                    i += 1;
                    j += 1;
                } else if j < tb.len()
                    && tb[j].kind == Kind::Code
                    && tb[j].text(new) == ";"
                    && i < ta.len()
                    && ga[i].contains('\n')
                {
                    j += 1; // a `;` where the original had a statement-ending newline
                } else {
                    let a = ta.get(i).map_or("<end>", |t| t.text(orig));
                    let b = tb.get(j).map_or("<end>", |t| t.text(new));
                    return Err(format!("token {a:?} vs {b:?}"));
                }
            }
        }
        _ => {
            if ta.len() != tb.len() {
                return Err(format!("token count {} vs {}", ta.len(), tb.len()));
            }
            for (a, b) in ta.iter().zip(&tb) {
                if a.text(orig) != b.text(new) {
                    return Err(format!("token {:?} vs {:?}", a.text(orig), b.text(new)));
                }
            }
        }
    }
    let (mut sa, mut sb) = (Vec::<String>::new(), Vec::<String>::new());
    let semi_free = lang.family() == Family::Js;
    shape(
        orig,
        parse(orig, lang)?.root_node(),
        semi_free,
        strip,
        &mut sa,
    );
    shape(
        new,
        parse(new, lang)?.root_node(),
        semi_free,
        strip,
        &mut sb,
    );
    if sa != sb {
        let i = sa
            .iter()
            .zip(&sb)
            .position(|(a, b)| a != b)
            .unwrap_or(sa.len().min(sb.len()));
        return Err(format!(
            "syntax tree shape differs at node {i}: {:?} vs {:?}",
            &sa[i.saturating_sub(2)..(i + 2).min(sa.len())],
            &sb[i.saturating_sub(2)..(i + 2).min(sb.len())]
        ));
    }
    Ok("lex+ast")
}

/// [`verify_how`] without the name of the check that vouched.
///
/// # Errors
/// What differs.
pub fn verify(orig: &str, new: &str, lang: Lang, strip: bool) -> Result<(), String> {
    verify_how(orig, new, lang, strip).map(|_| ())
}
