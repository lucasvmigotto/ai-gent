//! The projection for a planned card: weighted nearest neighbours by shared tags with a back-off,
//! effort and model tiers over history, else the starting table.
use super::pyjson::{J, obj};
use super::tags::{Tags, size_of, tag_get};
use super::util::{OMap, pysum, round_i, round2};

/// Samples a match level needs before history takes over.
pub const MIN_SAMPLES: usize = 5;
const NEIGHBOUR_BONUS: f64 = 0.25;
const STACK_BONUS: f64 = 0.5;
const INFERRED_WEIGHT: f64 = 0.5;
const CONTEXT_REF: &str = "sonnet/builder";
const BIAS_MIN: usize = 5;

/// Weight of a record by how sure its attribution was.
#[must_use]
pub fn conf_weight(c: &str) -> f64 {
    match c {
        "exact" => 1.0,
        "feature" => 0.6,
        _ => 0.3,
    }
}

fn size_mult(s: &str) -> Option<f64> {
    match s {
        "S" => Some(0.6),
        "M" => Some(1.0),
        "L" => Some(1.6),
        _ => None,
    }
}

/// The older Sonnet starting table (kept for reference): (median, p75) by effort.
pub const STARTING_PREVIOUS_SONNET: [(&str, (i64, i64)); 3] = [
    ("low", (150_000, 250_000)),
    ("medium", (350_000, 550_000)),
    ("high", (600_000, 850_000)),
];

/// The starting table: billable tokens per card, (median, p75), by model and effort.
#[must_use]
pub fn starting(family: &str, effort: &str) -> (i64, i64) {
    let t: [(i64, i64); 3] = match family {
        "opus" => [(250_000, 400_000), (450_000, 650_000), (700_000, 950_000)],
        "haiku" => [(60_000, 120_000), (120_000, 200_000), (200_000, 320_000)],
        _ => [(55_000, 80_000), (128_000, 179_000), (220_000, 275_000)],
    };
    t[match effort {
        "low" => 0,
        "high" => 2,
        _ => 1,
    }]
}

fn context_prior(model: &str) -> f64 {
    match model {
        "opus" => 1.3,
        "haiku" => 0.5,
        _ => 1.0,
    }
}

/// What a record teaches about the task itself: its cost without resumes, fix rounds and flake hunts.
#[must_use]
pub fn rec_cost(r: &J) -> f64 {
    for k in ["used_clean", "used"] {
        if let Some(v) = r.get(k).filter(|v| v.truthy()) {
            return v.as_f64().unwrap_or(0.0);
        }
    }
    0.0
}

fn num_j(f: f64) -> J {
    if f.fract() == 0.0 && f.abs() < 9e15 {
        J::Int(f as i64)
    } else {
        J::Float(f)
    }
}

/// A record's tags with the size filled in from its task count.
#[must_use]
pub fn rec_tags(r: &J) -> Tags {
    let mut t: Tags = match r.get("tags") {
        Some(J::Obj(o)) => o.clone(),
        _ => vec![],
    };
    if tag_get(&t, "size").is_none() {
        if let Some(J::Arr(a)) = r.get("tasks") {
            if let Some(s) = size_of(Some(a.len())) {
                t.push(("size".into(), J::s(s)));
            }
        }
    }
    t
}

fn model_of(r: &J) -> String {
    r.get("executor_model")
        .and_then(J::as_str)
        .map_or_else(|| "None".to_string(), String::from)
}

fn role_of(r: &J) -> String {
    r.get("executor_role")
        .and_then(J::as_str)
        .map_or_else(|| "None".to_string(), String::from)
}

fn ctx_key(model: &str, role: &str) -> String {
    format!("{model}/{role}")
}

