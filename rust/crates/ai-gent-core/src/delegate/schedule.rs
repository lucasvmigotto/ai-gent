//! The scheduler (spec 041, FR-004, FR-004a, FR-006): from a task list it forms tracks that touch
//! disjoint paths, classifies each task as simple or judgment, assigns each track to a worker, and
//! admits tracks against the worktree cap and the disk floor counted together.
//!
//! Pure: it reads no disk and starts nothing. The caller passes the pool numbers (`wt.py resources`
//! and `.ai-gent/parallel.yaml` supply them) and the agents the project allows (`delegate.yaml`).
//! It never overrides the floor or the cap.
use super::brief::protected_path;
use super::glob;
use super::{Error, Res};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// What a task is, as the task list declares it. Unknown or missing means `Other`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Kind {
    /// Research or a survey.
    Research,
    /// A documentation draft.
    Docs,
    /// Fixtures or test data.
    Fixtures,
    /// A small script.
    Script,
    /// A mechanical edit (rename, reformat, move).
    Mechanical,
    /// A first-pass review.
    Review,
    /// Rust development.
    Rust,
    /// Design work, or anything cross-cutting.
    Design,
    /// Not declared.
    #[default]
    Other,
}

/// What a task needs that no delegate may have (FR-004).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Needs {
    /// Credentials.
    pub credentials: bool,
    /// Database or container tools.
    pub containers: bool,
    /// An owner decision.
    pub owner_decision: bool,
}

/// One task of the list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Task {
    /// The task id (for example `T013`).
    pub id: String,
    /// The kind, if declared.
    #[serde(default)]
    pub kind: Kind,
    /// The paths or globs the task writes.
    #[serde(default)]
    pub paths: Vec<String>,
    /// What it needs.
    #[serde(default)]
    pub needs: Needs,
    /// Ids of tasks that must land first.
    #[serde(default)]
    pub depends: Vec<String>,
}

/// Simple or judgment (FR-004a).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Class {
    /// Simple and well bounded.
    Simple,
    /// Real design judgment.
    Judgment,
}

/// Who runs a track.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "worker", content = "agent", rename_all = "lowercase")]
pub enum Worker {
    /// Claude itself.
    Claude,
    /// A delegate agent, by adapter name.
    Delegate(String),
}

/// A track: tasks that share paths or depend on each other, run in order by one worker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Track {
    /// The track id, from its first declared task (`t013`), independent of execution order.
    pub id: String,
    /// Task ids in dependency order, with input order breaking ties.
    pub tasks: Vec<String>,
    /// The track's class: judgment if any task is.
    pub class: Class,
    /// The worker.
    #[serde(flatten)]
    pub worker: Worker,
    /// Why it is not a delegate's, when it is not.
    pub reason: Option<String>,
}

/// The pool, counted for Claude's and delegated tracks together (FR-006).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pool {
    /// The worktree cap (`max_volumes`).
    pub cap: usize,
    /// Tracks already live.
    pub live: usize,
    /// Free bytes in the container store.
    pub free: f64,
    /// The disk floor in bytes.
    pub floor: f64,
    /// Bytes one more track is estimated to take.
    pub estimate: f64,
}

impl Pool {
    /// How many more tracks may start: the cap and the disk floor both bind, whichever is lower.
    #[must_use]
    pub fn slots(&self) -> usize {
        let by_cap = self.cap.saturating_sub(self.live);
        let by_disk = if self.estimate <= 0.0 {
            usize::MAX
        } else {
            let fit = ((self.free - self.floor) / self.estimate).floor();
            if fit < 0.0 {
                0
            } else {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let fit = fit as usize;
                fit.saturating_sub(self.live)
            }
        };
        by_cap.min(by_disk)
    }
}

/// A track held back and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Deferred {
    /// The track id.
    pub track: String,
    /// `cap` or `disk-floor`.
    pub because: &'static str,
}

/// The plan.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Plan {
    /// Every track, in list order, with its class and assignment (started or deferred).
    pub tracks: Vec<Track>,
    /// Tracks to start now, in list order.
    pub start: Vec<Track>,
    /// Tracks that wait for a slot.
    pub deferred: Vec<Deferred>,
    /// Free slots counted.
    pub slots: usize,
}

