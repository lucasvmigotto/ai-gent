//! `to_html` of `specsync.py`: just enough Markdown to HTML for Azure DevOps fields (headings, lists,
//! checkboxes, code, bold). Ported line by line; the golden test in `tests` lists every construct.

fn escape(t: &str) -> String {
    let mut o = String::with_capacity(t.len());
    for c in t.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&#x27;"),
            c => o.push(c),
        }
    }
    o
}

/// Replace `open CONTENT open` (CONTENT: one or more characters that are not `stop`) by `<tag>CONTENT</tag>`,
/// scanning left to right without overlap, like `re.sub(open + "([^stop]+)" + open, ...)`.
fn wrap(text: &str, open: &str, stop: char, tag: &str) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i < text.len() {
        if text[i..].starts_with(open) {
            let body = &text[i + open.len()..];
            let n = body.find(stop).unwrap_or(body.len());
            if n > 0 && body[n..].starts_with(open) {
                out.push_str(&format!("<{tag}>{}</{tag}>", &body[..n]));
                i += open.len() + n + open.len();
                continue;
            }
        }
        let c = text[i..].chars().next().unwrap_or(' ');
        out.push(c);
        i += c.len_utf8();
    }
    out
}

fn inline(t: &str) -> String {
    let t = escape(t);
    let t = wrap(&t, "`", '`', "code");
    wrap(&t, "**", '*', "b")
}

/// `^\s*(?:-|\d+\.) (?:\[( |x)\] )?(.*)$`: the checkbox mark (if any) and the item text.
fn list_item(line: &str) -> Option<(Option<char>, &str)> {
    let s = line.trim_start();
    let rest = if let Some(r) = s.strip_prefix('-') {
        r
    } else {
        let d = s.chars().take_while(char::is_ascii_digit).count();
        if d == 0 {
            return None;
        }
        s[d..].strip_prefix('.')?
    };
    let rest = rest.strip_prefix(' ')?;
    for (open, mark) in [("[ ] ", ' '), ("[x] ", 'x')] {
        if let Some(r) = rest.strip_prefix(open) {
            return Some((Some(mark), r));
        }
    }
    Some((None, rest))
}

/// `^(#{1,6})\s+(.*)$`: the heading level and text.
fn heading(line: &str) -> Option<(usize, &str)> {
    let n = line.chars().take_while(|c| *c == '#').count();
    if n == 0 || n > 6 {
        return None;
    }
    let rest = &line[n..];
    let t = rest.trim_start();
    if t.len() == rest.len() {
        return None;
    }
    Some((n, t))
}

/// Markdown subset to HTML.
#[must_use]
pub fn to_html(md: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut para: Vec<String> = Vec::new();
    let mut in_list = false;

    fn flush(out: &mut Vec<String>, para: &mut Vec<String>, in_list: &mut bool) {
        if !para.is_empty() {
            out.push(format!("<p>{}</p>", para.join(" ")));
            para.clear();
        }
        if *in_list {
            out.push("</ul>".to_string());
            *in_list = false;
        }
    }

    for line in crate::tokens::util::splitlines(md) {
        if line.starts_with("<!--") {
            flush(&mut out, &mut para, &mut in_list);
            out.push(line.to_string());
            continue;
        }
        if let Some((mark, text)) = list_item(line) {
            if !para.is_empty() {
                out.push(format!("<p>{}</p>", para.join(" ")));
                para.clear();
            }
            if !in_list {
                out.push("<ul>".to_string());
                in_list = true;
            }
            let box_ = match mark {
                Some(' ') => "\u{2610} ",
                Some(_) => "\u{2611} ",
                None => "",
            };
            out.push(format!("<li>{box_}{}</li>", inline(text)));
            continue;
        }
        let h = heading(line);
        if h.is_some() || line.trim().is_empty() {
            flush(&mut out, &mut para, &mut in_list);
            if let Some((n, text)) = h {
                let n = (n + 1).min(6);
                out.push(format!("<h{n}>{}</h{n}>", inline(text)));
            }
            continue;
        }
        if in_list && line.starts_with("  ") {
            if let Some(last) = out.last_mut() {
                let cut = last.len() - 5;
                *last = format!("{} {}</li>", &last[..cut], inline(line.trim()));
            }
            continue;
        }
        if in_list {
            out.push("</ul>".to_string());
            in_list = false;
        }
        para.push(inline(line.trim()));
    }
    flush(&mut out, &mut para, &mut in_list);
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::to_html;

    /// Every construct the Python supports; expected strings were produced by `specsync.to_html`.
    #[test]
    fn golden_constructs() {
        let md = "<!-- ai-gent:sync specs/012-export US1 -->\nUser story **1** of `x < y` & \"q\" 'a'\n\n### Tasks\n- [x] T002 done `code`\n- [ ] T003 open\n  continued line\n1. numbered\nplain after list\n# Top\nline one\nline two\n\n- plain **bold** item";
        let want = "<!-- ai-gent:sync specs/012-export US1 -->\n<p>User story <b>1</b> of <code>x &lt; y</code> &amp; &quot;q&quot; &#x27;a&#x27;</p>\n<h4>Tasks</h4>\n<ul>\n<li>\u{2611} T002 done <code>code</code></li>\n<li>\u{2610} T003 open continued line</li>\n<li>numbered</li>\n</ul>\n<p>plain after list</p>\n<h2>Top</h2>\n<p>line one line two</p>\n<ul>\n<li>plain <b>bold</b> item</li>\n</ul>";
        assert_eq!(to_html(md), want);
    }

    #[test]
    fn unmatched_markers_stay() {
        assert_eq!(to_html("a ` b ** c"), "<p>a ` b ** c</p>");
        assert_eq!(to_html("``x``"), "<p>`<code>x</code>`</p>");
    }
}
