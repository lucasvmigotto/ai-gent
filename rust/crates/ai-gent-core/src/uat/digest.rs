//! Register snapshots and the status report (`ai-gent uat digest`, `data-model.md` "Derived values",
//! `contracts/reports.md`; FR-019 to FR-021, FR-075, FR-091, FR-095, FR-098, FR-099).
//!
//! A snapshot is what the agent saved from `ArtifactData` reads: one or more pages, each a file, merged
//! here by union (a later page wins per document). Everything in it is third-party data: it is quoted
//! in reports, never interpreted.
use super::model::{Roadmap, Step};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;

/// One mark on a step.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Mark {
    /// `OK` or `NOK`.
    pub r: String,
    /// The observation.
    pub obs: String,
    /// When.
    pub at: String,
    /// The step revision the validator saw.
    pub rev: u32,
    /// Links.
    pub links: Vec<String>,
}

/// One suggested improvement.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Improvement {
    /// The text.
    pub txt: String,
    /// When.
    pub at: String,
    /// The step revision.
    pub rev: u32,
}

/// A validator's document, `marks/{uid}`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct MarkDoc {
    /// Marks by step id.
    pub steps: BTreeMap<String, Mark>,
    /// Improvements by step id.
    pub improvements: BTreeMap<String, Improvement>,
}

/// `roster/{uid}`: the owner's placement of an account.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RosterEntry {
    /// The validator key.
    pub validator: String,
    /// The assigned role key.
    pub role: String,
}

/// `proposals/{uid}`: a role the validator proposed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Proposal {
    /// The proposed role key.
    pub role: String,
    /// The note.
    pub note: String,
    /// When.
    pub at: String,
    /// `pending`, `approved` or `rejected`.
    pub decision: String,
}

/// `triage/{step}~{uid}`: the agent's classification, written on the owner's request.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Triage {
    /// The class (FR-072).
    pub class: String,
    /// The reply drafted for the validator.
    pub reply: String,
    /// `open`, `answered`, `fixed-in-test` or `decision`.
    pub status: String,
}

/// The register at a moment, as saved by the agent.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Snapshot {
    /// When the agent read it, ISO-8601 UTC.
    pub at: String,
    /// `marks`.
    pub marks: BTreeMap<String, MarkDoc>,
    /// `roster`.
    pub roster: BTreeMap<String, RosterEntry>,
    /// `proposals`.
    pub proposals: BTreeMap<String, Proposal>,
    /// `triage`.
    pub triage: BTreeMap<String, Triage>,
    /// `meta/roadmap.version`.
    pub meta_version: Option<u32>,
    /// Display names the agent resolved, by uid.
    pub names: BTreeMap<String, String>,
    /// Accounts whose profile no longer resolves (FR-099).
    pub left: Vec<String>,
}

impl Snapshot {
    /// Parse one snapshot page.
    ///
    /// # Errors
    /// When the text is not JSON of this shape.
    pub fn parse(text: &str) -> Result<Self, String> {
        serde_json::from_str(text).map_err(|e| format!("not a snapshot: {e}"))
    }

    /// Merge pages: the union of their documents, a later page winning per key; the latest `at`.
    #[must_use]
    pub fn merge(pages: Vec<Snapshot>) -> Snapshot {
        let mut out = Snapshot::default();
        for p in pages {
            if p.at > out.at {
                out.at = p.at;
            }
            out.marks.extend(p.marks);
            out.roster.extend(p.roster);
            out.proposals.extend(p.proposals);
            out.triage.extend(p.triage);
            out.names.extend(p.names);
            out.meta_version = out.meta_version.max(p.meta_version);
            for l in p.left {
                if !out.left.contains(&l) {
                    out.left.push(l);
                }
            }
        }
        out
    }
}

