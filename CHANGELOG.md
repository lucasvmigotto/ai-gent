# Changelog

Releases are plain SemVer git tags (`1.1.0`). Each plugin also carries its
own `version` in `.claude-plugin/plugin.json`, bumped when that plugin
changes.

## 1.1.0 — 2026-09-24

### Added

- `git` plugin: `git:workflow` (formerly the `git-workflow` skill) with a
  `PreToolUse` guard hook that blocks `--no-verify`, co-author trailers
  and force pushes without a lease, and asks before pushes, commits or
  merges on `main`/`master`, `branch -D`, `reset --hard`, `clean -f`,
  `commit --amend`, history rewrites and `gh pr create`.
- `devsecops:iac` — infrastructure as code (OpenTofu/Terraform by
  default) for the environments the architecture chose.
- `project:status` — read-only pipeline status and next-stage
  recommendation.
- `git:workflow`: tag and release rules (plain SemVer, no `v` prefix
  unless the repo already uses one).
- `scripts/check.sh` (metadata, references, symlinks, shellcheck, guard
  and installer tests) and a GitHub Actions workflow that runs it.
- Trigger evals for every plugin skill (`plugins/*/evals/`, run with
  `claude plugin eval`).
- `AGENTS.md` (also `CLAUDE.md`) with the conventions for editing skills.
- `install.sh`: `AI_GENT_BRANCH` accepts a tag, to pin a release.

### Changed

- Skill descriptions shortened to at most 400 characters, which halves
  what every session loads (about 4.0k → 2.1k tokens for all plugins).
- `devcontainer:infra` moves identity (OAuth2/OIDC/SAML/LDAP) and mail
  into `references/`, `devcontainer:workflow` moves Docker-in-devcontainer
  and its worked example, and `git:workflow` moves issue linking; each
  is read only when a run needs it.
- **Breaking:** `/git-workflow` is now `/git:workflow` in Claude Code
  (still `git-workflow` in opencode). Re-run `setup.sh`.
- `project:docs` defers to `git:workflow` instead of restating its rules,
  and pins TypeScript to the current stable major rather than 7.

## 1.0.0 — 2026-09-24

First release: the `devcontainer`, `project`, `frontend`, `backend`,
`qa` and `devsecops` plugins, the shared product-pipeline contract, the
`git-workflow` skill, and `setup.sh` / `install.sh` for Claude Code and
opencode.
