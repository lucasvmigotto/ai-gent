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
    /// The track id, from its first task (`t013`).
    pub id: String,
    /// Task ids, in list order.
    pub tasks: Vec<String>,
    /// The track's class: judgment if any task is.
    pub class: Class,
    /// The worker.
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

/// Split the list into tracks: tasks that share a path, or depend on each other, join one track.
///
/// # Errors
/// [`Error::Invalid`] for a duplicate id, a dependency on an unknown id, or a task without paths.
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
            id: members[0].id.to_lowercase(),
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
    all.truncate(slots);
    Ok(Plan {
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