/// Execution-context factors relative to a Sonnet builder, learned from history.
#[must_use]
pub fn learn_context(records: &[J]) -> OMap<f64> {
    let mut groups: OMap<Vec<&J>> = OMap::new();
    for r in records {
        let t = rec_tags(r);
        let facets = ["kind", "surface", "novelty", "lang", "layer"];
        let vals: Vec<Option<&J>> = facets
            .iter()
            .map(|f| tag_get(&t, f).filter(|v| v.truthy()))
            .collect();
        if rec_cost(r) != 0.0 && vals.iter().all(Option::is_some) {
            let key = vals
                .iter()
                .map(|v| super::pyjson::dumps(v.unwrap_or(&J::Null), true, None))
                .collect::<Vec<_>>()
                .join("|");
            groups.entry(&key, Vec::new).push(r);
        }
    }
    let mut dev: OMap<Vec<f64>> = OMap::new();
    for g in groups.values() {
        let refs: Vec<f64> = g
            .iter()
            .filter(|r| ctx_key(&model_of(r), &role_of(r)) == CONTEXT_REF)
            .map(|r| rec_cost(r).ln())
            .collect();
        if refs.is_empty() {
            continue;
        }
        let base = pysum(&refs) / refs.len() as f64;
        for r in g {
            let k = ctx_key(&model_of(r), &role_of(r));
            if k != CONTEXT_REF {
                let d = rec_cost(r).ln() - base;
                dev.entry(&k, Vec::new).push(d);
                dev.entry(&model_of(r), Vec::new).push(d);
            }
        }
    }
    let mut out = OMap::new();
    for (k, v) in dev.iter() {
        if v.len() >= MIN_SAMPLES {
            out.set(k, (pysum(v) / v.len() as f64).exp());
        }
    }
    out
}

/// (factor, `learned` or `prior`) for a model and role.
#[must_use]
pub fn context_factor(learned: &OMap<f64>, model: &str, role: &str) -> (f64, &'static str) {
    for k in [ctx_key(model, role), model.to_string()] {
        if let Some(v) = learned.get(&k) {
            return (*v, "learned");
        }
    }
    let k = ctx_key(model, role);
    (
        if k == CONTEXT_REF {
            1.0
        } else {
            context_prior(model)
        },
        "prior",
    )
}

fn tag_values(v: &J) -> Vec<String> {
    match v {
        J::Arr(a) => a
            .iter()
            .map(|x| {
                x.as_str()
                    .map_or_else(|| super::pyjson::dumps(x, true, None), String::from)
            })
            .collect(),
        J::Str(s) => vec![s.clone()],
        other => vec![super::pyjson::dumps(other, true, None)],
    }
}

/// (facet, value) to (used/projected geometric mean, records), for tags with enough records.
fn bias_factors(records: &[J]) -> OMap<(f64, usize)> {
    let mut acc: OMap<Vec<f64>> = OMap::new();
    for r in records {
        let proj = r
            .get("projected")
            .filter(|p| p.truthy())
            .and_then(J::as_f64);
        let cost = rec_cost(r);
        if let (Some(p), true) = (proj, cost != 0.0) {
            let ratio = (cost / p).ln();
            for (f, v) in rec_tags(r) {
                for x in tag_values(&v) {
                    acc.entry(&format!("{f}\u{0}{x}"), Vec::new).push(ratio);
                }
            }
        }
    }
    let mut out = OMap::new();
    for (k, v) in acc.iter() {
        if v.len() >= BIAS_MIN {
            out.set(k, ((pysum(v) / v.len() as f64).exp(), v.len()));
        }
    }
    out
}

/// The first value whose cumulative weight reaches `q` of the total.
#[must_use]
pub fn wquantile(pairs: &[(f64, f64)], q: f64) -> f64 {
    let mut p = pairs.to_vec();
    p.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    let total = pysum(&p.iter().map(|x| x.1).collect::<Vec<_>>());
    let mut acc = 0.0;
    for (v, w) in &p {
        acc += w;
        if acc >= q * total - 1e-9 {
            return *v;
        }
    }
    p.last().map_or(0.0, |x| x.0)
}

fn jaccard(a: &[String], b: &[String]) -> f64 {
    let sa: std::collections::BTreeSet<&String> = a.iter().collect();
    let sb: std::collections::BTreeSet<&String> = b.iter().collect();
    let union = sa.union(&sb).count();
    if union == 0 {
        0.0
    } else {
        sa.intersection(&sb).count() as f64 / union as f64
    }
}

type Level = &'static [&'static str];
const LEVELS: [Level; 7] = [
    &[
        "kind",
        "surface",
        "novelty",
        "lang",
        "layer",
        "verification",
        "size",
        "+stack",
    ],
    &[
        "kind",
        "surface",
        "novelty",
        "lang",
        "layer",
        "verification",
        "+stack",
    ],
    &[
        "kind",
        "surface",
        "novelty",
        "lang",
        "layer",
        "verification",
    ],
    &["kind", "surface", "novelty", "lang", "layer"],
    &["kind", "surface", "novelty", "lang"],
    &["kind", "surface", "lang"],
    &["kind", "lang"],
];
const FULL_FACETS: [&str; 7] = [
    "kind",
    "surface",
    "novelty",
    "lang",
    "layer",
    "verification",
    "size",
];