/// `2026-11-19T14:05:00Z` as `2026-11-19 14:05`.
#[must_use]
pub fn stamp(at: &str) -> String {
    if at.len() >= 16 && at.as_bytes().get(10) == Some(&b'T') {
        format!("{} {}", &at[..10], &at[11..16])
    } else {
        at.to_string()
    }
}

fn hhmm(at: &str) -> String {
    stamp(at).split(' ').nth(1).unwrap_or("").to_string()
}

/// Progress of one script.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Progress {
    /// Script id.
    pub id: String,
    /// Title.
    pub title: String,
    /// Active steps.
    pub steps: usize,
    /// Steps whose result is OK.
    pub ok: usize,
    /// Steps whose result is NOK.
    pub nok: usize,
    /// Steps with no current mark.
    pub unmarked: usize,
    /// Marks older than their step's revision.
    pub stale: usize,
    /// Distinct accounts that marked a step of the script.
    pub markers: usize,
}

/// A NOK.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Nok {
    /// The step.
    pub step: String,
    /// Who, by label.
    pub who: String,
    /// When.
    pub at: String,
    /// The observation, quoted.
    pub obs: String,
    /// The class, if the agent triaged it.
    pub class: String,
    /// The drafted reply.
    pub reply: String,
    /// The triage status.
    pub status: String,
}

/// An improvement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Idea {
    /// The step.
    pub step: String,
    /// Who.
    pub who: String,
    /// The text, quoted.
    pub txt: String,
}

/// A pending role proposal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Pending {
    /// Who.
    pub who: String,
    /// The assigned role label, or `role not assigned`.
    pub assigned: String,
    /// The proposed role label.
    pub proposed: String,
    /// The note, quoted.
    pub note: String,
}

/// Follow-up settings as saved in `follow-up.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct FollowUp {
    /// Periodic follow-up is on.
    pub on: bool,
    /// The interval.
    pub every_minutes: u32,
    /// When it started.
    pub started_at: String,
    /// When it expires.
    pub expires_at: String,
    /// The last check.
    pub last_check: String,
}

/// Everything a status report says.
#[derive(Debug, Clone, Serialize)]
pub struct Status {
    /// The time of the snapshot the report is built from (FR-095).
    pub last_check: String,
    /// The time of the snapshot compared with, if any.
    pub previous_check: Option<String>,
    /// Nothing changed since the previous snapshot.
    pub no_news: bool,
    /// The follow-up line: the next check, an expired timer, or `on request`.
    pub timer: String,
    /// Progress per script.
    pub scripts: Vec<Progress>,
    /// NOKs not in the previous snapshot.
    pub new_noks: Vec<Nok>,
    /// Improvements not in the previous snapshot.
    pub new_improvements: Vec<Idea>,
    /// Role proposals waiting for the owner.
    pub proposals_pending: Vec<Pending>,
    /// Steps marked only by roles other than their target.
    pub gap_role: Vec<String>,
    /// Steps whose only current marks come from people who left.
    pub gap_left: Vec<String>,
    /// Validators with no write-check mark yet.
    pub write_checks_missing: Vec<String>,
    /// Steps with a mark older than the step's revision.
    pub stale_steps: Vec<String>,
    /// Accounts that marked and have no roster entry.
    pub unassigned: Vec<String>,
    /// Change requests and decisions waiting for the owner.
    pub decisions: Vec<String>,
}

/// The derived view of a roadmap and one merged snapshot.
pub struct View<'a> {
    r: &'a Roadmap,
    s: &'a Snapshot,
}

impl<'a> View<'a> {
    /// Build the view.
    #[must_use]
    pub fn new(r: &'a Roadmap, s: &'a Snapshot) -> Self {
        Self { r, s }
    }

    /// The label to show the owner for an account.
    #[must_use]
    pub fn label(&self, uid: &str) -> String {
        if let Some(v) = self
            .s
            .roster
            .get(uid)
            .and_then(|e| self.r.validators.iter().find(|v| v.key == e.validator))
        {
            return v.label.clone();
        }
        match self.s.names.get(uid).filter(|n| !n.is_empty()) {
            Some(n) => n.clone(),
            None => "an unlisted account".to_string(),
        }
    }

