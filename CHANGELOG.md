# Changelog

Releases are plain SemVer git tags (`1.1.0`). Each plugin also carries its
own `version` in `.claude-plugin/plugin.json`, bumped when that plugin
changes.

## Unreleased

Fixes from a dogfood run of `project:init` → `project:architecture` →
`project:spec` on a sample product.

### Fixed

- `project:spec` bootstraps Spec Kit with `--force`; without it `specify
  init` always stopped with "Current directory is not empty".
- The domain model no longer has a circular dependency:
  `project:architecture` and `project:spec` use it only if it exists and
  derive modules from the brief otherwise.
- `project:spec` keeps Spec Kit's per-story task phases (with tests) and
  points each story at its `## Frontend` / `## Backend` sections, runs
  `/speckit-analyze` per feature, follows `/speckit-clarify`'s
  one-question flow, and fills `Feature Branch` with the `git:workflow`
  branch.
- The pipeline contract covers `.specify/feature.json` and
  `SPECIFY_FEATURE` (so parallel stages don't act on the wrong feature),
  `checklists/`, and `speckit-taskstoissues` (remote issues only on
  request).
- Status words: feature statuses stay in `specs/README.md`; documents are
  `Draft` / `Accepted`; ADRs use MADR's statuses.
- QA personas come from the product's roles and authorization rules, and
  log in however the architecture's identity decision says (dev IdP,
  Mailpit-captured magic link, or seeded credentials).
- `x-status` lives in the contract's `info` object.
- Installer tests tag unique commits, so they pass in a repo that has
  release tags.

### Changed

- `project:architecture` decides the stack (new dimension 12) and writes
  ADRs only for contested decisions; obvious defaults get a one-line
  reason in the decisions table.
- Stages point to the pipeline's clarify rule instead of restating it.

### Added

- `project:spec` behavior eval: bootstrap in a non-empty repository.

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
