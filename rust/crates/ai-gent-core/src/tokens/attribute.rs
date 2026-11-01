//! Which feature, card and task ids an agent's usage belongs to, grouping of agents per card, and
//! the history record of a group.
use super::pyjson::{J, obj};
use super::specs::Feature;
use super::tags::{RepoInfo, Tags, infer_tags, tag_get};
use super::transcripts::{Row, count_in_order, max_count};
use super::util::{OMap, expand_ids, is_word, splitlines};

/// How sure an attribution is.
pub const CONFIDENCE_ORDER: [&str; 3] = ["agent", "feature", "exact"];

fn rank(c: &str) -> usize {
    CONFIDENCE_ORDER.iter().position(|x| *x == c).unwrap_or(0)
}

/// Which feature, card and tasks, and why.
#[derive(Debug, Clone)]
pub struct Attr {
    /// The feature folder name.
    pub feature: Option<String>,
    /// The card id.
    pub card: Option<String>,
    /// Task ids.
    pub tasks: Vec<String>,
    /// `exact`, `feature` or `agent`.
    pub confidence: &'static str,
    /// What matched.
    pub how: String,
}

impl Default for Attr {
    fn default() -> Self {
        Attr {
            feature: None,
            card: None,
            tasks: vec![],
            confidence: "agent",
            how: String::new(),
        }
    }
}