    fn role_label(&self, key: &str) -> String {
        self.r
            .roles
            .iter()
            .find(|x| x.key == key)
            .map_or_else(|| "role not assigned".to_string(), |x| x.label.clone())
    }

    /// The assigned role of an account; none without a roster entry (a pending proposal changes nothing).
    #[must_use]
    pub fn assigned(&self, uid: &str) -> Option<&str> {
        self.s
            .roster
            .get(uid)
            .map(|e| e.role.as_str())
            .filter(|x| !x.is_empty())
    }

    fn current(&self, step: &Step, uid: &str) -> Option<&Mark> {
        self.s
            .marks
            .get(uid)?
            .steps
            .get(&step.id)
            .filter(|m| m.rev >= step.rev)
    }

    /// NOK if any current mark is NOK; OK if any current mark; else `unmarked`.
    #[must_use]
    pub fn result(&self, step: &Step) -> &'static str {
        let mut ok = false;
        for uid in self.s.marks.keys() {
            match self.current(step, uid).map(|m| m.r.as_str()) {
                Some("NOK") => return "NOK",
                Some("OK") => ok = true,
                _ => {}
            }
        }
        if ok { "OK" } else { "unmarked" }
    }

    /// A current mark by an account whose assigned role is the step's target role.
    #[must_use]
    pub fn covered(&self, step: &Step) -> bool {
        self.s
            .marks
            .keys()
            .any(|u| self.current(step, u).is_some() && self.assigned(u) == Some(step.who.as_str()))
    }

    fn marked_by(&self, step: &Step) -> Vec<&String> {
        self.s
            .marks
            .iter()
            .filter(|(_, d)| d.steps.contains_key(&step.id))
            .map(|(u, _)| u)
            .collect()
    }

    fn left(&self, uid: &str) -> bool {
        self.s.left.iter().any(|l| l == uid)
    }

    /// Progress per script.
    #[must_use]
    pub fn progress(&self) -> Vec<Progress> {
        self.r
            .scripts
            .iter()
            .map(|sc| {
                let mut p = Progress {
                    id: sc.id.clone(),
                    title: sc.title.clone(),
                    steps: 0,
                    ok: 0,
                    nok: 0,
                    unmarked: 0,
                    stale: 0,
                    markers: 0,
                };
                let mut who = BTreeSet::new();
                for st in sc.steps.iter().filter(|x| !x.retired()) {
                    p.steps += 1;
                    match self.result(st) {
                        "OK" => p.ok += 1,
                        "NOK" => p.nok += 1,
                        _ => p.unmarked += 1,
                    }
                    for (uid, d) in &self.s.marks {
                        if let Some(m) = d.steps.get(&st.id) {
                            who.insert(uid.as_str());
                            if m.rev < st.rev {
                                p.stale += 1;
                            }
                        }
                    }
                }
                p.markers = who.len();
                p
            })
            .collect()
    }

    /// Every current NOK, newest first.
    #[must_use]
    pub fn noks(&self) -> Vec<(Nok, String)> {
        let mut out = Vec::new();
        for (sc, st) in self.r.active_steps() {
            let _ = sc;
            for uid in self.s.marks.keys() {
                if let Some(m) = self.current(st, uid).filter(|m| m.r == "NOK") {
                    let t = self.s.triage.get(&format!("{}~{uid}", st.id));
                    out.push((
                        Nok {
                            step: st.id.clone(),
                            who: self.label(uid),
                            at: m.at.clone(),
                            obs: m.obs.clone(),
                            class: t.map(|x| x.class.clone()).unwrap_or_default(),
                            reply: t.map(|x| x.reply.clone()).unwrap_or_default(),
                            status: t.map(|x| x.status.clone()).unwrap_or_default(),
                        },
                        uid.clone(),
                    ));
                }
            }
        }
        out.sort_by(|a, b| b.0.at.cmp(&a.0.at).then(a.0.step.cmp(&b.0.step)));
        out
    }

    /// Every improvement.
    #[must_use]
    pub fn ideas(&self) -> Vec<(Idea, String, Improvement)> {
        let mut out = Vec::new();
        for (uid, d) in &self.s.marks {
            for (step, i) in &d.improvements {
                if !i.txt.trim().is_empty() {
                    out.push((
                        Idea {
                            step: step.clone(),
                            who: self.label(uid),
                            txt: i.txt.clone(),
                        },
                        uid.clone(),
                        i.clone(),
                    ));
                }
            }
        }
        out
    }

    /// Role proposals still pending.
    #[must_use]
    pub fn pending(&self) -> Vec<Pending> {
        self.s
            .proposals
            .iter()
            .filter(|(_, p)| p.decision == "pending")
            .map(|(uid, p)| Pending {
                who: self.label(uid),
                assigned: self.role_label(self.assigned(uid).unwrap_or("")),
                proposed: self.role_label(&p.role),
                note: p.note.clone(),
            })
            .collect()
    }

    /// Steps marked, but not by their target role.
    #[must_use]
    pub fn gap_role(&self) -> Vec<String> {
        self.r
            .active_steps()
            .filter(|(_, st)| !self.marked_by(st).is_empty() && !self.covered(st))
            .map(|(_, st)| st.id.clone())
            .collect()
    }

    /// Steps whose only current marks come from people who left (FR-099).
    #[must_use]
    pub fn gap_left(&self) -> Vec<String> {
        self.r
            .active_steps()
            .filter(|(_, st)| {
                let cur: Vec<_> = self
                    .s
                    .marks
                    .keys()
                    .filter(|u| self.current(st, u).is_some())
                    .collect();
                !cur.is_empty() && cur.iter().all(|u| self.left(u))
            })
            .map(|(_, st)| st.id.clone())
            .collect()
    }

    /// Validators with no mark on the write-check step.
    #[must_use]
    pub fn write_checks_missing(&self) -> Vec<String> {
        let checks: Vec<&Step> = self
            .r
            .active_steps()
            .filter(|(_, st)| st.kind == "write-check")
            .map(|(_, st)| st)
            .collect();
        self.r
            .validators
            .iter()
            .filter(|v| {
                !self.s.roster.iter().any(|(uid, e)| {
                    e.validator == v.key
                        && checks.iter().any(|st| {
                            self.s
                                .marks
                                .get(uid)
                                .is_some_and(|d| d.steps.contains_key(&st.id))
                        })
                })
            })
            .map(|v| v.label.clone())
            .collect()
    }

    /// Steps with a mark older than their revision.
    #[must_use]
    pub fn stale_steps(&self) -> Vec<String> {
        self.r
            .active_steps()
            .filter(|(_, st)| {
                self.s
                    .marks
                    .values()
                    .any(|d| d.steps.get(&st.id).is_some_and(|m| m.rev < st.rev))
            })
            .map(|(_, st)| st.id.clone())
            .collect()
    }

    /// Accounts that marked and are not on the roster.
    #[must_use]
    pub fn unassigned(&self) -> Vec<String> {
        self.s
            .marks
            .keys()
            .filter(|u| self.assigned(u).is_none())
            .map(|u| self.label(u))
            .collect()
    }

    /// Steps covered per assigned role.
    #[must_use]
    pub fn coverage(&self) -> Vec<(String, usize, usize)> {
        self.r
            .roles
            .iter()
            .map(|role| {
                let targeted: Vec<&Step> = self
                    .r
                    .active_steps()
                    .filter(|(_, st)| st.who == role.key)
                    .map(|(_, st)| st)
                    .collect();
                let done = targeted.iter().filter(|st| self.covered(st)).count();
                (role.label.clone(), done, targeted.len())
            })
            .collect()
    }
}

