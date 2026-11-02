//! The supervisor's checks on a delegate's diff (FR-011), run automatically and before review.
//!
//! Every refusal carries a reason id from [`REASONS`]; the id is what a status file's verdict and a
//! log line hold, never the diff. The checks are heuristics that refuse early: a finding means "a
//! person or the supervisor looks", and a clean run means only that these six rules hold.
use super::brief::protected_path;
use super::diff::{FileChange, Kind};
use super::glob;
use std::collections::BTreeMap;

/// A path outside `allowed_paths`.
pub const PATH_OUTSIDE: &str = "path-outside-allowed";
/// A path no delegate may edit (skills, plugins, hooks, shared references, AGENTS.md, CLAUDE.md, CHANGELOG.md).
pub const PROTECTED: &str = "protected-path";
/// A forbidden trailer or marker in a commit message or PR text.
pub const TRAILER: &str = "forbidden-trailer";
/// New prose about environment or sandbox failures.
pub const ENV_PROSE: &str = "environment-prose";
/// A test made weaker.
pub const WEAK_TEST: &str = "weakened-test";
/// A `tasks.md` mark without evidence.
pub const NO_EVIDENCE: &str = "tasks-mark-without-evidence";

/// Every reason id, in the order the checks run.
pub const REASONS: [&str; 6] = [
    PATH_OUTSIDE,
    PROTECTED,
    TRAILER,
    ENV_PROSE,
    WEAK_TEST,
    NO_EVIDENCE,
];

/// What the checks look at.
#[derive(Debug, Clone, Default)]
pub struct CheckInput {
    /// The changed files.
    pub changes: Vec<FileChange>,
    /// Commit messages and pull request text.
    pub texts: Vec<String>,
    /// The brief's `allowed_paths` globs.
    pub allowed: Vec<String>,
    /// Task id to the evidence for marking it done (a command and its result, a commit).
    pub evidence: BTreeMap<String, String>,
}

/// One refusal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// The reason id.
    pub reason: &'static str,
    /// The file, when the finding is about one.
    pub path: Option<String>,
    /// A short explanation (at most about 120 characters of the offending line).
    pub detail: String,
}

fn clip(s: &str) -> String {
    let t = s.trim();
    if t.chars().count() > 100 {
        let c: String = t.chars().take(100).collect();
        format!("{c}...")
    } else {
        t.to_string()
    }
}

/// Run every check.
#[must_use]
pub fn run(input: &CheckInput) -> Vec<Finding> {
    let mut f = Vec::new();
    paths(input, &mut f);
    trailers(input, &mut f);
    for c in &input.changes {
        env_prose(c, &mut f);
        weak_tests(c, &mut f);
        task_marks(c, &input.evidence, &mut f);
    }
    f
}

fn touched(c: &FileChange) -> Vec<&str> {
    let mut v = vec![c.path.as_str()];
    if let Some(o) = &c.old_path {
        v.push(o);
    }
    v
}

fn paths(input: &CheckInput, out: &mut Vec<Finding>) {
    for c in &input.changes {
        for p in touched(c) {
            if protected_path(p) {
                out.push(Finding {
                    reason: PROTECTED,
                    path: Some(p.to_string()),
                    detail: "a delegate may not edit skills, plugins, hooks, shared references or the agent instruction files".into(),
                });
            } else if !input.allowed.iter().any(|g| glob::matches(g, p)) {
                out.push(Finding {
                    reason: PATH_OUTSIDE,
                    path: Some(p.to_string()),
                    detail: "not covered by allowed_paths".into(),
                });
            }
        }
    }
}

/// Whether a line is a forbidden trailer or marker: `Co-Authored-By:`, `Generated with ...`,
/// `Built-by:` / `Built by ...`, ignoring case, a leading emoji or markup, and quoting.
#[must_use]
pub fn is_marker_line(line: &str) -> bool {
    let t = line.trim_start_matches(|c: char| !c.is_ascii_alphanumeric());
    let l = t.to_ascii_lowercase();
    l.starts_with("co-authored-by:")
        || l.starts_with("co-authored-by ")
        || l.starts_with("generated with")
        || l.starts_with("built-by")
        || l.starts_with("built by")
}

fn trailers(input: &CheckInput, out: &mut Vec<Finding>) {
    for t in &input.texts {
        for line in t.lines().filter(|l| is_marker_line(l)) {
            out.push(Finding {
                reason: TRAILER,
                path: None,
                detail: format!("marker line: {}", clip(line)),
            });
        }
    }
}

const PROSE_EXT: [&str; 6] = ["md", "markdown", "txt", "rst", "adoc", "mdx"];
const ENV_PHRASES: [&str; 11] = [
    "sandbox",
    "operation not permitted",
    "read-only file system",
    "no network",
    "network is unreachable",
    "environment failure",
    "environment limitation",
    "denied socket",
    "sockets denied",
    "sockets are denied",
    "not real failures",
];

