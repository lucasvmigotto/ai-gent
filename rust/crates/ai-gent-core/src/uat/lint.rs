//! Roadmap checks (`ai-gent uat lint`, FR-001 to FR-010, FR-040, FR-080, FR-091, FR-098).
//!
//! Pure: the roadmap goes in, findings come out. Mode B needs the features' scenarios, which the caller
//! reads with [`super::scenarios`] and passes in.
use super::model::{LABELS, Roadmap, Step};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

/// How bad a finding is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    /// Blocks `render`.
    Error,
    /// Reported, does not block.
    Warn,
}

/// One thing wrong with a roadmap.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Finding {
    /// Error or warning.
    pub severity: Severity,
    /// A stable kebab-case code.
    pub code: &'static str,
    /// The script, step or section it concerns.
    pub at: String,
    /// What is wrong, in a line.
    pub message: String,
}

/// What `lint` needs besides the roadmap.
#[derive(Debug, Default)]
pub struct Context {
    /// Mode B: each feature's scenario anchors (`specs/NNN-x/spec.md#US1-AS1`), by feature.
    pub scenarios: BTreeMap<String, Vec<String>>,
    /// Overrides `settings.doc_budget_bytes`.
    pub budget: Option<u64>,
}

/// The most text a mark can hold: observation, improvement and three links (data model, FR-098).
pub const MARK_BYTES: u64 = 1500 + 1500 + 3 * 500;
/// Page and register overhead counted in the size check.
pub const OVERHEAD_BYTES: u64 = 20_000;

struct Out(Vec<Finding>);

impl Out {
    fn err(&mut self, code: &'static str, at: &str, message: impl Into<String>) {
        self.0.push(Finding {
            severity: Severity::Error,
            code,
            at: at.to_string(),
            message: message.into(),
        });
    }
    fn warn(&mut self, code: &'static str, at: &str, message: impl Into<String>) {
        self.0.push(Finding {
            severity: Severity::Warn,
            code,
            at: at.to_string(),
            message: message.into(),
        });
    }
}

/// Check a roadmap. Findings come in a stable order: sections, ids, scripts, steps, then the global checks.
#[must_use]
pub fn lint(r: &Roadmap, ctx: &Context) -> Vec<Finding> {
    let mut o = Out(Vec::new());
    sections(r, &mut o);
    environment(r, &mut o);
    ids(r, &mut o);
    people(r, &mut o);
    scripts(r, &mut o);
    for (script, step) in r.steps() {
        step_checks(r, &script.id, step, &mut o);
    }
    rules(r, &mut o);
    page_text(r, &mut o);
    mode_b(r, ctx, &mut o);
    size(r, ctx, &mut o);
    o.0
}

/// Whether any finding is an error.
#[must_use]
pub fn has_errors(findings: &[Finding]) -> bool {
    findings.iter().any(|f| f.severity == Severity::Error)
}

fn sections(r: &Roadmap, o: &mut Out) {
    if r.title.trim().is_empty() {
        o.err("missing-section", "title", "the roadmap has no title");
    }
    if r.how_to.trim().is_empty() {
        o.err(
            "missing-section",
            "how_to",
            "no how-to: OK versus NOK versus improvement, what to write, exclusivity and waits (FR-001)",
        );
    }
    if r.roles.is_empty() {
        o.err("missing-section", "roles", "no roles (FR-001)");
    }
    if r.validators.is_empty() {
        o.err("missing-section", "validators", "no validators (FR-001)");
    }
    if r.rules.is_empty() {
        o.err(
            "missing-section",
            "rules",
            "no rules in plain words (FR-001)",
        );
    }
    if r.scripts.is_empty() {
        o.err("missing-section", "scripts", "no scripts (FR-001)");
    }
    if r.changelog.is_empty() {
        o.err(
            "missing-section",
            "changelog",
            "no changelog entry; the first version needs one (FR-001)",
        );
    }
    if !matches!(r.mode.as_str(), "A" | "B") {
        o.err(
            "bad-mode",
            "mode",
            "mode is `A` (whole application) or `B` (features)",
        );
    }
    if r.version == 0 {
        o.err("bad-version", "version", "version starts at 1");
    }
}

