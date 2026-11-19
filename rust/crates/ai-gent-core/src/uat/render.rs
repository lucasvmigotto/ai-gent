//! The page (`ai-gent uat render`, FR-011 to FR-025, FR-050, FR-051, `contracts/page.md`).
//!
//! The page is a template, `template/page.html`, filled from the roadmap. All roadmap text is HTML
//! escaped, the data the script needs goes into one JSON island with `<` escaped, and the attachment
//! controls exist only when `settings.attachments.on`. The validators' accounts (`invite`) never reach
//! the page.
use super::lint::{self, Context, Finding, Severity};
use super::model::{Roadmap, Script, Step};
use serde_json::json;
use std::fmt::Write as _;

const TEMPLATE: &str = include_str!("template/page.html");

/// Escape text for HTML content and attribute values.
#[must_use]
pub fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&#39;"),
            _ => o.push(c),
        }
    }
    o
}

/// Lint, then render. Refuses (with the errors) when the roadmap has any.
///
/// # Errors
/// The lint errors, when there are any.
pub fn render(r: &Roadmap, ctx: &Context) -> Result<String, Vec<Finding>> {
    let errors: Vec<Finding> = lint::lint(r, ctx)
        .into_iter()
        .filter(|f| f.severity == Severity::Error)
        .collect();
    if errors.is_empty() {
        Ok(page(r))
    } else {
        Err(errors)
    }
}

fn role_label<'a>(r: &'a Roadmap, key: &'a str) -> &'a str {
    r.roles
        .iter()
        .find(|x| x.key == key)
        .map_or(key, |x| x.label.as_str())
}

fn paragraphs(text: &str) -> String {
    text.split("\n\n")
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .fold(String::new(), |mut o, p| {
            let _ = writeln!(o, "<p>{}</p>", esc(p));
            o
        })
}

/// The data the page script reads. Names, roles and ids only: never an account or an e-mail.
fn data(r: &Roadmap) -> String {
    let v = json!({
        "version": r.version,
        "attachments": {"on": r.settings.attachments.on, "budget_mb": r.settings.attachments.budget_mb},
        "roles": r.roles.iter().map(|x| json!({"key": x.key, "label": x.label})).collect::<Vec<_>>(),
        "scripts": r.scripts.iter().map(|s| json!({"id": s.id, "title": s.title})).collect::<Vec<_>>(),
        "steps": r.active_steps().map(|(s, st)| json!({
            "id": st.id, "rev": st.rev, "script": s.id, "role": st.who, "kind": st.kind
        })).collect::<Vec<_>>(),
    });
    // `<` as < keeps `</script>` and `<!--` out of the island; it is still valid JSON.
    serde_json::to_string(&v)
        .unwrap_or_default()
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('&', "\\u0026")
}

fn controls(r: &Roadmap, st: &Step) -> String {
    let id = &st.id;
    let mut o = String::new();
    let _ = write!(o, "<div class=\"chips\" id=\"chips-{id}\"></div>");
    let _ = write!(
        o,
        "<div class=\"ctl\"><button type=\"button\" class=\"okb\" id=\"res-{id}-ok\" aria-pressed=\"false\">OK</button> \
<button type=\"button\" class=\"nokb\" id=\"res-{id}-nok\" aria-pressed=\"false\">NOK</button> \
<button type=\"button\" id=\"imp-btn-{id}\">Suggest improvement</button></div>"
    );
    let describe = if r.settings.attachments.on {
        "What happened, when, the message you saw and which screen."
    } else {
        "What happened, when, the message you saw and which screen. Describe the screen in words."
    };
    let _ = write!(
        o,
        "<div class=\"ctl\" id=\"nokbox-{id}\" hidden><label for=\"obs-{id}\">{describe}</label>\
<textarea id=\"obs-{id}\" rows=\"3\" maxlength=\"1500\"></textarea> \
<span class=\"tag nok\" id=\"miss-{id}\" hidden>observation missing</span>\
<label for=\"links-{id}\">Links (optional, up to 3, https only)</label>\
<input type=\"text\" id=\"links-{id}\" maxlength=\"1600\">\
<p class=\"muted\">Do not paste passwords, tokens, cookies or personal data.</p>"
    );
    if r.settings.attachments.on {
        let _ = write!(
            o,
            "<div class=\"ev\"><label for=\"ev-{id}\">Attach a file ({} MB for the whole session). Do not upload cookies, tokens, passwords or personal data.</label>\
<input type=\"file\" id=\"ev-{id}\"><span id=\"evl-{id}\"></span></div>",
            r.settings.attachments.budget_mb
        );
    }
    o.push_str("</div>");
    let _ = write!(
        o,
        "<div class=\"ctl\" id=\"impbox-{id}\" hidden><label for=\"imp-{id}\">Your suggestion</label>\
<textarea id=\"imp-{id}\" rows=\"2\" maxlength=\"1500\"></textarea>\
<p class=\"muted\">Do not paste passwords, tokens, cookies or personal data.</p></div>\
<div id=\"imps-{id}\"></div>"
    );
    if st.kind == "role-confirmation" {
        let _ = write!(
            o,
            "<div class=\"ctl\"><label for=\"prop-role\">If your role is another one, propose it</label>\
<select id=\"prop-role\"><option value=\"\">Keep my role</option>"
        );
        for role in &r.roles {
            let _ = write!(
                o,
                "<option value=\"{}\">{}</option>",
                esc(&role.key),
                esc(&role.label)
            );
        }
        let _ = write!(
            o,
            "</select><label for=\"prop-note\">Note for the owner (optional)</label>\
<textarea id=\"prop-note\" rows=\"2\" maxlength=\"500\"></textarea>\
<button type=\"button\" id=\"prop-save\">Send proposal</button>\
<p class=\"muted\">The owner decides. Nothing changes until then.</p></div>"
        );
    }
    o
}