fn rust_path(p: &str) -> bool {
    let p = glob::norm(p);
    std::path::Path::new(&p)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("rs"))
        || p.ends_with("Cargo.toml")
        || p.ends_with("Cargo.lock")
        || p.starts_with("rust/crates/")
}

/// Classify a task (FR-004a). Judgment: Rust, design, undeclared kinds, and any path that is Rust
/// code or a protected path (hooks, skills, plugins, shared references).
#[must_use]
pub fn classify(t: &Task) -> Class {
    let kind_simple = matches!(
        t.kind,
        Kind::Research
            | Kind::Docs
            | Kind::Fixtures
            | Kind::Script
            | Kind::Mechanical
            | Kind::Review
    );
    let path_judgment = t.paths.iter().any(|p| rust_path(p) || protected_path(p));
    if kind_simple && !path_judgment {
        Class::Simple
    } else {
        Class::Judgment
    }
}

fn why_not_delegate(t: &Task) -> Option<String> {
    let n = &t.needs;
    if n.credentials {
        return Some(format!("{} needs credentials (FR-004)", t.id));
    }
    if n.containers {
        return Some(format!(
            "{} needs database or container tools (FR-004)",
            t.id
        ));
    }
    if n.owner_decision {
        return Some(format!("{} needs an owner decision (FR-004)", t.id));
    }
    if t.paths.iter().any(|p| protected_path(p)) {
        return Some(format!("{} edits a protected path (FR-004)", t.id));
    }
    if classify(t) == Class::Judgment {
        return Some(format!("{} is a judgment task (FR-004a)", t.id));
    }
    None
}

fn overlap(a: &str, b: &str) -> bool {
    let pa = glob::literal_prefix(a);
    let pb = glob::literal_prefix(b);
    let (pa, pb) = (pa.trim_end_matches('/'), pb.trim_end_matches('/'));
    let under = |long: &str, short: &str| {
        short.is_empty() || long == short || long.starts_with(&format!("{short}/"))
    };
    under(pa, pb) || under(pb, pa)
}

fn share(a: &Task, b: &Task) -> bool {
    a.paths
        .iter()
        .any(|x| b.paths.iter().any(|y| overlap(x, y)))
}

// The caller has already checked unique ids and known dependencies. Kahn's algorithm keeps
// unrelated tasks in input order; duplicate dependency edges count only once.
fn dependency_order(tasks: &[Task]) -> Res<Vec<usize>> {
    let indexes: BTreeMap<&str, usize> = tasks
        .iter()
        .enumerate()
        .map(|(i, t)| (t.id.as_str(), i))
        .collect();
    let dependencies: Vec<BTreeSet<usize>> = tasks
        .iter()
        .map(|t| t.depends.iter().map(|d| indexes[d.as_str()]).collect())
        .collect();
    let mut remaining: Vec<usize> = dependencies.iter().map(BTreeSet::len).collect();
    let mut consumers = vec![Vec::new(); tasks.len()];
    for (i, deps) in dependencies.iter().enumerate() {
        for &dep in deps {
            consumers[dep].push(i);
        }
    }
    let mut ready: BTreeSet<usize> = remaining
        .iter()
        .enumerate()
        .filter_map(|(i, &n)| (n == 0).then_some(i))
        .collect();
    let mut ordered = Vec::with_capacity(tasks.len());
    while let Some(i) = ready.pop_first() {
        ordered.push(i);
        for &consumer in &consumers[i] {
            remaining[consumer] -= 1;
            if remaining[consumer] == 0 {
                ready.insert(consumer);
            }
        }
    }
    if let Some(mut current) = remaining.iter().position(|&n| n > 0) {
        // A blocked node still has a blocked dependency. Follow those edges until one repeats
        // to report an actual cycle rather than also naming its downstream consumers.
        let mut positions = vec![None; tasks.len()];
        let mut path: Vec<usize> = Vec::new();
        loop {
            if let Some(start) = positions[current] {
                let mut cycle: Vec<&str> = path[start..]
                    .iter()
                    .map(|&i| tasks[i].id.as_str())
                    .collect();
                cycle.push(tasks[current].id.as_str());
                return Err(Error::Invalid(format!(
                    "dependency cycle: {}",
                    cycle.join(" -> ")
                )));
            }
            positions[current] = Some(path.len());
            path.push(current);
            current = *dependencies[current]
                .iter()
                .find(|&&dep| remaining[dep] > 0)
                .expect("a blocked task has a blocked dependency");
        }
    }
    Ok(ordered)
}

