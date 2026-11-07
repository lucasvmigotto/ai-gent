# Plan: renaming the integration branch (`dev` to `develop`) - PLAN ONLY

**The rename stays on hold until the owner says so.** Nothing has been renamed, no workflow, hook, fixture, ruleset or document was edited, and nothing here may be executed on the strength of this file alone. Base read: `origin/dev` at 41dff19 (2026-11-07), by `grep` on an export of that tree; the GitHub repository settings (rulesets, default branch, open PRs) are not visible from the repository and were not inspected.

Direction assumed: `dev` becomes `develop`. The brief says "develop-rename" without a direction; the repository already accepts both spellings in most places (below), which is the reason the rename is cheap in code and expensive in settings and history. If the direction is the other way (or a third name), every "to" in this plan flips.

## What references the branch name

Counts are files, from `grep -rI` over the tree, excluding `node_modules`, `target` and lock files.

**Already name-agnostic (both `dev` and `develop` are accepted, so no change needed):**

- `plugins/git/hooks/guard.sh` (the commit-on-`dev`/`develop` rule at line ~200 and the autonomy-grant default `develop dev`), `plugins/repo/hooks/guard.sh` (same default), `opencode/guard/rules.mjs` (lines 35 and 175), `shared/guard-ux.ts` copies.
- The Rust evaluator: `rust/crates/ai-gent-core/src/guard/grant.rs`, `rust/crates/ai-gent-core/src/wt/git.rs` (looks for `["dev", "develop"]`), `doctor/checks.rs`.
- `plugins/project/hooks/grant-model.ts` (default `develop dev`) and `plugins/project/scripts/repo_state.py` (`MAIN_BRANCHES` lists both).
- Skills: `plugins/git/skills/workflow/SKILL.md` and `references/chain.md` say `dev`/`develop` throughout.

**Hard-wired to the literal `dev` (must change or be re-checked):**

| Where | What | Change |
|---|---|---|
| `.github/workflows/release.yml` line ~94 | `integ=dev`: the release job's back-merge target; the warning text names it | `integ=develop` (or detect both) |
| `.github/workflows/docs-ci.yml` line 5 | `push: branches: [main, dev]` | `[main, develop]` |
| `.github/workflows/check.yml`, `rust.yml`, `rust-build.yml`, `scheduled.yml` | no branch filter on `pull_request`; `workflow_dispatch` runs use `--ref dev` in docs and specs | none in the YAML; re-check `rust-build.yml` `on:` block |
| `.github/dependabot.yml` | no `target-branch` found: Dependabot targets the default branch | none, unless the default branch changes |
| `scripts/release.py`, `shared/release-single.py` | no `dev` literal found in `release.py`; it plans on `main` history | re-run `--dry-run` after; no code change expected |
| `scripts/consistency.py` (lines ~33, 526, 532) | samples the release subjects `chore(release): merge X.Y.Z back into dev` (via `$integ`/`<integ>` replaced by `dev`) and checks the project band's patterns | change the sample name to `develop`; the band patterns in `plugins/git/hooks/` must match either name |
| `shared/releasing.md` (lines ~62, 199) | `dev`/`develop` and "branch protection of `dev` and `main`" | edit the sentence |
| `shared/guard-fixtures/git.json` (13 uses), `repo.json` (3), `rust/fixtures/wt/*.json`, `rust/fixtures/spec/live-corpus.json`, `rust/fixtures/tokens/*.json` | fixtures that create a `dev` branch in a sandbox or print one | usually sandbox names; change only where a fixture asserts the real repo's branch. These are parity fixtures: change both implementations' expected outputs together |
| `scripts/test-parity-wt.py` (12), `plugins/agents/scripts/test_wt.py` (11), `plugins/project/scripts/test_scripts.py` (9), `plugins/repo/hooks/ci.test.tsx` (11), `plugins/git/hooks/hygiene.test.tsx` (5), `plugins/agents/hooks/worktrees.test.tsx` (5), `plugins/repo/scripts/test_reporun.py`, `scripts/test-parity-repo.py` | tests that make a `dev` branch | sandbox-local: they pass with either name as long as the tool under test accepts both; verify rather than rewrite |
| `plugins/agents/scripts/wt.py` (4), `token_report.py` (3), `plugins/repo/scripts/reporun.py` (2), and their Rust ports | `dev` as the base of a worktree chain or a branch label | accept both already in the ports; check the Python originals |
| `README.md` (the "Land" diagram and the CI watch band example), `plugins/git/skills/workflow/SKILL.md` | prose and mermaid | text edits |
| `docs/site/src/i18n.tsx`, `docs/product/*` | prose | text edits; docs CI deploy runs on pushes to `main` and `dev` |
| `specs/**` | historical text (`merge dev`, "PR to `dev`") | leave as history; add a note in `specs/README.md` |
| `CHANGELOG.md` | past entries mention `dev` | leave |