fn ext(path: &str) -> &str {
    path.rsplit_once('.').map_or("", |(_, e)| e)
}

fn env_prose(c: &FileChange, out: &mut Vec<Finding>) {
    if !PROSE_EXT.contains(&ext(&c.path).to_ascii_lowercase().as_str()) {
        return;
    }
    for line in &c.added {
        let l = line.to_ascii_lowercase();
        if ENV_PHRASES.iter().any(|p| l.contains(p)) {
            out.push(Finding {
                reason: ENV_PROSE,
                path: Some(c.path.clone()),
                detail: format!("new prose about an environment failure: {}", clip(line)),
            });
        }
    }
}

/// Whether a path is a test file.
#[must_use]
pub fn is_test_path(path: &str) -> bool {
    let p = glob::norm(path);
    let segs: Vec<&str> = p.split('/').collect();
    let name = segs.last().copied().unwrap_or("");
    let stem = name.split('.').next().unwrap_or("");
    segs[..segs.len() - 1]
        .iter()
        .any(|s| matches!(*s, "tests" | "test" | "__tests__" | "spec" | "specs_tests"))
        || stem.ends_with("_test")
        || stem.starts_with("test_")
        || stem == "tests"
        || name.contains(".test.")
        || name.contains(".spec.")
}

fn assertion(line: &str) -> bool {
    let l = line.trim().to_ascii_lowercase();
    if l.starts_with("//") || (l.starts_with('#') && !l.starts_with("#[test]")) {
        return false;
    }
    l.contains("assert")
        || l.contains("expect(")
        || l.contains("#[test]")
        || l.contains("def test_")
        || l.starts_with("it(")
        || l.starts_with("test(")
}

const SKIPS: [&str; 8] = [
    "#[ignore",
    "@pytest.mark.skip",
    "@unittest.skip",
    ".skip(",
    "xit(",
    "xdescribe(",
    "assert!(true)",
    "assert true",
];

fn weak_tests(c: &FileChange, out: &mut Vec<Finding>) {
    for line in &c.added {
        let l = line.to_ascii_lowercase();
        if SKIPS.iter().any(|s| l.contains(s)) {
            out.push(Finding {
                reason: WEAK_TEST,
                path: Some(c.path.clone()),
                detail: format!("test skipped or made vacuous: {}", clip(line)),
            });
        }
    }
    if !is_test_path(&c.path) {
        return;
    }
    if c.kind == Kind::Deleted {
        out.push(Finding {
            reason: WEAK_TEST,
            path: Some(c.path.clone()),
            detail: "a test file was deleted".into(),
        });
        return;
    }
    let added = c.added.iter().filter(|l| assertion(l)).count();
    let removed = c.removed.iter().filter(|l| assertion(l)).count();
    if removed > added {
        out.push(Finding {
            reason: WEAK_TEST,
            path: Some(c.path.clone()),
            detail: format!("assertions removed {removed}, added {added}"),
        });
    }
}

fn mark(line: &str) -> Option<(bool, String)> {
    let t = line.trim_start();
    let rest = t.strip_prefix("- [").or_else(|| t.strip_prefix("* ["))?;
    let (box_, rest) = rest.split_once("] ")?;
    let done = match box_ {
        "x" | "X" => true,
        " " => false,
        _ => return None,
    };
    let id: String = rest
        .chars()
        .take_while(char::is_ascii_alphanumeric)
        .collect();
    super::status::valid_task_id(&id).then_some((done, id))
}