/// Split the list into tracks: tasks that share a path, or depend on each other, join one track.
/// Tasks within each track are ordered by their dependencies, with input order breaking ties.
///
/// # Errors
/// [`Error::Invalid`] for duplicate ids, unknown dependencies, dependency cycles or missing paths.
pub fn tracks(tasks: &[Task]) -> Res<Vec<Vec<usize>>> {
    for (i, t) in tasks.iter().enumerate() {
        if tasks[..i].iter().any(|o| o.id == t.id) {
            return Err(Error::Invalid(format!("duplicate task id {}", t.id)));
        }
        if t.paths.is_empty() {
            return Err(Error::Invalid(format!("task {} declares no paths", t.id)));
        }
        if let Some(d) = t
            .depends
            .iter()
            .find(|d| !tasks.iter().any(|o| &o.id == *d))
        {
            return Err(Error::Invalid(format!(
                "task {} depends on unknown {d}",
                t.id
            )));
        }
    }
    let mut rank = vec![0; tasks.len()];
    for (position, i) in dependency_order(tasks)?.into_iter().enumerate() {
        rank[i] = position;
    }
    let mut group: Vec<usize> = (0..tasks.len()).collect();
    let find = |g: &Vec<usize>, mut i: usize| {
        while g[i] != i {
            i = g[i];
        }
        i
    };
    for i in 0..tasks.len() {
        for j in 0..i {
            let dep =
                tasks[i].depends.contains(&tasks[j].id) || tasks[j].depends.contains(&tasks[i].id);
            if dep || share(&tasks[i], &tasks[j]) {
                let (a, b) = (find(&group, i), find(&group, j));
                let (lo, hi) = (a.min(b), a.max(b));
                group[hi] = lo;
            }
        }
    }
    let mut out: Vec<Vec<usize>> = Vec::new();
    let mut roots: Vec<usize> = Vec::new();
    for i in 0..tasks.len() {
        let r = find(&group, i);
        if let Some(k) = roots.iter().position(|&x| x == r) {
            out[k].push(i);
        } else {
            roots.push(r);
            out.push(vec![i]);
        }
    }
    for members in &mut out {
        members.sort_by_key(|&i| rank[i]);
    }
    Ok(out)
}