/// `(uid, step)` pairs.
type Pairs<'a> = BTreeSet<(&'a str, &'a str)>;

/// What changed since the previous snapshot, by entry (a different value counts as new).
fn fresh<'a>(now: &'a Snapshot, before: Option<&Snapshot>) -> (Pairs<'a>, Pairs<'a>) {
    let mut marks = BTreeSet::new();
    let mut ideas = BTreeSet::new();
    for (uid, d) in &now.marks {
        let old = before.and_then(|b| b.marks.get(uid));
        for (step, m) in &d.steps {
            if old.and_then(|o| o.steps.get(step)) != Some(m) {
                marks.insert((uid.as_str(), step.as_str()));
            }
        }
        for (step, i) in &d.improvements {
            if old.and_then(|o| o.improvements.get(step)) != Some(i) {
                ideas.insert((uid.as_str(), step.as_str()));
            }
        }
    }
    (marks, ideas)
}

fn timer_line(at: &str, f: Option<&FollowUp>) -> String {
    let Some(f) = f.filter(|f| f.on) else {
        return "on request".to_string();
    };
    if !f.expires_at.is_empty() && at >= f.expires_at.as_str() {
        return format!(
            "Timer expired on {}; checks are on request now",
            stamp(&f.expires_at)
        );
    }
    let next = hhmm(&f.last_check);
    if next.is_empty() || f.every_minutes == 0 {
        return "on request".to_string();
    }
    format!("Next check about {} minutes after {next}", f.every_minutes)
}