/// A projection and how it was reached.
#[derive(Debug, Clone)]
pub struct Est {
    /// Weighted median.
    pub median: f64,
    /// Weighted 75th percentile.
    pub p75: f64,
    /// Records used.
    pub n: usize,
    /// `tags`, `history` or `starting table`.
    pub basis: &'static str,
    /// The facets that matched, or the history tier.
    pub tier: Option<String>,
    /// Match level (tags only).
    pub level: usize,
    /// high, medium or low.
    pub confidence_label: Option<&'static str>,
    /// Records by confidence.
    pub mix: Vec<(String, i64)>,
    /// (factor, how) of the execution context (tags only).
    pub context: (f64, &'static str),
    /// (factor, which tags) of the bias correction (tags only).
    pub bias: (f64, Vec<String>),
}

impl Est {
    /// The projection as a JSON object, in the Python key order.
    #[must_use]
    pub fn to_j(&self) -> Vec<(String, J)> {
        let mix = J::Obj(
            self.mix
                .iter()
                .map(|(k, v)| (k.clone(), J::Int(*v)))
                .collect(),
        );
        let label = self.confidence_label.map_or(J::Null, J::s);
        let tier = J::opt_s(self.tier.as_deref());
        let mut v: Vec<(&str, J)> = vec![
            ("median", num_j(self.median)),
            ("p75", num_j(self.p75)),
            ("n", J::Int(self.n as i64)),
            ("basis", J::s(self.basis)),
        ];
        if self.basis == "tags" {
            {
                v.push(("tier", tier));
                v.push(("level", J::Int(self.level as i64)));
                v.push(("confidence_label", label));
                v.push(("mix", mix));
                v.push((
                    "context",
                    J::Arr(vec![J::Float(round2(self.context.0)), J::s(self.context.1)]),
                ));
                v.push((
                    "bias",
                    obj(vec![
                        ("factor", J::Float(round2(self.bias.0))),
                        ("from", J::strs(&self.bias.1)),
                    ]),
                ));
            }
        } else {
            v.push(("tier", tier));
            v.push(("mix", mix));
            v.push(("confidence_label", label));
        }
        v.into_iter().map(|(k, x)| (k.to_string(), x)).collect()
    }
}

fn mix_of<'a>(it: impl Iterator<Item = &'a J>) -> Vec<(String, i64)> {
    let mut mix: Vec<(String, i64)> = Vec::new();
    for r in it {
        let c = r.get("confidence").and_then(J::as_str).unwrap_or("agent");
        match mix.iter_mut().find(|(k, _)| k == c) {
            Some(e) => e.1 += 1,
            None => mix.push((c.to_string(), 1)),
        }
    }
    mix
}

