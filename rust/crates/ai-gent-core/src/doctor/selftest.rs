//! The guard self-test: a built-in set of decisions the evaluator (spec 033) must keep making.
use crate::guard::{Ctx, Event, GuardId, Level, Shell, evaluate};

/// One expectation.
#[derive(Debug, Clone, Copy)]
pub struct Case {
    /// Guard to ask.
    pub guard: GuardId,
    /// Shell command.
    pub command: &'static str,
    /// Expected decision.
    pub want: Level,
}

const fn c(guard: GuardId, command: &'static str, want: Level) -> Case {
    Case {
        guard,
        command,
        want,
    }
}

/// One deny, one ask and one none where the guard has them (taken from `shared/guard-fixtures`).
pub const CASES: &[Case] = &[
    c(
        GuardId::Git,
        "git commit --no-verify -m \"fix: x\"",
        Level::Deny,
    ),
    c(
        GuardId::Git,
        "git push --force-with-lease origin feat/x",
        Level::Ask,
    ),
    c(GuardId::Git, "ls -la", Level::None),
    c(
        GuardId::Db,
        "psql -h db.internal -U app -c \"DELETE FROM orders WHERE id = 1\"",
        Level::Deny,
    ),
    c(
        GuardId::Db,
        "psql -h db.internal -U app -c \"SELECT id FROM orders LIMIT 5\"",
        Level::Ask,
    ),
    c(GuardId::Db, "ls -la", Level::None),
    c(
        GuardId::Secrets,
        "pass show ai-gent/db/shop-dev",
        Level::Deny,
    ),
    c(GuardId::Secrets, "pass rm ai-gent/db/old", Level::Ask),
    c(
        GuardId::Secrets,
        "pass-run GH_TOKEN=ai-gent/github/pat -- gh pr list",
        Level::None,
    ),
    c(
        GuardId::Repo,
        "python3 /home/me/ai-gent/plugins/repo/scripts/reporun.py cli github --write -- issue comment 3 --body 'ok\\n\\nCo-Authored-By: Someone <a@b.c>'",
        Level::Deny,
    ),
    c(
        GuardId::Repo,
        "python3 /home/me/ai-gent/plugins/repo/scripts/reporun.py cli github --write -- pr merge 3 --squash",
        Level::Ask,
    ),
    c(GuardId::Repo, "ls -la", Level::None),
    c(
        GuardId::Containers,
        "podman system prune -a -f",
        Level::Deny,
    ),
    c(GuardId::Containers, "ls -la", Level::None),
];

/// Evaluate `cases`; the mismatches as text, empty when all agree.
#[must_use]
pub fn run(cases: &[Case]) -> Vec<String> {
    let branch = |_: &str| Some("feat/x".to_string());
    let ctx = Ctx { branch_of: &branch };
    cases
        .iter()
        .filter_map(|k| {
            let ev = Event {
                tool_name: "Bash".into(),
                cwd: "/tmp".into(),
                tool_input: serde_json::json!({ "command": k.command }),
            };
            let got = evaluate(k.guard, &ev, Shell::Sh, &ctx).level;
            (got != k.want).then(|| {
                format!(
                    "{}: wanted {}, got {} for {:?}",
                    k.guard.name(),
                    k.want.name(),
                    got.name(),
                    k.command
                )
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_expectations_hold() {
        assert_eq!(run(CASES), Vec::<String>::new());
    }

    #[test]
    fn a_flipped_expectation_is_reported() {
        let mut flipped = CASES.to_vec();
        flipped[0].want = Level::None;
        let bad = run(&flipped);
        assert_eq!(bad.len(), 1);
        assert!(bad[0].starts_with("git: wanted none, got deny"), "{bad:?}");
    }
}