fn environment(r: &Roadmap, o: &mut Out) {
    if r.environment.name.trim().is_empty() {
        o.err("missing-section", "environment", "no environment name");
    }
    let class = r.environment.class.trim().to_ascii_lowercase();
    if class.is_empty() {
        o.err(
            "environment-unconfirmed",
            "environment",
            "the environment has no class; the owner must confirm it is a test environment (FR-080)",
        );
    } else if class == "production" || class == "prod" {
        o.err(
            "environment-production",
            "environment",
            "a production environment is refused (FR-080)",
        );
    }
}

fn is_script_id(id: &str) -> bool {
    id.strip_prefix('r')
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

fn step_parts(id: &str) -> Option<(&str, &str)> {
    let rest = id.strip_prefix('r')?;
    let (n, m) = rest.split_once('-')?;
    let ok = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    (ok(n) && ok(m)).then_some((n, m))
}

fn ids(r: &Roadmap, o: &mut Out) {
    let mut seen_scripts = BTreeSet::new();
    for s in &r.scripts {
        if !is_script_id(&s.id) {
            o.err("bad-id", &s.id, "a script id is `r<N>`");
        } else if !seen_scripts.insert(s.id.as_str()) {
            o.err("duplicate-id", &s.id, "two scripts share this id");
        }
        if !matches!(s.kind.as_str(), "prep" | "flow" | "rule" | "complement") {
            o.err(
                "bad-kind",
                &s.id,
                "a script is `prep`, `flow`, `rule` or `complement` (FR-003)",
            );
        }
    }
    let mut seen = BTreeSet::new();
    for (s, st) in r.steps() {
        match step_parts(&st.id) {
            None => o.err("bad-id", &st.id, "a step id is `r<N>-<M>`"),
            Some((n, _)) if format!("r{n}") != s.id => o.err(
                "bad-id",
                &st.id,
                format!("the id belongs to another script than {}", s.id),
            ),
            Some(_) => {}
        }
        if !seen.insert(st.id.as_str()) {
            o.err("duplicate-id", &st.id, "two steps share this id");
        }
        if !matches!(st.status.as_str(), "active" | "retired") {
            o.err("bad-status", &st.id, "status is `active` or `retired`");
        }
        if !matches!(
            st.kind.as_str(),
            "normal" | "write-check" | "role-confirmation"
        ) {
            o.err(
                "bad-kind",
                &st.id,
                "kind is `normal`, `write-check` or `role-confirmation`",
            );
        }
        if st.rev == 0 {
            o.err("bad-revision", &st.id, "the revision starts at 1");
        }
    }
}

fn people(r: &Roadmap, o: &mut Out) {
    let roles: BTreeSet<&str> = r.roles.iter().map(|x| x.key.as_str()).collect();
    let mut keys = BTreeSet::new();
    for v in &r.validators {
        if !keys.insert(v.key.as_str()) {
            o.err("duplicate-id", &v.key, "two validators share this key");
        }
        if v.assigned_role.trim().is_empty() {
            o.err(
                "validator-no-role",
                &v.key,
                "the owner assigns every validator a role (FR-091)",
            );
        } else if !roles.contains(v.assigned_role.as_str()) {
            o.err(
                "unknown-role",
                &v.key,
                format!(
                    "assigned role `{}` is not a role of this roadmap",
                    v.assigned_role
                ),
            );
        }
        if v.label.trim().is_empty() {
            o.err(
                "validator-no-label",
                &v.key,
                "the page needs a label for this person (D9)",
            );
        }
    }
    for (_, st) in r.steps() {
        if !st.who.is_empty() && !roles.contains(st.who.as_str()) {
            o.err(
                "unknown-role",
                &st.id,
                format!("target role `{}` is not a role of this roadmap", st.who),
            );
        }
        if !st.validator.is_empty() && !keys.contains(st.validator.as_str()) {
            o.err(
                "unknown-validator",
                &st.id,
                format!("named validator `{}` is not on the list", st.validator),
            );
        }
    }
}

fn is_record_name(name: &str, script: &str) -> bool {
    let Some(rest) = name.strip_prefix(script).and_then(|x| x.strip_prefix('-')) else {
        return false;
    };
    let date = rest.get(..8).unwrap_or("");
    date.len() == 8
        && date.bytes().all(|b| b.is_ascii_digit())
        && rest[8..].strip_prefix('-').is_some_and(|t| !t.is_empty())
}

fn scripts(r: &Roadmap, o: &mut Out) {
    match r.scripts.first() {
        Some(s) if s.id == "r0" => r0(s, o),
        _ => o.err(
            "missing-r0",
            "r0",
            "the first script is R0, the preparation script (FR-002)",
        ),
    }
    let mut owner: BTreeMap<&str, &str> = BTreeMap::new();
    for s in &r.scripts {
        for rec in &s.test_data {
            if !is_record_name(rec, &s.id) {
                o.err(
                    "test-record-name",
                    &s.id,
                    format!(
                        "test record `{rec}` is not `{}-<yyyymmdd>-<name>` (FR-010)",
                        s.id
                    ),
                );
            }
            if let Some(first) = owner.insert(rec.as_str(), s.id.as_str())
                && first != s.id
            {
                o.err(
                    "shared-test-record",
                    &s.id,
                    format!("test record `{rec}` is also used by {first} (FR-010)"),
                );
            }
        }
        if s.exclusive && s.test_data.is_empty() {
            o.err(
                "exclusive-no-record",
                &s.id,
                "an exclusive script names the test record nobody else may touch",
            );
        }
        if !s.cleanup_step.is_empty() && !s.steps.iter().any(|st| st.id == s.cleanup_step) {
            o.err(
                "unknown-step",
                &s.id,
                format!("cleanup step `{}` is not in the script", s.cleanup_step),
            );
        }
    }
}

fn r0(s: &super::model::Script, o: &mut Out) {
    let has = |kind: &str| s.steps.iter().any(|st| !st.retired() && st.kind == kind);
    if !has("write-check") {
        o.err(
            "missing-r0",
            "r0",
            "R0 has no write-check step: each validator proves they can write (FR-002, FR-093)",
        );
    }
    if !has("role-confirmation") {
        o.err(
            "missing-r0",
            "r0",
            "R0 has no role-confirmation step (FR-002, FR-091)",
        );
    }
}

fn step_checks(r: &Roadmap, script: &str, st: &Step, o: &mut Out) {
    if st.retired() {
        return;
    }
    let at = st.id.as_str();
    if st.who.is_empty() {
        o.err("missing-field", at, "the step has no target role (`who`)");
    }
    if st.action.trim().is_empty() {
        o.err("missing-field", at, "the step has no action");
    }
    if st.expected.trim().is_empty() {
        o.err(
            "unobservable-expected",
            at,
            "no expected result on screen for the target role (FR-006)",
        );
    }
    if !st.seen_by.is_empty() && st.seen_by != st.who {
        o.err(
            "result-seen-by-other-role",
            at,
            format!(
                "only `{}` can see the result; make it a separate step for that role (FR-006)",
                st.seen_by
            ),
        );
    }
    if st
        .wait
        .as_ref()
        .is_some_and(|w| w.seconds > 0 && w.look_at.trim().is_empty())
    {
        o.err("wait-no-target", at, "a wait says what to look at (FR-016)");
    }
    let parent = r.scripts.iter().find(|s| s.id == script);
    if let Some(p) = parent {
        let first_active = p.steps.iter().find(|x| !x.retired()).map(|x| x.id.as_str());
        if p.kind != "prep"
            && !p.test_data.is_empty()
            && st.pre.trim().is_empty()
            && first_active != Some(at)
        {
            o.err(
                "missing-precondition",
                at,
                "the script uses test records, so the step says what must exist first",
            );
        }
        if p.exclusive && st.exclusive_note.trim().is_empty() && first_active == Some(at) {
            o.err(
                "missing-exclusivity",
                at,
                "the script is exclusive; the first step carries the note (FR-015)",
            );
        }
    }
    if st.claims.is_empty() && st.kind == "normal" {
        o.warn(
            "no-claims",
            at,
            "the step records no claim, so the review has nothing to check (FR-007)",
        );
    }
    for (i, c) in st.claims.iter().enumerate() {
        let here = format!("{at} claim {}", i + 1);
        if !LABELS.contains(&c.label.as_str()) {
            o.err(
                "claim-without-label",
                &here,
                "every claim carries OBSERVED, INFERRED or ASSUMPTION (FR-007)",
            );
        }
        if c.label == "INFERRED" && c.reference.trim().is_empty() {
            o.err(
                "inferred-no-confirmation",
                &here,
                "an INFERRED claim names how to confirm it (FR-007)",
            );
        }
        if c.label == "OBSERVED" && c.reference.trim().is_empty() {
            o.err(
                "observed-no-ref",
                &here,
                "an OBSERVED claim names where it was seen",
            );
        }
    }
}

fn rules(r: &Roadmap, o: &mut Out) {
    for rule in &r.rules {
        if rule.authorization
            && !r
                .active_steps()
                .any(|(_, st)| st.rule == rule.key && st.refused)
        {
            o.err(
                "rule-no-refused-step",
                &rule.key,
                "an authorization rule needs a step whose target role must be refused (FR-004)",
            );
        }
    }
    let known: BTreeSet<&str> = r.rules.iter().map(|x| x.key.as_str()).collect();
    for (_, st) in r.active_steps() {
        if !st.rule.is_empty() && !known.contains(st.rule.as_str()) {
            o.err(
                "unknown-rule",
                &st.id,
                format!("rule `{}` is not in the rules section", st.rule),
            );
        }
    }
}

/// Words that give SQL away when they come in this order.
const SQL_PAIRS: [(&str, &str); 6] = [
    ("select", "from"),
    ("insert", "into"),
    ("delete", "from"),
    ("update", "set"),
    ("drop", "table"),
    ("alter", "table"),
];

const FILE_EXT: [&str; 14] = [
    "rs", "py", "ts", "tsx", "js", "json", "md", "sh", "sql", "yml", "yaml", "toml", "html", "java",
];

/// What in `text` looks like code, a path, an e-mail or an internal id, or `None`.
#[must_use]
pub fn code_like(text: &str) -> Option<&'static str> {
    if text.contains('`') || text.contains("::") || text.contains("()") || text.contains("=>") {
        return Some("code");
    }
    let lower = text.to_ascii_lowercase();
    let words: Vec<&str> = lower
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    for (a, b) in SQL_PAIRS {
        if let Some(i) = words.iter().position(|w| *w == a)
            && words[i + 1..].contains(&b)
        {
            return Some("SQL");
        }
    }
    for tok in text.split_whitespace() {
        let t = tok.trim_matches(|c: char| ",.;:!?()[]\"'".contains(c));
        if let Some((local, domain)) = t.split_once('@')
            && !local.is_empty()
            && domain.contains('.')
        {
            return Some("an e-mail address");
        }
        let slashes = t.matches('/').count();
        let ext = t.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase());
        let file = ext.is_some_and(|e| FILE_EXT.contains(&e.as_str())) && t.len() > 3;
        if slashes >= 2
            || t.starts_with("./")
            || t.starts_with("../")
            || t.starts_with("~/")
            || file
        {
            return Some("a file path");
        }
        let snake = t.contains('_')
            && t.split('_')
                .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_alphanumeric()));
        let camel = t.len() > 3
            && t.bytes().next().is_some_and(|b| b.is_ascii_lowercase())
            && t.bytes().all(|b| b.is_ascii_alphanumeric())
            && t.bytes().any(|b| b.is_ascii_uppercase());
        if snake || camel {
            return Some("an identifier");
        }
    }
    None
}