/// Build the status report.
#[must_use]
pub fn status(
    r: &Roadmap,
    now: &Snapshot,
    before: Option<&Snapshot>,
    follow_up: Option<&FollowUp>,
) -> Status {
    let v = View::new(r, now);
    let (new_marks, new_ideas) = fresh(now, before);
    let new_noks: Vec<Nok> = v
        .noks()
        .into_iter()
        .filter(|(n, uid)| new_marks.contains(&(uid.as_str(), n.step.as_str())))
        .map(|(n, _)| n)
        .collect();
    let new_improvements: Vec<Idea> = v
        .ideas()
        .into_iter()
        .filter(|(i, uid, _)| new_ideas.contains(&(uid.as_str(), i.step.as_str())))
        .map(|(i, _, _)| i)
        .collect();
    let pending = v.pending();
    let new_pending: Vec<Pending> = if before.is_some() {
        let old: BTreeSet<&String> = before
            .map(|b| {
                b.proposals
                    .iter()
                    .filter(|(_, p)| p.decision == "pending")
                    .map(|(u, _)| u)
                    .collect()
            })
            .unwrap_or_default();
        now.proposals
            .iter()
            .filter(|(u, p)| p.decision == "pending" && !old.contains(u))
            .map(|(u, p)| Pending {
                who: v.label(u),
                assigned: v.role_label(v.assigned(u).unwrap_or("")),
                proposed: v.role_label(&p.role),
                note: p.note.clone(),
            })
            .collect()
    } else {
        pending.clone()
    };
    let decisions: Vec<String> = now
        .triage
        .iter()
        .filter(|(_, t)| t.class == "change-request" || t.status == "decision")
        .map(|(k, t)| {
            format!(
                "{k}: {}",
                if t.class.is_empty() {
                    "decision"
                } else {
                    &t.class
                }
            )
        })
        .collect();
    let no_news = before.is_some()
        && new_noks.is_empty()
        && new_improvements.is_empty()
        && new_pending.is_empty()
        && new_marks.is_empty();
    Status {
        last_check: now.at.clone(),
        previous_check: before.map(|b| b.at.clone()),
        no_news,
        timer: timer_line(&now.at, follow_up),
        scripts: v.progress(),
        new_noks,
        new_improvements,
        proposals_pending: pending,
        gap_role: v.gap_role(),
        gap_left: v.gap_left(),
        write_checks_missing: v.write_checks_missing(),
        stale_steps: v.stale_steps(),
        unassigned: v.unassigned(),
        decisions,
    }
}