fn estimate_by_tags(records: &[J], family: &str, role: &str, tags: &Tags) -> Option<Est> {
    let q: Tags = tags.iter().filter(|(_, v)| v.truthy()).cloned().collect();
    let learned = learn_context(records);
    let (qfac, qhow) = context_factor(&learned, family, role);
    let mut seen: Vec<(Vec<&str>, bool)> = Vec::new();
    let rtags: Vec<Tags> = records.iter().map(rec_tags).collect();
    for (idx, level) in LEVELS.iter().enumerate() {
        let facets: Vec<&str> = level
            .iter()
            .copied()
            .filter(|f| *f != "+stack" && tag_get(&q, f).is_some())
            .collect();
        let use_stack = level.contains(&"+stack") && tag_get(&q, "stack").is_some();
        if facets.is_empty() || seen.contains(&(facets.clone(), use_stack)) {
            continue;
        }
        seen.push((facets.clone(), use_stack));
        let dropped: Vec<&str> = FULL_FACETS
            .iter()
            .copied()
            .filter(|f| tag_get(&q, f).is_some() && !facets.contains(f))
            .collect();
        let mut pairs: Vec<(f64, f64)> = Vec::new();
        let mut used_recs: Vec<&J> = Vec::new();
        for (r, t) in records.iter().zip(&rtags) {
            if rec_cost(r) == 0.0 || facets.iter().any(|f| tag_get(t, f) != tag_get(&q, f)) {
                continue;
            }
            let mut w = conf_weight(r.get("confidence").and_then(J::as_str).unwrap_or(""));
            if r.get("tags_source").and_then(J::as_str) == Some("inferred") {
                w *= INFERRED_WEIGHT;
            }
            w *= 1.0
                + NEIGHBOUR_BONUS
                    * dropped
                        .iter()
                        .filter(|f| tag_get(t, f) == tag_get(&q, f))
                        .count() as f64;
            if use_stack {
                let a = tag_get(t, "stack").map(tag_values).unwrap_or_default();
                let b = tag_get(&q, "stack").map(tag_values).unwrap_or_default();
                w *= 1.0 + STACK_BONUS * jaccard(&a, &b);
            }
            let mut v = rec_cost(r) / context_factor(&learned, &model_of(r), &role_of(r)).0;
            if !facets.contains(&"size") {
                let qs = tag_get(&q, "size").and_then(J::as_str).and_then(size_mult);
                let ts = tag_get(t, "size").and_then(J::as_str).and_then(size_mult);
                if let (Some(qs), Some(ts)) = (qs, ts) {
                    v *= qs / ts;
                }
            }
            pairs.push((v, w));
            used_recs.push(r);
        }
        if pairs.len() < MIN_SAMPLES {
            continue;
        }
        let med = wquantile(&pairs, 0.5) * qfac;
        let p75 = wquantile(&pairs, 0.75) * qfac;
        let bias = bias_factors(records);
        let mut used: Vec<(String, (f64, usize))> = Vec::new();
        for (f, v) in &q {
            for x in tag_values(v) {
                if let Some(b) = bias.get(&format!("{f}\u{0}{x}")) {
                    used.push((format!("{f}={x}"), *b));
                }
            }
        }
        let mut corr = 1.0;
        if !used.is_empty() {
            let logs: Vec<f64> = used.iter().map(|(_, b)| b.0.ln()).collect();
            corr = (pysum(&logs) / used.len() as f64).exp().clamp(0.5, 2.0);
        }
        let label = if idx <= 1 {
            "high"
        } else if idx <= 4 {
            "medium"
        } else {
            "low"
        };
        let mut tier = facets.join("+");
        if use_stack {
            tier.push_str("+stack");
        }
        return Some(Est {
            median: round_i(med * corr) as f64,
            p75: round_i(p75 * corr) as f64,
            n: pairs.len(),
            basis: "tags",
            tier: Some(tier),
            level: idx,
            confidence_label: Some(label),
            mix: mix_of(used_recs.into_iter()),
            context: (qfac, qhow),
            bias: (
                corr,
                used.iter()
                    .map(|(k, b)| format!("{k} x{:.2} (n={})", b.0, b.1))
                    .collect(),
            ),
        });
    }
    None
}

