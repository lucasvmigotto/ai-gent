//! `chain`: the branch chain from the branches that exist (work, integration, optional staging, production).
use super::Ctx;
use super::remote::git;
use crate::tokens::pyjson::{J, dumps, obj};
use std::collections::BTreeSet;

const LEVELS: [(&str, &[&str]); 3] = [
    ("integration", &["dev", "develop"]),
    ("staging", &["staging", "homolog", "homologation", "hml"]),
    ("production", &["main", "master"]),
];

/// The computed chain.
#[derive(Debug, Clone)]
pub struct Chain {
    /// The remote's default branch.
    pub default: Option<String>,
    /// Levels with more than one candidate.
    pub ambiguous: Vec<String>,
    /// Integration branch.
    pub integration: Option<String>,
    /// Staging branch.
    pub staging: Option<String>,
    /// Production branch.
    pub production: Option<String>,
    /// Display order.
    pub order: Vec<String>,
    /// Where a story's work counts as integrated.
    pub integrated_on: Option<String>,
    /// Where closing keywords act.
    pub keywords_close_on: Option<String>,
    /// Whether the flow must ask the user.
    pub ask: bool,
}

/// Compute the chain.
#[must_use]
pub fn chain(ctx: &Ctx) -> Chain {
    let remote = ctx
        .env
        .text("AI_GENT_REPO_REMOTE")
        .unwrap_or_else(|| "origin".to_string());
    let mut names: BTreeSet<String> = git(
        ctx,
        &["for-each-ref", "--format=%(refname:short)", "refs/heads"],
    )
    .split_whitespace()
    .map(String::from)
    .collect();
    for r in git(
        ctx,
        &[
            "for-each-ref",
            "--format=%(refname:short)",
            &format!("refs/remotes/{remote}"),
        ],
    )
    .split_whitespace()
    {
        if let Some((_, rest)) = r.split_once('/') {
            if !r.ends_with("/HEAD") {
                names.insert(rest.to_string());
            }
        }
    }
    let head = git(
        ctx,
        &[
            "symbolic-ref",
            "--short",
            &format!("refs/remotes/{remote}/HEAD"),
        ],
    );
    let default = head
        .strip_prefix(&format!("{remote}/"))
        .unwrap_or(&head)
        .to_string();
    let default = (!default.is_empty()).then_some(default);
    let mut ambiguous = Vec::new();
    let mut found_level = |level: &str, cands: &[&str]| -> Option<String> {
        let found: Vec<&str> = cands
            .iter()
            .copied()
            .filter(|c| names.contains(*c))
            .collect();
        if found.len() > 1 {
            ambiguous.push(format!("{level}: {} both exist", found.join(" and ")));
        }
        (found.len() == 1).then(|| found[0].to_string())
    };
    let integration = found_level(LEVELS[0].0, LEVELS[0].1);
    let staging = found_level(LEVELS[1].0, LEVELS[1].1);
    let mut production = found_level(LEVELS[2].0, LEVELS[2].1);
    if production.is_none() && !LEVELS[2].1.iter().any(|c| names.contains(*c)) {
        production = default.clone();
        if default.is_none() {
            ambiguous.push("production: no main/master and no remote default branch".to_string());
        }
    }
    let mut order = vec!["work branches".to_string()];
    order.extend(
        [&integration, &staging, &production]
            .into_iter()
            .flatten()
            .cloned(),
    );
    let integrated_on = integration.clone().or_else(|| production.clone());
    let keywords_close_on = default.clone().or_else(|| production.clone());
    let ask = !ambiguous.is_empty();
    Chain {
        default,
        ambiguous,
        integration,
        staging,
        production,
        order,
        integrated_on,
        keywords_close_on,
        ask,
    }
}

/// `chain --json` text (`json.dumps(chain, indent=2)`).
#[must_use]
pub fn to_json(c: &Chain) -> String {
    let o = |v: &Option<String>| J::opt_s(v.as_deref());
    dumps(
        &obj(vec![
            ("default", o(&c.default)),
            ("ambiguous", J::strs(&c.ambiguous)),
            ("integration", o(&c.integration)),
            ("staging", o(&c.staging)),
            ("production", o(&c.production)),
            ("order", J::strs(&c.order)),
            ("integrated_on", o(&c.integrated_on)),
            ("keywords_close_on", o(&c.keywords_close_on)),
            ("ask", J::Bool(c.ask)),
        ]),
        true,
        Some(2),
    )
}

/// The text form.
#[must_use]
pub fn to_text(c: &Chain) -> Vec<String> {
    let q = |v: &Option<String>| v.clone().unwrap_or_else(|| "?".to_string());
    let mut lines = vec![
        format!("chain     {}", c.order.join(" \u{2192} ")),
        format!(
            "default   {}",
            c.default
                .clone()
                .unwrap_or_else(|| "(unknown \u{2014} no remote HEAD)".to_string())
        ),
        format!(
            "integrated on   {}   (where a story's work lands; repo:sync closes items at Verified there)",
            q(&c.integrated_on)
        ),
        format!(
            "keywords close  {}   (Closes #N acts only when merged here)",
            q(&c.keywords_close_on)
        ),
    ];
    for a in &c.ambiguous {
        lines.push(format!(
            "ASK       {a} \u{2014} ask the user which one the flow uses"
        ));
    }
    lines
}