fn task_marks(c: &FileChange, evidence: &BTreeMap<String, String>, out: &mut Vec<Finding>) {
    if !c.path.ends_with("tasks.md") {
        return;
    }
    let was_done: Vec<String> = c
        .removed
        .iter()
        .filter_map(|l| mark(l))
        .filter(|(d, _)| *d)
        .map(|(_, id)| id)
        .collect();
    for (done, id) in c.added.iter().filter_map(|l| mark(l)) {
        if !done || was_done.contains(&id) {
            continue;
        }
        if evidence.get(&id).is_none_or(|e| e.trim().is_empty()) {
            out.push(Finding {
                reason: NO_EVIDENCE,
                path: Some(c.path.clone()),
                detail: format!("{id} is marked done with no evidence"),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::delegate::diff;
    use crate::delegate::tests_support::fixtures;

    fn allowed() -> Vec<String> {
        vec![
            "rust/spikes/dbrun/**".into(),
            "specs/038-db-runner/tasks.md".into(),
            "README.md".into(),
        ]
    }

    fn input(diff_name: &str, texts: &[&str], evidence: &[(&str, &str)]) -> CheckInput {
        let d = std::fs::read_to_string(fixtures().join("checks").join(diff_name)).unwrap();
        CheckInput {
            changes: diff::parse(&d),
            texts: texts.iter().map(|s| (*s).to_string()).collect(),
            allowed: allowed(),
            evidence: evidence
                .iter()
                .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
                .collect(),
        }
    }

    fn reasons(i: &CheckInput) -> Vec<&'static str> {
        run(i).iter().map(|f| f.reason).collect()
    }

    #[test]
    fn trial_2_diff_is_clean() {
        let i = input(
            "trial2-clean.diff",
            &["chore(rust): dbrun spike _exec bridge (spec 038 T003)"],
            &[("T003", "cargo test -p dbrun-spike: 8 journeys pass")],
        );
        assert_eq!(run(&i), vec![]);
    }

    #[test]
    fn trial_1_marker_line_is_refused() {
        let msg = "chore(rust): dbrun spike core fixtures\n\nCo-Authored-By: Some Agent <noreply@example.com>\n";
        let i = input("trial2-clean.diff", &[msg], &[("T003", "ok")]);
        let f = run(&i);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].reason, TRAILER);
        for m in [
            "Generated with [Claude Code](https://x)",
            "🤖 Generated with Foo",
            "Built-by: codex",
            "built by Codex",
        ] {
            assert!(is_marker_line(m), "{m}");
        }
        assert!(!is_marker_line("Refactor the co-authored-by parser"));
    }

    #[test]
    fn trial_1_out_of_scope_path_is_refused() {
        let i = input("trial1-out-of-scope.diff", &[], &[("T001", "ok")]);
        let f = run(&i);
        let outside: Vec<_> = f
            .iter()
            .filter(|f| f.reason == PATH_OUTSIDE)
            .map(|f| f.path.clone().unwrap())
            .collect();
        assert_eq!(
            outside,
            [
                "rust/crates/ai-gent-core/src/lib.rs",
                "rust/crates/ai-gent/src/main.rs"
            ]
        );
        assert_eq!(f.len(), 2);
    }

    #[test]
    fn protected_paths_are_their_own_reason() {
        let i = input("protected.diff", &[], &[]);
        assert_eq!(reasons(&i), [PROTECTED, PROTECTED]);
    }

    #[test]
    fn trial_1_environment_prose_is_refused() {
        let i = input("trial1-env-prose.diff", &[], &[]);
        let f = run(&i);
        assert_eq!(f.len(), 2, "{f:?}");
        assert!(
            f.iter()
                .all(|f| f.reason == ENV_PROSE && f.path.as_deref() == Some("README.md"))
        );
    }

    #[test]
    fn environment_words_in_code_are_not_prose() {
        let c = FileChange::added_file(
            "rust/spikes/dbrun/src/lib.rs",
            "// the sandbox flag is workspace-write\n",
        );
        let i = CheckInput {
            changes: vec![c],
            allowed: allowed(),
            ..CheckInput::default()
        };
        assert!(run(&i).is_empty());
    }

    #[test]
    fn weakened_tests_are_refused() {
        let i = input("weakened-test.diff", &[], &[]);
        let f = run(&i);
        let r: Vec<_> = f
            .iter()
            .map(|f| (f.reason, f.path.as_deref().unwrap()))
            .collect();
        assert_eq!(
            r,
            [
                (WEAK_TEST, "rust/spikes/dbrun/tests/journeys.rs"),
                (WEAK_TEST, "rust/spikes/dbrun/tests/journeys.rs"),
                (WEAK_TEST, "rust/spikes/dbrun/tests/old.rs"),
            ]
        );
    }

    #[test]
    fn rewriting_a_test_with_equal_assertions_passes() {
        let mut c = FileChange::added_file("rust/spikes/dbrun/tests/a.rs", "assert_eq!(a, 2);\n");
        c.kind = Kind::Modified;
        c.removed = vec!["assert_eq!(a, 1);".into()];
        let i = CheckInput {
            changes: vec![c],
            allowed: allowed(),
            ..CheckInput::default()
        };
        assert!(run(&i).is_empty());
    }

    #[test]
    fn tasks_marks_need_evidence() {
        let i = input(
            "tasks-marks.diff",
            &[],
            &[("T003", "cargo test -p dbrun-spike: pass"), ("T013", "  ")],
        );
        let f = run(&i);
        assert_eq!(f.len(), 1, "{f:?}");
        assert_eq!(f[0].reason, NO_EVIDENCE);
        assert!(f[0].detail.starts_with("T013"));
        // T001 was already done before the diff: not a new mark.
    }

    #[test]
    fn every_finding_uses_a_listed_reason() {
        for f in [
            "trial1-out-of-scope.diff",
            "trial1-env-prose.diff",
            "weakened-test.diff",
            "tasks-marks.diff",
            "protected.diff",
        ] {
            for x in run(&input(f, &["Co-Authored-By: a"], &[])) {
                assert!(REASONS.contains(&x.reason));
            }
        }
    }
}