/// Plan the list: tracks, assignment (FR-004, FR-004a) and admission against the pool (FR-006).
/// A track goes to the first allowed delegate only when every task in it may be delegated.
///
/// # Errors
/// As [`tracks`].
pub fn plan(tasks: &[Task], allowed: &[String], pool: &Pool) -> Res<Plan> {
    let mut all = Vec::new();
    for g in tracks(tasks)? {
        let anchor = g
            .iter()
            .min()
            .copied()
            .ok_or_else(|| Error::Invalid("a track contains no tasks".into()))?;
        let members: Vec<&Task> = g.iter().map(|&i| &tasks[i]).collect();
        let class = if members.iter().all(|t| classify(t) == Class::Simple) {
            Class::Simple
        } else {
            Class::Judgment
        };
        let blocked = members.iter().find_map(|t| why_not_delegate(t));
        let (worker, reason) = match (blocked, allowed.first()) {
            (Some(r), _) => (Worker::Claude, Some(r)),
            (None, None) => (
                Worker::Claude,
                Some("no delegate agent is allowed".to_string()),
            ),
            (None, Some(a)) => (Worker::Delegate(a.clone()), None),
        };
        all.push(Track {
            id: tasks[anchor].id.to_lowercase(),
            tasks: members.iter().map(|t| t.id.clone()).collect(),
            class,
            worker,
            reason,
        });
    }
    let slots = pool.slots();
    let by_cap = pool.cap.saturating_sub(pool.live);
    let because = if slots < by_cap { "disk-floor" } else { "cap" };
    let deferred = all
        .iter()
        .skip(slots)
        .map(|t| Deferred {
            track: t.id.clone(),
            because,
        })
        .collect();
    let tracks = all.clone();
    all.truncate(slots);
    Ok(Plan {
        tracks,
        start: all,
        deferred,
        slots,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::delegate::tests_support::fixtures;

    const GB: f64 = 1e9;

    fn load(name: &str) -> Vec<Task> {
        let s = std::fs::read_to_string(fixtures().join("schedule").join(name)).unwrap();
        serde_json::from_str(&s).unwrap()
    }

    fn pool(cap: usize, live: usize, free_gb: f64) -> Pool {
        Pool {
            cap,
            live,
            free: free_gb * GB,
            floor: 20.0 * GB,
            estimate: 10.0 * GB,
        }
    }

    fn codex() -> Vec<String> {
        vec!["codex".to_string()]
    }

    fn ids(t: &Track) -> Vec<&str> {
        t.tasks.iter().map(String::as_str).collect()
    }

    #[test]
    fn overlapping_paths_share_a_track_and_tracks_are_disjoint() {
        let tasks = load("mixed.json");
        let g = tracks(&tasks).unwrap();
        let names: Vec<Vec<&str>> = g
            .iter()
            .map(|t| t.iter().map(|&i| tasks[i].id.as_str()).collect())
            .collect();
        assert_eq!(
            names,
            [
                vec!["A1", "A2"],
                vec!["B1"],
                vec!["C1"],
                vec!["D1"],
                vec!["E1"],
                vec!["F1"],
                vec!["G1"]
            ]
        );
        for (i, x) in g.iter().enumerate() {
            for y in &g[i + 1..] {
                for a in x {
                    for b in y {
                        assert!(!share(&tasks[*a], &tasks[*b]));
                    }
                }
            }
        }
    }

    #[test]
    fn simple_tasks_go_to_the_delegate_and_judgment_stays_with_claude() {
        let p = plan(&load("mixed.json"), &codex(), &pool(9, 0, 500.0)).unwrap();
        let w = |id: &str| p.start.iter().find(|t| t.id == id).unwrap();
        assert_eq!(w("a1").worker, Worker::Delegate("codex".into()));
        assert_eq!(w("a1").class, Class::Simple);
        assert_eq!(w("b1").worker, Worker::Delegate("codex".into()));
        for id in ["c1", "d1", "e1", "f1", "g1"] {
            assert_eq!(w(id).worker, Worker::Claude, "{id}");
            assert!(w(id).reason.is_some(), "{id}");
        }
        assert!(w("c1").reason.as_deref().unwrap().contains("credentials"));
        assert!(w("d1").reason.as_deref().unwrap().contains("container"));
        assert!(
            w("e1")
                .reason
                .as_deref()
                .unwrap()
                .contains("owner decision")
        );
        assert!(w("f1").reason.as_deref().unwrap().contains("protected"));
        assert!(w("g1").reason.as_deref().unwrap().contains("judgment"));
    }

    #[test]
    fn a_judgment_task_taints_its_whole_track() {
        let p = plan(&load("tainted.json"), &codex(), &pool(9, 0, 500.0)).unwrap();
        assert_eq!(p.start.len(), 1);
        assert_eq!(ids(&p.start[0]), ["S1", "J1"]);
        assert_eq!(p.start[0].class, Class::Judgment);
        assert_eq!(p.start[0].worker, Worker::Claude);
    }

    #[test]
    fn no_allowed_delegate_means_claude() {
        let p = plan(&load("mixed.json"), &[], &pool(9, 0, 500.0)).unwrap();
        assert!(p.start.iter().all(|t| t.worker == Worker::Claude));
    }

    #[test]
    fn the_cap_counts_live_tracks_of_every_worker() {
        let p = plan(&load("mixed.json"), &codex(), &pool(3, 1, 500.0)).unwrap();
        assert_eq!(p.slots, 2);
        assert_eq!(p.start.len(), 2);
        assert_eq!(p.deferred.len(), 5);
        assert!(p.deferred.iter().all(|d| d.because == "cap"));
    }

    #[test]
    fn the_disk_floor_binds_before_the_cap_and_is_never_overridden() {
        // 55 GB free, 20 GB floor, 10 GB each: three fit in all, one live leaves two.
        let p = plan(&load("mixed.json"), &codex(), &pool(9, 1, 55.0)).unwrap();
        assert_eq!(p.slots, 2);
        assert!(p.deferred.iter().all(|d| d.because == "disk-floor"));
        // Below the floor nothing starts.
        let p = plan(&load("mixed.json"), &codex(), &pool(9, 0, 19.0)).unwrap();
        assert_eq!(p.slots, 0);
        assert!(p.start.is_empty());
        assert_eq!(p.deferred.len(), 7);
    }

    #[test]
    fn bad_lists_are_refused() {
        let t = |id: &str, deps: &[&str], paths: &[&str]| Task {
            id: id.into(),
            kind: Kind::Docs,
            paths: paths.iter().map(|s| (*s).into()).collect(),
            needs: Needs::default(),
            depends: deps.iter().map(|s| (*s).into()).collect(),
        };
        assert!(tracks(&[t("A", &[], &["a"]), t("A", &[], &["b"])]).is_err());
        assert!(tracks(&[t("A", &["Z"], &["a"])]).is_err());
        assert!(tracks(&[t("A", &[], &[])]).is_err());
    }

    #[test]
    fn dependent_tasks_are_ordered_before_their_consumers() {
        let tasks: Vec<Task> = serde_json::from_value(serde_json::json!([
            {"id": "T004", "kind": "docs", "paths": ["docs/join.md"], "depends": ["T002", "T003"]},
            {"id": "T002", "kind": "docs", "paths": ["docs/left.md"], "depends": ["T001", "T001"]},
            {"id": "T003", "kind": "docs", "paths": ["docs/right.md"], "depends": ["T001"]},
            {"id": "T001", "kind": "docs", "paths": ["docs/root.md"]}
        ]))
        .unwrap();
        let p = plan(&tasks, &codex(), &pool(3, 0, 100.0)).unwrap();
        assert_eq!(p.tracks.len(), 1);
        assert_eq!(p.tracks[0].id, "t004");
        assert_eq!(ids(&p.tracks[0]), ["T001", "T002", "T003", "T004"]);
    }

    #[test]
    fn dependency_cycles_are_refused_including_self_dependencies() {
        for (first, second) in [("T001", "T002"), ("T001", "T001")] {
            let mut tasks = vec![Task {
                id: first.into(),
                kind: Kind::Docs,
                paths: vec!["docs/a.md".into()],
                depends: vec![second.into()],
                needs: Needs::default(),
            }];
            if first != second {
                tasks.push(Task {
                    id: second.into(),
                    kind: Kind::Docs,
                    paths: vec!["docs/b.md".into()],
                    depends: vec![first.into()],
                    needs: Needs::default(),
                });
            }
            let e = tracks(&tasks).unwrap_err().to_string();
            assert!(e.contains("cycle"), "{e}");
            assert!(e.contains(first) && e.contains(second), "{e}");
        }
    }

    #[test]
    fn independent_tasks_keep_input_order_and_an_empty_list_is_valid() {
        let tasks: Vec<Task> = serde_json::from_value(serde_json::json!([
            {"id": "T002", "paths": ["docs/shared.md"]},
            {"id": "T001", "paths": ["docs/shared.md"]},
            {"id": "T003", "paths": ["docs/other.md"]}
        ]))
        .unwrap();
        assert_eq!(tracks(&tasks).unwrap(), [vec![0, 1], vec![2]]);
        assert!(tracks(&[]).unwrap().is_empty());
    }

    #[test]
    fn cycle_diagnostic_excludes_consumers_outside_the_cycle() {
        let tasks: Vec<Task> = serde_json::from_value(serde_json::json!([
            {"id": "T004", "paths": ["docs/consumer.md"], "depends": ["T003"]},
            {"id": "T003", "paths": ["docs/a.md"], "depends": ["T002"]},
            {"id": "T002", "paths": ["docs/b.md"], "depends": ["T003"]}
        ]))
        .unwrap();
        let e = tracks(&tasks).unwrap_err().to_string();
        assert!(e.contains("T003 -> T002 -> T003"), "{e}");
        assert!(!e.contains("T004"), "{e}");
    }

    #[test]
    fn rust_fixtures_are_simple_but_rust_code_is_not() {
        let t = |kind, path: &str| Task {
            id: "T1".into(),
            kind,
            paths: vec![path.into()],
            needs: Needs::default(),
            depends: vec![],
        };
        assert_eq!(
            classify(&t(Kind::Fixtures, "rust/fixtures/x/a.json")),
            Class::Simple
        );
        assert_eq!(
            classify(&t(Kind::Fixtures, "rust/crates/c/src/a.rs")),
            Class::Judgment
        );
        assert_eq!(classify(&t(Kind::Other, "docs/a.md")), Class::Judgment);
    }
}