fn norm(s: &str) -> String {
    s.trim()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Runs of exactly three ASCII digits whose neighbours are not word characters or dots.
fn three_digit_runs(c: &[char]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < c.len() {
        if c[i].is_ascii_digit() {
            let s = i;
            while i < c.len() && c[i].is_ascii_digit() {
                i += 1;
            }
            let bad = |x: char| is_word(x) || x == '.';
            if i - s == 3 && (s == 0 || !bad(c[s - 1])) && (i >= c.len() || !bad(c[i])) {
                out.push((s, i));
            }
        } else {
            i += 1;
        }
    }
    out
}

fn feature_mentions(text: &str, by_num: &OMap<String>) -> Vec<String> {
    let c: Vec<char> = text.chars().collect();
    let mut seen: Vec<String> = Vec::new();
    for (a, b) in three_digit_runs(&c) {
        let n: String = c[a..b].iter().collect();
        if !seen.contains(&n) {
            seen.push(n);
        }
    }
    seen.into_iter()
        .filter(|n| by_num.contains_key(n))
        .collect()
}

fn prompt_feature(prompt: &str, by_num: &OMap<String>) -> Option<String> {
    let c: Vec<char> = prompt.chars().collect();
    let mut counts: Vec<(String, i64)> = Vec::new();
    let mut bump = |n: String| {
        if by_num.contains_key(&n) {
            match counts.iter_mut().find(|(k, _)| *k == n) {
                Some(e) => e.1 += 1,
                None => counts.push((n, 1)),
            }
        }
    };
    // specs/(\d{3})-
    let key: Vec<char> = "specs/".chars().collect();
    let mut i = 0;
    while i + key.len() + 4 <= c.len() {
        if c[i..i + key.len()] == key[..]
            && c[i + 6..i + 9].iter().all(char::is_ascii_digit)
            && c[i + 9] == '-'
        {
            bump(c[i + 6..i + 9].iter().collect());
            i += 10;
        } else {
            i += 1;
        }
    }
    // (?<![\w.])(\d{3})-[a-z]
    for (a, b) in three_digit_runs(&c) {
        if c.get(b) == Some(&'-') && c.get(b + 1).is_some_and(char::is_ascii_lowercase) {
            bump(c[a..b].iter().collect());
        }
    }
    counts.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    match counts.as_slice() {
        [] => None,
        [one] => Some(one.0.clone()),
        [a, b, ..] => (a.1 > b.1).then(|| a.0.clone()),
    }
}

/// `(?<![\w-])cid(?![\w-])` found in `desc`.
fn names_card(desc: &[char], cid: &str) -> bool {
    let k: Vec<char> = cid.chars().collect();
    if k.is_empty() || k.len() > desc.len() {
        return false;
    }
    let bad = |x: char| is_word(x) || x == '-';
    (0..=desc.len() - k.len()).any(|i| {
        desc[i..i + k.len()] == k[..]
            && (i == 0 || !bad(desc[i - 1]))
            && desc.get(i + k.len()).is_none_or(|&x| !bad(x))
    })
}

/// `re.match(rf"\W*(?:T\d+\W+)*{num}\b", desc)`.
fn leads_with(desc: &str, num: &str) -> bool {
    let c: Vec<char> = desc.chars().collect();
    let n: Vec<char> = num.chars().collect();
    let mut i = 0;
    loop {
        while i < c.len() && !is_word(c[i]) {
            i += 1;
        }
        if c[i..].starts_with(&n) && c.get(i + n.len()).is_none_or(|&x| !is_word(x)) {
            return true;
        }
        if c.get(i) == Some(&'T') {
            let mut j = i + 1;
            while j < c.len() && c[j].is_ascii_digit() {
                j += 1;
            }
            if j > i + 1 && j < c.len() && !is_word(c[j]) {
                i = j;
                continue;
            }
        }
        return false;
    }
}

/// Which feature, card and task ids an agent's usage belongs to, and how sure that is.
#[must_use]
pub fn attribute(row: &Row, feats: &OMap<Feature>) -> Attr {
    let mut res = Attr::default();
    if feats.is_empty() {
        return res;
    }
    let (desc, prompt) = (row.desc.as_str(), row.prompt.as_str());
    let mut by_num: OMap<String> = OMap::new();
    for f in feats.values() {
        by_num.set(&f.num, f.name.clone());
    }
    let hit = |f: &Feature, cid: &str, how: &str| {
        let c = f.cards.get(cid);
        let ctasks = c.map(|c| c.tasks.clone()).unwrap_or_default();
        let named: Vec<String> = expand_ids(desc)
            .into_iter()
            .filter(|t| ctasks.contains(t))
            .collect();
        Attr {
            feature: Some(f.name.clone()),
            card: Some(cid.to_string()),
            tasks: if named.is_empty() { ctasks } else { named },
            confidence: "exact",
            how: how.to_string(),
        }
    };
    for f in feats.values() {
        for (cid, c) in f.cards.iter() {
            if c.dispatch
                .as_deref()
                .is_some_and(|d| !d.is_empty() && norm(d) == norm(desc))
            {
                return hit(f, cid, "dispatch description");
            }
        }
    }
    let mut ids_by_len: Vec<(&String, &Feature)> = feats
        .values()
        .flat_map(|f| f.cards.keys().map(move |c| (c, f)))
        .collect();
    ids_by_len.sort_by_key(|(cid, _)| std::cmp::Reverse(cid.chars().count()));
    let dchars: Vec<char> = desc.chars().collect();
    for (cid, f) in ids_by_len {
        if names_card(&dchars, cid) {
            return hit(f, cid, "card id in description");
        }
    }

    let mut ids = expand_ids(desc);
    if ids.is_empty() {
        let goal = splitlines(prompt)
            .into_iter()
            .find(|l| l.trim_start().to_lowercase().starts_with("goal"))
            .unwrap_or("");
        ids = expand_ids(goal);
    }
    let mention = feature_mentions(desc, &by_num);
    let mut num = mention.first().cloned();
    let mut how = "feature named in description".to_string();
    if mention.len() > 1 {
        if leads_with(desc, &mention[0]) {
            how = "first of several features, leading the description".into();
        } else {
            return res;
        }
    }
    if num.is_none() && !ids.is_empty() {
        let owners: Vec<&Feature> = feats
            .values()
            .filter(|f| ids.iter().all(|t| f.tasks.contains_key(t)))
            .collect();
        if owners.len() == 1 {
            num = Some(owners[0].num.clone());
            how = "task ids unique to one feature".into();
        }
    }
    if num.is_none() {
        num = prompt_feature(prompt, &by_num);
        how = "feature named in prompt".into();
    }
    let Some(num) = num else { return res };
    let Some(fname) = by_num.get(&num) else {
        return res;
    };
    let Some(f) = feats.get(fname) else {
        return res;
    };
    let known: Vec<String> = ids
        .iter()
        .filter(|t| f.tasks.contains_key(t))
        .cloned()
        .collect();
    res.feature = Some(f.name.clone());
    res.confidence = "feature";
    res.how = how.clone();
    if !known.is_empty() {
        res.confidence = "exact";
        res.how = format!("{how} + task ids");
        let inside: Vec<&String> = f
            .cards
            .iter()
            .filter(|(_, c)| known.iter().all(|t| c.tasks.contains(t)))
            .map(|(cid, _)| cid)
            .collect();
        if inside.len() == 1 {
            res.card = Some(inside[0].clone());
        }
        res.tasks = known;
    }
    res
}

fn ci_starts(s: &str, p: &str) -> bool {
    s.to_lowercase().starts_with(p)
}

fn word_runs(d: &str) -> Vec<(usize, usize, String)> {
    let c: Vec<char> = d.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < c.len() {
        if is_word(c[i]) {
            let s = i;
            while i < c.len() && is_word(c[i]) {
                i += 1;
            }
            out.push((s, i, c[s..i].iter().collect()));
        } else {
            i += 1;
        }
    }
    out
}

/// Work that is process overhead rather than the task itself: `reviews`, `fix_rounds` or
/// `ci_flake`; `None` for the task's own work.
#[must_use]
pub fn process_kind(desc: &str) -> Option<&'static str> {
    let lead = desc.trim_start();
    if ci_starts(lead, "review") {
        return Some("reviews");
    }
    let d = desc.to_lowercase();
    let r = word_runs(&d);
    let dc: Vec<char> = d.chars().collect();
    let flake = r.iter().any(|(_, _, w)| w.starts_with("flak"))
        || r.windows(2).any(|w| {
            w[0].1 + 1 == w[1].0
                && dc[w[0].1] == ' '
                && ((w[0].2 == "ci"
                    && (["watch", "fix"].contains(&w[1].2.as_str()) || w[1].2.starts_with("fail")))
                    || (w[0].2 == "red" && w[1].2 == "run"))
        });
    if flake {
        return Some("ci_flake");
    }
    let first = word_runs(&lead.to_lowercase()).into_iter().next();
    let leads =
        first.is_some_and(|(s, _, w)| s == 0 && ["fix", "address", "rework"].contains(&w.as_str()));
    if leads || r.iter().any(|(_, _, w)| w == "finding" || w == "findings") {
        return Some("fix_rounds");
    }
    None
}

