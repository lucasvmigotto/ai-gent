//! The safety check: same tokens, same tree shape.

use crate::tokenize::{parse, tokenize, Lang};
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

fn shape(node: Node, out: &mut Vec<String>) {
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
        if !node.is_missing() && node.end_byte() > node.start_byte() {
            out.push("L".into()); // a token between two structure marks: a `;` that moved is caught here
        }
        return;
    }
    let mut c = node.walk();
    for ch in node.children(&mut c) {
        shape(ch, out);
    }
    if open {
        out.push(">".into());
    }
}

/// Compare the original and the compacted text: the token texts (strings and comments verbatim, comments
/// compared with whitespace folded) and the sequence of named node kinds must be equal.
///
/// # Errors
/// What differs.
pub fn verify(orig: &str, new: &str, lang: Lang) -> Result<(), String> {
    let (ta, _) = tokenize(orig, lang)?;
    let (tb, _) = tokenize(new, lang).map_err(|e| format!("compact output: {e}"))?;
    if ta.len() != tb.len() {
        return Err(format!("token count {} vs {}", ta.len(), tb.len()));
    }
    for (a, b) in ta.iter().zip(&tb) {
        if a.text(orig) != b.text(new) {
            return Err(format!("token {:?} vs {:?}", a.text(orig), b.text(new)));
        }
    }
    let (mut sa, mut sb) = (Vec::<String>::new(), Vec::<String>::new());
    shape(parse(orig, lang)?.root_node(), &mut sa);
    shape(parse(new, lang)?.root_node(), &mut sb);
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
    Ok(())
}