/// The projection for a planned card: the weighted median of similar records (p75 as the upper
/// bound). With tags: nearest neighbours by shared tags, backing off until `MIN_SAMPLES` records
/// agree; otherwise (or when even that fails) effort and model tiers; else the starting table.
#[must_use]
pub fn estimate_card(
    records: &[J],
    family: &str,
    effort: Option<&str>,
    layer: Option<&str>,
    tasks: Option<usize>,
    tags: Option<&Tags>,
    role: Option<&str>,
) -> Est {
    let tags = tags.filter(|t| !t.is_empty());
    let mut layer = layer.map(String::from);
    let mut tasks = tasks.filter(|&n| n != 0);
    if let Some(t) = tags {
        if let Some(got) = estimate_by_tags(records, family, role.unwrap_or("builder"), t) {
            return got;
        }
        if layer.is_none() {
            layer = tag_get(t, "layer").and_then(J::as_str).map(String::from);
        }
        tasks = tasks.filter(|_| true);
    }
    let size: Option<String> = size_of(tasks).map(String::from).or_else(|| {
        tags.and_then(|t| tag_get(t, "size"))
            .and_then(J::as_str)
            .map(String::from)
    });
    let crit_all: [(&str, Vec<(&str, Option<String>)>); 4] = [
        (
            "layer+effort+model+size",
            vec![
                ("layer", layer.clone()),
                ("effort", effort.map(String::from)),
                ("size", size.clone()),
            ],
        ),
        (
            "effort+model+size",
            vec![("effort", effort.map(String::from)), ("size", size.clone())],
        ),
        ("effort+model", vec![("effort", effort.map(String::from))]),
        ("model", vec![]),
    ];
    let mut seen: Vec<Vec<(String, String)>> = Vec::new();
    let mut best = 0;
    for (name, crit) in &crit_all {
        let mut key: Vec<(String, String)> = crit
            .iter()
            .filter_map(|(k, v)| v.clone().map(|v| ((*k).to_string(), v)))
            .collect();
        key.sort();
        if seen.contains(&key) {
            continue;
        }
        seen.push(key.clone());
        let sel: Vec<&J> = records
            .iter()
            .filter(|r| {
                rec_cost(r) != 0.0
                    && r.get("executor_model").and_then(J::as_str) == Some(family)
                    && key.iter().all(|(k, v)| {
                        if k == "size" {
                            tag_get(&rec_tags(r), "size").and_then(J::as_str) == Some(v.as_str())
                        } else {
                            r.get(k).and_then(J::as_str) == Some(v.as_str())
                        }
                    })
            })
            .collect();
        best = best.max(sel.len());
        if sel.len() >= MIN_SAMPLES {
            let pairs: Vec<(f64, f64)> = sel
                .iter()
                .map(|r| {
                    (
                        rec_cost(r),
                        conf_weight(r.get("confidence").and_then(J::as_str).unwrap_or("")),
                    )
                })
                .collect();
            return Est {
                median: wquantile(&pairs, 0.5),
                p75: wquantile(&pairs, 0.75),
                n: sel.len(),
                basis: "history",
                tier: Some((*name).to_string()),
                level: 0,
                confidence_label: if tags.is_some() { Some("low") } else { None },
                mix: mix_of(sel.into_iter()),
                context: (1.0, "prior"),
                bias: (1.0, vec![]),
            };
        }
    }
    let eff = effort
        .filter(|e| ["low", "medium", "high"].contains(e))
        .unwrap_or("medium");
    let (med, p75) = starting(family, eff);
    let mult = size.as_deref().and_then(size_mult).unwrap_or(1.0);
    Est {
        median: round_i(med as f64 * mult) as f64,
        p75: round_i(p75 as f64 * mult) as f64,
        n: best,
        basis: "starting table",
        tier: None,
        level: 0,
        confidence_label: Some("low"),
        mix: vec![],
        context: (1.0, "prior"),
        bias: (1.0, vec![]),
    }
}