/// `T001` or `T001..T003`.
#[must_use]
pub fn tasks_key(ids: &[String]) -> String {
    if ids.len() == 1 {
        ids[0].clone()
    } else {
        format!("{}..{}", ids[0], ids[ids.len() - 1])
    }
}

/// The agents that worked on one card (or task set).
#[derive(Debug, Clone)]
pub struct Group {
    /// The feature folder.
    pub feature: Option<String>,
    /// The card id.
    pub card: Option<String>,
    /// Sorted task ids.
    pub tasks: Vec<String>,
    /// The best confidence of its agents.
    pub confidence: &'static str,
    /// Indexes into the rows slice.
    pub rows: Vec<usize>,
    /// Billable tokens.
    pub billable: i64,
    /// Cache reads.
    pub cache_read: i64,
    /// Messages.
    pub calls: i64,
    /// Latest end.
    pub end: String,
    /// Earliest start.
    pub start: String,
    /// Files written.
    pub files: Vec<String>,
    /// Lines written.
    pub lines: i64,
    /// Model family of the lead agent.
    pub model: String,
    /// Full model id of the lead agent.
    pub model_id: Option<String>,
    /// The lead agent's description.
    pub desc: String,
    /// Process events by kind, in first-seen order.
    pub events: Vec<(String, i64)>,
    /// What the events cost.
    pub event_cost: i64,
}