fn step_row(r: &Roadmap, s: &Script, st: &Step) -> String {
    let who = {
        let mut w = esc(role_label(r, &st.who));
        if let Some(v) = r.validators.iter().find(|v| v.key == st.validator) {
            let _ = write!(w, "<br><span class=\"muted\">{}</span>", esc(&v.label));
        }
        w
    };
    let mut cell = format!(
        "<p>{}</p><p class=\"exp\"><b>Expected:</b> {}</p>",
        esc(&st.action),
        esc(&st.expected)
    );
    if !st.pre.is_empty() {
        let _ = write!(
            cell,
            "<p class=\"muted\">Before you start: {}</p>",
            esc(&st.pre)
        );
    }
    if let Some(w) = st.wait.as_ref().filter(|w| w.seconds > 0) {
        let _ = write!(
            cell,
            "<p class=\"muted\">Wait {} seconds, then look at: {}</p>",
            w.seconds,
            esc(&w.look_at)
        );
    }
    if !st.exclusive_note.is_empty() {
        let _ = write!(
            cell,
            "<p><span class=\"tag stale\">{}</span>",
            esc(&st.exclusive_note)
        );
        if !s.test_data.is_empty() {
            let _ = write!(cell, " Test record: {}", esc(&s.test_data.join(", ")));
        }
        cell.push_str("</p>");
    }
    format!(
        "<tr data-step=\"{id}\" data-rev=\"{rev}\" data-role=\"{role}\" data-kind=\"{kind}\" data-exclusive=\"{ex}\">\
<td>{id}</td><td>{who}</td><td>{cell}</td><td>{ctl}</td></tr>\n",
        id = esc(&st.id),
        rev = st.rev,
        role = esc(&st.who),
        kind = esc(&st.kind),
        ex = s.exclusive,
        ctl = controls(r, st),
    )
}

fn script_section(r: &Roadmap, s: &Script) -> String {
    let mut o = format!(
        "<h3 id=\"{id}\">{} {}</h3>\n<div class=\"scroll\"><table class=\"steps\"><thead><tr>\
<th>#</th><th>Who</th><th>Action and expected result</th><th>Result</th></tr></thead><tbody>\n",
        esc(&s.id.to_uppercase()),
        esc(&s.title),
        id = esc(&s.id)
    );
    for st in s.steps.iter().filter(|x| !x.retired()) {
        o.push_str(&step_row(r, s, st));
    }
    o.push_str("</tbody></table></div>\n");
    o
}

fn list(items: impl Iterator<Item = String>) -> String {
    let body = items.fold(String::new(), |mut o, i| {
        let _ = write!(o, "<li>{i}</li>");
        o
    });
    if body.is_empty() {
        String::new()
    } else {
        format!("<ul class=\"plain\">{body}</ul>\n")
    }
}