fn page_text(r: &Roadmap, o: &mut Out) {
    let mut check = |at: &str, text: &str| {
        if let Some(what) = code_like(text) {
            o.err(
                "code-like-text",
                at,
                format!("the page text has {what}; say it in plain words (FR-010)"),
            );
        }
    };
    check("title", &r.title);
    check("how_to", &r.how_to);
    check("environment", &r.environment.name);
    for x in &r.roles {
        check(&format!("role {}", x.key), &x.label);
        for c in &x.can {
            check(&format!("role {}", x.key), c);
        }
    }
    for v in &r.validators {
        check(&format!("validator {}", v.key), &v.label);
    }
    for x in &r.rules {
        check(&format!("rule {}", x.key), &x.text);
    }
    for l in &r.known_limitations {
        check("known limitation", &l.text);
    }
    for s in &r.scripts {
        check(&s.id, &s.title);
        for st in s.steps.iter().filter(|x| !x.retired()) {
            for (field, text) in [
                ("action", &st.action),
                ("expected", &st.expected),
                ("pre", &st.pre),
                ("exclusive note", &st.exclusive_note),
                ("evidence", &st.evidence_expected),
            ] {
                check(&format!("{} {field}", st.id), text);
            }
            if let Some(w) = &st.wait {
                check(&format!("{} wait", st.id), &w.look_at);
            }
        }
    }
}