fn bump(ev: &mut Vec<(String, i64)>, k: &str, n: i64) {
    match ev.iter_mut().find(|(x, _)| x == k) {
        Some(e) => e.1 += n,
        None => ev.push((k.to_string(), n)),
    }
}

/// Group agents that worked on the same card (or task set); other agents stay one group each.
#[must_use]
pub fn build_groups(rows: &[Row], feats: &OMap<Feature>) -> Vec<Group> {
    let mut groups: OMap<Group> = OMap::new();
    for (i, r) in rows.iter().enumerate() {
        let a = attribute(r, feats);
        let key = if a.confidence == "agent" || a.feature.is_none() {
            format!("agent\u{0}{}", r.aid)
        } else if let Some(c) = &a.card {
            format!("{}\u{0}card\u{0}{c}", a.feature.as_deref().unwrap_or(""))
        } else if !a.tasks.is_empty() {
            format!(
                "{}\u{0}tasks\u{0}{}",
                a.feature.as_deref().unwrap_or(""),
                tasks_key(&a.tasks)
            )
        } else {
            format!(
                "{}\u{0}agent\u{0}{}",
                a.feature.as_deref().unwrap_or(""),
                r.aid
            )
        };
        let g = groups.entry(&key, || Group {
            feature: a.feature.clone(),
            card: a.card.clone(),
            tasks: vec![],
            confidence: a.confidence,
            rows: vec![],
            billable: 0,
            cache_read: 0,
            calls: 0,
            end: String::new(),
            start: String::new(),
            files: vec![],
            lines: 0,
            model: String::new(),
            model_id: None,
            desc: String::new(),
            events: vec![],
            event_cost: 0,
        });
        g.rows.push(i);
        g.tasks.extend(a.tasks.iter().cloned());
        g.tasks.sort();
        g.tasks.dedup();
        if rank(a.confidence) > rank(g.confidence) {
            g.confidence = a.confidence;
        }
    }
    let mut out: Vec<Group> = Vec::new();
    for (_, mut g) in groups.iter().map(|(k, v)| (k.clone(), v.clone())) {
        let rs: Vec<&Row> = g.rows.iter().map(|&i| &rows[i]).collect();
        g.billable = rs.iter().map(|r| r.s.billable).sum();
        g.cache_read = rs.iter().map(|r| r.s.cache_r).sum();
        g.calls = rs.iter().map(|r| r.s.calls).sum();
        g.end = rs.iter().map(|r| r.s.end.clone()).max().unwrap_or_default();
        g.start = rs
            .iter()
            .map(|r| r.s.start.clone())
            .min()
            .unwrap_or_default();
        let mut files: Vec<String> = rs.iter().flat_map(|r| r.files.iter().cloned()).collect();
        files.sort();
        files.dedup();
        g.files = files;
        g.lines = rs.iter().map(|r| r.lines).sum();
        let kinds: Vec<Option<&'static str>> = rs.iter().map(|r| process_kind(&r.desc)).collect();
        let pick = |pred: &dyn Fn(Option<&'static str>) -> bool| -> Vec<usize> {
            (0..rs.len()).filter(|&i| pred(kinds[i])).collect()
        };
        let mut main = pick(&|k| k.is_none());
        if main.is_empty() {
            main = pick(&|k| k != Some("fix_rounds"));
        }
        if main.is_empty() {
            main = (0..rs.len()).collect();
        }
        let mut lead = main[0];
        for &i in &main[1..] {
            if rs[i].s.billable > rs[lead].s.billable {
                lead = i;
            }
        }
        g.model = rs[lead].family.clone();
        let ids = count_in_order(rs[lead].msgs.iter().map(|m| m.model.as_str()));
        g.model_id = max_count(&ids).map(String::from);
        g.desc = rs[lead].desc.clone();
        let mut ev: Vec<(String, i64)> = Vec::new();
        let mut extra_cost = 0;
        for (i, r) in rs.iter().enumerate() {
            let k = kinds[i];
            let side = i != lead && matches!(k, Some("fix_rounds" | "ci_flake"));
            if side {
                bump(&mut ev, k.unwrap_or_default(), 1);
                extra_cost += r.s.billable;
            } else if k == Some("reviews") {
                bump(&mut ev, "reviews", 1);
            }
            if r.resumed != 0 {
                bump(&mut ev, "resumed", r.resumed);
                if !side {
                    extra_cost += r.resume_cost;
                }
            }
        }
        g.events = ev;
        g.event_cost = extra_cost.min(g.billable);
        out.push(g);
    }
    out.sort_by(|a, b| a.start.cmp(&b.start));
    out
}