**Outside the repository (cannot be grepped):** the GitHub ruleset and required checks of `dev` ("dev's ruleset requires pull requests", `release.yml`), the default-branch setting, open PRs targeting `dev`, local clones and `.worktrees/*` of every contributor, `origin/dev` remote-tracking refs, any pinned `workflow_dispatch` command in the owner's notes, the autonomy grants in `.ai-gent/autonomy` (they list `branches: develop dev`, so they keep working; they are never edited here), and `docs/site` deploy settings.

## Steps (to run only after the owner says go)

1. Freeze: no open PR other than the rename; the bot is not mid-release (check `gh run list --workflow release.yml`); the owner announces the window.
2. Prepare the repo changes on a branch off `dev`: the edits from the table (release.yml, docs-ci.yml, consistency.py, releasing.md, README, workflow skill, docs), plus a test that proves the release job's back-merge picks `develop`. Run `scripts/check.sh` in full. Propose first (CLAUDE.md); no edit until approved.
3. On GitHub (owner): copy the ruleset of `dev` to the new name first (so protection is never missing), then rename the branch in Settings > Branches (GitHub retargets open PRs and keeps redirects for the old name in the web UI). Do not delete `dev` as a separate step; the rename is atomic and keeps history.
4. Merge the prepared branch (now into `develop`) by PR.
5. Everyone: `git branch -m dev develop && git fetch origin && git branch -u origin/develop develop && git remote set-head origin -a`; worktrees under `.worktrees/` are re-based on the new name by `git worktree` metadata only if their upstream is repaired the same way.
6. Verify: `python3 scripts/release.py --dry-run` still plans from `main`; push a trivial docs change to `develop` and see Docs CI run; run the next release in `--dry-run` and then for real, and confirm the back-merge warning (or success) names `develop`; `gh api repos/lucasvmigotto/ai-gent/branches/develop/protection` shows the required checks of `shared/releasing.md` ("Required checks").
7. Update `specs/README.md` notes and the memory notes of any agent that quotes `dev`.

## Risks

- **The back-merge** is the one place a wrong name is silent: `release.yml` checks `git ls-remote --heads origin "$integ"` and does nothing when the branch does not exist, so after the rename with `integ=dev` the release commit would never return to the integration branch and the next plan would count from an old tag. Edit and test this before step 3.
- **Branch protection gap:** renaming without copying the ruleset can leave the branch unprotected for a window, or the required status names mismatch and block every PR.
- **Required checks** must still be the list in `shared/releasing.md`; a renamed base does not change check names, but a new ruleset can be created with a different set.
- **Fixtures/tests** that assert the literal text `dev` in guard reasons or in the project band will fail in two implementations at once (`guard.sh` and the Rust evaluator, which must decide the same). Change them in the same commit.
- **Autonomy grants** list `develop dev` today: they keep covering both. A grant that lists only `dev` stops covering pushes after the rename; the owner re-issues it (the agents never edit `.ai-gent/autonomy`).
- **External links** (docs pages, issues, the 034/039 specs) that cite `origin/dev` or `--ref dev` break for copy-paste, not for the web UI.
- **Concurrent agents/worktrees** on the old name: pushes to a deleted `dev` fail loudly; still, do it in an idle window.

## Rollback

GitHub can rename the branch back (Settings > Branches) and redirects both ways; restore the ruleset from the copy. Revert the prepared PR (`git revert -m 1`). Contributors rerun the step-5 commands with the names swapped. Nothing in this plan deletes history, so the rollback is symmetric. If the release job ran in between with the new name, the back-merge it made is on `develop` and moves with the rename back.

## Decision log

| Question | Owner answer |
|---|---|
| Direction and exact new name | |
| Go-ahead and window | |
| Keep `dev` as an alias (a second branch) or drop it | |