/// Share of spend that went to process events, and where that number came from.
#[must_use]
pub fn event_rate(history: &[J]) -> (f64, &'static str) {
    let recs: Vec<&J> = history
        .iter()
        .filter(|r| r.get("used").is_some_and(J::truthy))
        .collect();
    if recs.len() >= MIN_SAMPLES && recs.iter().any(|r| r.get("used_clean").is_some()) {
        let total: f64 = recs
            .iter()
            .map(|r| r.get("used").and_then(J::as_f64).unwrap_or(0.0))
            .sum();
        let clean: f64 = recs.iter().map(|r| rec_cost(r)).sum();
        return ((total - clean) / total, "history");
    }
    (0.08, "prior")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::pyjson::parse;

    fn rec(i: usize, used: i64, conf: &str, model: &str, effort: &str, layer: &str) -> J {
        parse(&format!(
            r#"{{"id": "p:{i}", "source": "measured", "confidence": "{conf}", "project": "p", "feature": "001-x", "card": "c{i}", "tasks": ["T001", "T002"], "layer": "{layer}", "effort": "{effort}", "executor_model": "{model}", "used": {used}, "date": "2026-10-2{}", "agents": ["ag{i}"]}}"#,
            i % 9
        ))
        .unwrap()
    }

    fn est(
        recs: &[J],
        family: &str,
        effort: &str,
        layer: Option<&str>,
        tasks: Option<usize>,
    ) -> Est {
        estimate_card(recs, family, Some(effort), layer, tasks, None, None)
    }

    #[test]
    fn few_records_fall_back_to_the_starting_table() {
        let recs: Vec<J> = (0..4)
            .map(|i| rec(i, 999, "exact", "sonnet", "medium", "backend"))
            .collect();
        let e = est(&recs, "sonnet", "medium", Some("backend"), Some(4));
        assert_eq!(
            (e.basis, e.median, e.p75, e.n),
            ("starting table", 128_000.0, 179_000.0, 4)
        );
        assert_eq!(
            est(&[], "opus", "high", None, Some(9)).median,
            (700_000.0f64 * 1.6).round()
        );
    }

    #[test]
    fn history_takes_over_with_median_p75_and_widens_the_match() {
        let recs: Vec<J> = [100, 200, 300, 400, 500]
            .iter()
            .enumerate()
            .map(|(i, v)| rec(i, *v, "exact", "sonnet", "medium", "backend"))
            .collect();
        let e = est(&recs, "sonnet", "medium", Some("backend"), Some(2));
        assert_eq!(
            (e.basis, e.median, e.p75, e.n, e.tier.as_deref()),
            ("history", 300.0, 400.0, 5, Some("layer+effort+model+size"))
        );
        let mixed: Vec<J> = [100, 200, 300, 400, 500]
            .iter()
            .enumerate()
            .map(|(i, v)| {
                rec(
                    i,
                    *v,
                    "exact",
                    "sonnet",
                    "medium",
                    if i < 3 { "backend" } else { "qa" },
                )
            })
            .collect();
        assert_eq!(
            est(&mixed, "sonnet", "medium", Some("backend"), Some(2))
                .tier
                .as_deref(),
            Some("effort+model+size")
        );
        assert_eq!(
            est(&recs, "opus", "medium", None, None).basis,
            "starting table"
        );
    }

    #[test]
    fn confidence_weights_pull_the_median_to_exact_records() {
        let mut recs: Vec<J> = (0..2)
            .map(|i| rec(i, 1000 + i as i64, "exact", "sonnet", "medium", "backend"))
            .collect();
        recs.extend((0..3).map(|i| {
            rec(
                i + 10,
                100 + i as i64,
                "agent",
                "sonnet",
                "medium",
                "backend",
            )
        }));
        let e = est(&recs, "sonnet", "medium", Some("backend"), Some(2));
        assert_eq!(e.mix, [("exact".to_string(), 2), ("agent".to_string(), 3)]);
        assert_eq!(e.median, 1000.0);
    }

    #[test]
    fn previous_sonnet_defaults_are_kept_and_the_measured_ones_documented() {
        assert_eq!(STARTING_PREVIOUS_SONNET[1].1, (350_000, 550_000));
        let doc = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../shared/token-tracking.md"
        ))
        .unwrap();
        for key in ["128k (179k)", "350k (550k)"] {
            assert!(doc.contains(key), "{key}");
        }
    }

    fn trec(i: usize, used: i64, over: &[(&str, J)]) -> J {
        let mut r = rec(i, used, "exact", "sonnet", "medium", "backend");
        let mut tags = parse(r#"{"kind": "extend", "layer": "backend", "surface": "protocol", "lang": "rust", "novelty": "known-pattern", "verification": "integration", "size": "M"}"#).unwrap();
        for (k, v) in over {
            tags.set(k, v.clone());
        }
        r.set("tags", tags);
        r.set("executor_role", J::s("builder"));
        r
    }

    fn q() -> Tags {
        match parse(r#"{"kind": "extend", "layer": "backend", "surface": "protocol", "lang": "rust", "novelty": "known-pattern", "verification": "integration", "size": "M"}"#).unwrap() {
            J::Obj(o) => o,
            _ => unreachable!(),
        }
    }

    fn tag_est(recs: &[J], model: &str, role: &str) -> Est {
        estimate_card(
            recs,
            model,
            Some("medium"),
            None,
            None,
            Some(&q()),
            Some(role),
        )
    }

    #[test]
    fn full_match_is_high_confidence_and_backoff_drops_facets() {
        let recs: Vec<J> = [100, 200, 300, 400, 500]
            .iter()
            .enumerate()
            .map(|(i, v)| trec(i, *v, &[]))
            .collect();
        let e = tag_est(&recs, "sonnet", "builder");
        assert_eq!(
            (e.basis, e.level, e.confidence_label, e.n, e.median, e.p75),
            ("tags", 0, Some("high"), 5, 300.0, 400.0)
        );
        let recs: Vec<J> = (0..5)
            .map(|i| trec(i, 100 * (i as i64 + 1), &[("size", J::s("S"))]))
            .collect();
        let e = tag_est(&recs, "sonnet", "builder");
        assert_eq!(
            (e.level, e.tier.as_deref()),
            (1, Some("kind+surface+novelty+lang+layer+verification"))
        );
        assert_eq!(e.median, (300.0f64 / 0.6).round());
        let recs: Vec<J> = (0..5)
            .map(|i| trec(i, 100, &[("verification", J::s("unit"))]))
            .collect();
        assert_eq!(
            tag_est(&recs, "sonnet", "builder").tier.as_deref(),
            Some("kind+surface+novelty+lang+layer")
        );
        let recs: Vec<J> = (0..5)
            .map(|i| {
                trec(
                    i,
                    100,
                    &[("layer", J::s("tui")), ("verification", J::s("unit"))],
                )
            })
            .collect();
        let e = tag_est(&recs, "sonnet", "builder");
        assert_eq!(
            (e.tier.as_deref(), e.confidence_label),
            (Some("kind+surface+novelty+lang"), Some("medium"))
        );
    }

    #[test]
    fn too_few_matches_fall_back_and_inferred_weigh_half() {
        let recs: Vec<J> = (0..9)
            .map(|i| trec(i, 100, &[("kind", J::s("fix")), ("lang", J::s("go"))]))
            .collect();
        let e = tag_est(&recs, "sonnet", "builder");
        assert_eq!((e.basis, e.confidence_label), ("history", Some("low")));
        assert_eq!(tag_est(&recs, "opus", "builder").basis, "starting table");
        let mut recs: Vec<J> = (0..2).map(|i| trec(i, 100, &[])).collect();
        recs.extend((0..4).map(|i| {
            let mut r = trec(i + 9, 900, &[]);
            r.set("tags_source", J::s("inferred"));
            r
        }));
        assert_eq!(tag_est(&recs, "sonnet", "builder").median, 100.0);
    }

    #[test]
    fn context_bias_and_events() {
        let mut recs = Vec::new();
        for g in 0..5 {
            let kind = J::s(["extend", "fix", "refactor", "docs", "ci"][g]);
            recs.push(trec(
                g,
                100,
                &[("surface", J::s("storage")), ("kind", kind.clone())],
            ));
            let mut o = trec(g + 20, 200, &[("surface", J::s("storage")), ("kind", kind)]);
            o.set("executor_model", J::s("opus"));
            o.set("executor_role", J::s("thinker"));
            recs.push(o);
        }
        let learned = learn_context(&recs);
        assert!((learned.get("opus/thinker").copied().unwrap() - 2.0).abs() < 1e-9);
        assert_eq!(context_factor(&learned, "opus", "thinker").1, "learned");
        assert_eq!(
            context_factor(&OMap::new(), "opus", "thinker"),
            (1.3, "prior")
        );
        let base: Vec<J> = (0..5).map(|i| trec(i, 100, &[])).collect();
        assert_eq!(tag_est(&base, "opus", "thinker").median, 130.0);
        let mut biased: Vec<J> = (0..5).map(|i| trec(i, 200, &[])).collect();
        for r in &mut biased {
            r.set("projected", J::Int(100));
        }
        let e = tag_est(&biased, "sonnet", "builder");
        assert_eq!((e.bias.0, e.median), (2.0, 400.0));
        let mut four = biased[..4].to_vec();
        four.push(trec(9, 200, &[]));
        assert_eq!(tag_est(&four, "sonnet", "builder").bias.0, 1.0);
        let mut ev: Vec<J> = (0..5).map(|i| trec(i, 500, &[])).collect();
        for r in &mut ev {
            r.set("used_clean", J::Int(100));
        }
        assert_eq!(tag_est(&ev, "sonnet", "builder").median, 100.0);
    }

    #[test]
    fn stack_overlap_adds_weight_without_excluding() {
        let st = |v: &[&str]| J::strs(v);
        let mut recs: Vec<J> = (0..3)
            .map(|i| trec(i, 100, &[("stack", st(&["react"]))]))
            .collect();
        recs.extend((0..3).map(|i| trec(i + 9, 900, &[("stack", st(&["tokio", "clap"]))])));
        let mut query = q();
        query.push(("stack".into(), st(&["tokio", "clap"])));
        let e = estimate_card(
            &recs,
            "sonnet",
            Some("medium"),
            None,
            None,
            Some(&query),
            Some("builder"),
        );
        assert_eq!((e.n, e.median), (6, 900.0));
        assert_eq!(tag_est(&recs, "sonnet", "builder").n, 6);
    }
}