/// backend, frontend, qa or general.
#[must_use]
pub fn layer_of(g: &Group, feats: &OMap<Feature>) -> String {
    if let Some(f) = g.feature.as_deref().and_then(|n| feats.get(n)) {
        for t in &g.tasks {
            if let Some(l) = f.tasks.get(t).and_then(|t| t.layer.clone()) {
                return l;
            }
        }
    }
    let probe = format!(
        "{} {}",
        g.card.as_deref().unwrap_or(""),
        g.desc.to_lowercase()
    );
    for (word, layer) in [
        ("frontend", "frontend"),
        ("qa", "qa"),
        ("e2e", "qa"),
        ("-core", "backend"),
        ("backend", "backend"),
    ] {
        if probe.contains(word) {
            return layer.to_string();
        }
    }
    "general".into()
}

fn effort_rank(e: &str) -> Option<usize> {
    ["low", "medium", "high"].iter().position(|x| *x == e)
}

/// low, medium or high from the card or its tasks.
#[must_use]
pub fn effort_of(g: &Group, feats: &OMap<Feature>) -> Option<String> {
    let f = g.feature.as_deref().and_then(|n| feats.get(n))?;
    if let Some(e) = g
        .card
        .as_deref()
        .and_then(|c| f.cards.get(c))
        .and_then(|c| c.effort.as_deref())
    {
        if effort_rank(e).is_some() {
            return Some(e.to_string());
        }
    }
    let mut best: Option<&str> = None;
    for t in &g.tasks {
        if let Some(e) = f
            .tasks
            .get(t)
            .and_then(|t| t.effort.as_deref())
            .filter(|e| effort_rank(e).is_some())
        {
            if best.is_none_or(|b| effort_rank(e) > effort_rank(b)) {
                best = Some(e);
            }
        }
    }
    best.map(String::from)
}

fn first_task<'a>(g: &Group, f: &'a Feature) -> Option<&'a super::specs::Task> {
    g.tasks
        .iter()
        .filter_map(|t| f.tasks.get(t))
        .min_by_key(|t| t.line)
}

/// The projection recorded for the group's card, else its first task.
#[must_use]
pub fn projected_of(g: &Group, feats: &OMap<Feature>) -> Option<i64> {
    let f = g.feature.as_deref().and_then(|n| feats.get(n))?;
    if let Some(p) = g
        .card
        .as_deref()
        .and_then(|c| f.cards.get(c))
        .and_then(|c| c.projected)
    {
        return Some(p);
    }
    first_task(g, f).and_then(|t| t.projected)
}

/// The tags the plan gave the group's card, else its first task.
#[must_use]
pub fn planned_tags(g: &Group, feats: &OMap<Feature>) -> Tags {
    let Some(f) = g.feature.as_deref().and_then(|n| feats.get(n)) else {
        return vec![];
    };
    if let Some(c) = g.card.as_deref().and_then(|c| f.cards.get(c)) {
        if !c.tags.is_empty() {
            return c.tags.clone();
        }
    }
    first_task(g, f).map(|t| t.tags.clone()).unwrap_or_default()
}