fn quote(s: &str) -> String {
    let one: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    format!("\"{one}\"")
}

/// The status report as the owner reads it (`contracts/reports.md`).
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn text(s: &Status) -> String {
    let mut o = String::new();
    if s.no_news {
        let since = s.previous_check.as_deref().map_or(String::new(), hhmm);
        let next = if s.timer == "on request" {
            "on request".to_string()
        } else {
            s.timer.clone()
        };
        let _ = writeln!(
            o,
            "No news since {since} (last real check {}). Next check {next}.",
            hhmm(&s.last_check)
        );
        return o;
    }
    let _ = writeln!(o, "Last real check: {}", stamp(&s.last_check));
    let _ = writeln!(
        o,
        "{}",
        if s.timer == "on request" {
            "Checks are on request.".to_string()
        } else {
            s.timer.clone()
        }
    );
    let _ = writeln!(o, "\nProgress");
    for p in &s.scripts {
        let _ = writeln!(
            o,
            "- {} {}: {}/{} OK, {} NOK, {} unmarked, {} stale",
            p.id.to_uppercase(),
            p.title,
            p.ok,
            p.steps,
            p.nok,
            p.unmarked,
            p.stale
        );
    }
    if !s.new_noks.is_empty() {
        let _ = writeln!(o, "\nNew NOKs");
        for n in &s.new_noks {
            let class = if n.class.is_empty() {
                "not yet triaged"
            } else {
                &n.class
            };
            let _ = write!(
                o,
                "- {} by {} at {}: {} — class: {class}",
                n.step,
                n.who,
                stamp(&n.at),
                quote(&n.obs)
            );
            if !n.reply.is_empty() {
                let _ = write!(o, " — suggested reply: {}", quote(&n.reply));
            }
            o.push('\n');
        }
    }
    if !s.new_improvements.is_empty() {
        let _ = writeln!(o, "\nNew improvements");
        for i in &s.new_improvements {
            let _ = writeln!(o, "- {} by {}: {}", i.step, i.who, quote(&i.txt));
        }
    }
    if !s.proposals_pending.is_empty() {
        let _ = writeln!(o, "\nRole proposals pending");
        for p in &s.proposals_pending {
            let _ = writeln!(
                o,
                "- {}: assigned {}, proposes {}: {}",
                p.who,
                p.assigned,
                p.proposed,
                quote(&p.note)
            );
        }
    }
    let gaps = !(s.gap_role.is_empty() && s.gap_left.is_empty() && s.unassigned.is_empty());
    if gaps {
        let _ = writeln!(o, "\nCoverage gaps");
        if !s.gap_role.is_empty() {
            let _ = writeln!(
                o,
                "- marked only by another role: {}",
                s.gap_role.join(", ")
            );
        }
        if !s.gap_left.is_empty() {
            let _ = writeln!(
                o,
                "- marked only by people who left: {}",
                s.gap_left.join(", ")
            );
        }
        if !s.unassigned.is_empty() {
            let _ = writeln!(
                o,
                "- accounts with no assigned role: {}",
                s.unassigned.join(", ")
            );
        }
    }
    if !s.write_checks_missing.is_empty() {
        let _ = writeln!(o, "\nWrite checks missing");
        let _ = writeln!(
            o,
            "- {} (re-check their sharing level)",
            s.write_checks_missing.join(", ")
        );
    }
    if !s.stale_steps.is_empty() {
        let _ = writeln!(o, "\nStale marks");
        let _ = writeln!(
            o,
            "- steps rewritten after marks: {}",
            s.stale_steps.join(", ")
        );
    }
    if !s.decisions.is_empty() {
        let _ = writeln!(o, "\nFor you to decide");
        for d in &s.decisions {
            let _ = writeln!(o, "- {d}");
        }
    }
    o
}