/// The page's sections, in the order of FR-001.
#[must_use]
pub fn body(r: &Roadmap) -> String {
    let mut o = String::new();
    let _ = write!(
        o,
        "<section id=\"how-to\"><h2>How to use this page</h2>\n{}</section>\n",
        paragraphs(&r.how_to)
    );
    let _ = write!(
        o,
        "<section id=\"people\"><h2>Who is involved</h2>\n<h3>Roles</h3>\n{}<h3>Validators</h3>\n{}</section>\n",
        list(
            r.roles
                .iter()
                .map(|x| format!("{}: {}", esc(&x.label), esc(&x.can.join("; "))))
        ),
        list(r.validators.iter().map(|v| format!(
            "{} — {}",
            esc(&v.label),
            esc(role_label(r, &v.assigned_role))
        ))),
    );
    o.push_str("<section id=\"roster\"><h2>Roster</h2>\n<p class=\"muted\">Who is placed in which role. Only roles the owner assigned count for coverage.</p>\n<div id=\"roster-body\"></div></section>\n");
    let _ = write!(
        o,
        "<section id=\"rules\"><h2>Rules being tested</h2>\n{}</section>\n",
        list(r.rules.iter().map(|x| esc(&x.text)))
    );
    let _ = write!(
        o,
        "<section id=\"test-data\"><h2>Test data</h2>\n{}</section>\n",
        list(
            r.scripts
                .iter()
                .filter(|s| !s.test_data.is_empty())
                .map(|s| {
                    format!(
                        "{} {}: {}",
                        esc(&s.id.to_uppercase()),
                        esc(&s.title),
                        esc(&s.test_data.join(", "))
                    )
                })
        )
    );
    o.push_str("<section id=\"scripts\"><h2>Scripts</h2>\n");
    for s in &r.scripts {
        o.push_str(&script_section(r, s));
    }
    o.push_str("</section>\n");
    o.push_str(
        "<section id=\"results\"><h2>Results</h2>\n<div id=\"results-body\"></div>\n<h3>NOKs</h3>\n<ul class=\"plain\" id=\"nok-list\"></ul>\n<h3>Suggestions</h3>\n<ul class=\"plain\" id=\"imp-list\"></ul></section>\n",
    );
    let limits = list(r.known_limitations.iter().map(|l| esc(&l.text)));
    let _ = write!(
        o,
        "<section id=\"limitations\"><h2>Known limitations</h2>\n{}</section>\n",
        if limits.is_empty() {
            "<p class=\"muted\">None known.</p>\n".to_string()
        } else {
            limits
        }
    );
    let _ = write!(
        o,
        "<section id=\"changelog\"><h2>Changes</h2>\n{}</section>\n",
        list(r.changelog.iter().rev().map(|c| format!(
            "Version {} — {}",
            c.version,
            esc(&c.summary)
        )))
    );
    o
}

fn nav(r: &Roadmap) -> String {
    let mut items = String::new();
    for s in &r.scripts {
        let _ = write!(
            items,
            "<li><a href=\"#{}\">{} {}</a></li>",
            esc(&s.id),
            esc(&s.id.to_uppercase()),
            esc(&s.title)
        );
    }
    format!("<nav aria-label=\"Scripts\"><ul>{items}</ul></nav>")
}

/// Fill the template. A token is replaced once and its replacement is never scanned again, so roadmap
/// text that looks like a token stays text.
#[must_use]
pub fn page(r: &Roadmap) -> String {
    let mut out = String::with_capacity(TEMPLATE.len() + 4096);
    let mut rest = TEMPLATE;
    while let Some(i) = rest.find("@@") {
        out.push_str(&rest[..i]);
        let after = &rest[i + 2..];
        let Some(j) = after.find("@@") else {
            out.push_str(&rest[i..]);
            rest = "";
            break;
        };
        let value = match &after[..j] {
            "TITLE" => Some(esc(&r.title)),
            "VERSION" => Some(r.version.to_string()),
            "ATTACH" => Some(
                if r.settings.attachments.on {
                    "on"
                } else {
                    "off"
                }
                .to_string(),
            ),
            "ENV" => Some(format!("Environment: {}", esc(&r.environment.name))),
            "NAV" => Some(nav(r)),
            "BODY" => Some(body(r)),
            "DATA" => Some(data(r)),
            _ => None,
        };
        if let Some(v) = value {
            out.push_str(&v);
            rest = &after[j + 2..];
        } else {
            out.push_str("@@");
            rest = after;
        }
    }
    out.push_str(rest);
    out
}