fn role_of_family(f: &str) -> &'static str {
    match f {
        "opus" => "thinker",
        "haiku" => "scout",
        _ => "builder",
    }
}

/// A history record for a group.
#[must_use]
pub fn make_record(
    project: &str,
    g: &Group,
    rows: &[Row],
    feats: &OMap<Feature>,
    coord_card: Option<f64>,
    source: &str,
    repo: Option<&RepoInfo>,
) -> J {
    let f = g.feature.as_deref().and_then(|n| feats.get(n));
    let c = f.and_then(|f| g.card.as_deref().and_then(|c| f.cards.get(c)));
    let first_aid = &rows[g.rows[0]].aid;
    let mut ident = if g.feature.is_some()
        && g.confidence != "agent"
        && (g.card.is_some() || !g.tasks.is_empty())
    {
        let key = g.card.clone().unwrap_or_else(|| tasks_key(&g.tasks));
        format!("{project}:{}:{key}", g.feature.as_deref().unwrap_or(""))
    } else {
        format!("{project}:agent:{first_aid}")
    };
    if source == "seed" {
        ident = format!("seed:{ident}");
    }
    let layer = layer_of(g, feats);
    let planned = planned_tags(g, feats);
    let guess = infer_tags(
        &g.desc,
        Some(&layer),
        g.tasks.len(),
        &g.files,
        g.lines,
        repo,
    );
    let mut tags: Tags = guess.clone();
    for (k, v) in &planned {
        if let Some(slot) = tags.iter_mut().find(|(x, _)| x == k) {
            slot.1 = v.clone();
        } else {
            tags.push((k.clone(), v.clone()));
        }
    }
    let tags_source = if planned.is_empty() {
        "inferred"
    } else if guess.iter().any(|(k, _)| tag_get(&planned, k).is_none()) {
        "planned+inferred"
    } else {
        "planned"
    };
    let role = c
        .and_then(|c| c.executor.clone())
        .filter(|e| !e.is_empty())
        .unwrap_or_else(|| role_of_family(&g.model).to_string());
    let mut agents: Vec<&str> = g.rows.iter().map(|&i| rows[i].aid.as_str()).collect();
    agents.sort_unstable();
    obj(vec![
        ("id", J::s(ident)),
        ("source", J::s(source)),
        ("confidence", J::s(g.confidence)),
        ("project", J::s(project)),
        ("feature", J::opt_s(g.feature.as_deref())),
        ("card", J::opt_s(g.card.as_deref())),
        ("tasks", J::strs(&g.tasks)),
        ("layer", J::s(layer)),
        ("effort", J::opt_s(effort_of(g, feats).as_deref())),
        ("tags", J::Obj(tags)),
        ("tags_source", J::s(tags_source)),
        ("executor_role", J::s(role)),
        ("executor_model", J::s(&g.model)),
        ("model_id", J::opt_s(g.model_id.as_deref())),
        ("files", J::Int(g.files.len() as i64)),
        ("lines", J::Int(g.lines)),
        ("projected", J::opt_i(projected_of(g, feats))),
        ("used", J::Int(g.billable)),
        ("used_clean", J::Int(g.billable - g.event_cost)),
        (
            "events",
            J::Obj(
                g.events
                    .iter()
                    .map(|(k, v)| (k.clone(), J::Int(*v)))
                    .collect(),
            ),
        ),
        ("cache_read", J::Int(g.cache_read)),
        (
            "coordinator",
            J::opt_i(coord_card.map(super::util::round_i)),
        ),
        ("calls", J::Int(g.calls)),
        ("agents", J::strs(&agents)),
        ("desc", J::s(super::util::head(&g.desc, 120))),
        ("date", J::s(super::util::head(&g.end, 10))),
    ])
}