fn mode_b(r: &Roadmap, ctx: &Context, o: &mut Out) {
    if r.mode != "B" {
        return;
    }
    if r.features.is_empty() {
        o.err(
            "missing-section",
            "features",
            "mode B names the features under test (FR-008)",
        );
    }
    for f in &r.features {
        let Some(anchors) = ctx.scenarios.get(f) else {
            o.err(
                "feature-unreadable",
                f,
                "the feature's spec was not read, so its scenarios cannot be checked (FR-008)",
            );
            continue;
        };
        for a in anchors {
            let key = a.rsplit('#').next().unwrap_or(a);
            let traced =
                |t: &str| t == a || (t.ends_with(&format!("#{key}")) && t.contains(f.as_str()));
            let by_step = r
                .active_steps()
                .any(|(_, st)| st.traces.iter().any(|t| traced(t)));
            let by_list = r.not_testable.iter().any(|n| traced(&n.trace));
            if !by_step && !by_list {
                o.err(
                    "scenario-without-step",
                    a,
                    "no step traces to this scenario and it is not listed as not testable (FR-008)",
                );
            }
        }
    }
    for n in &r.not_testable {
        if n.reason.trim().is_empty() {
            o.err(
                "not-testable-no-reason",
                &n.trace,
                "a scenario not testable by hand says why",
            );
        }
    }
}

fn size(r: &Roadmap, ctx: &Context, o: &mut Out) {
    let Some(budget) = ctx.budget.or(r.settings.doc_budget_bytes) else {
        return;
    };
    let steps = r.active_steps().count() as u64;
    let worst = steps * MARK_BYTES + OVERHEAD_BYTES;
    if worst > budget {
        o.warn(
            "size-budget",
            "scripts",
            format!(
                "{steps} active steps could fill {worst} bytes of marks per validator; the budget is {budget} (FR-098)"
            ),
        );
    }
}

/// The findings as text, one line each, then a summary.
#[must_use]
pub fn text(findings: &[Finding]) -> String {
    let mut out = String::new();
    for f in findings {
        let sev = match f.severity {
            Severity::Error => "error",
            Severity::Warn => "warn ",
        };
        let _ = writeln!(out, "{sev} {:<26} {:<18} {}", f.code, f.at, f.message);
    }
    let errors = findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .count();
    let warns = findings.len() - errors;
    let _ = writeln!(out, "{errors} error(s), {warns} warning(s)");
    out
}
