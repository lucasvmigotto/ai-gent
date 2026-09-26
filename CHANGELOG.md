# Changelog

Releases are plain SemVer git tags (`1.1.0`). Each plugin also carries its
own `version` in `.claude-plugin/plugin.json`, bumped when that plugin
changes.

## Unreleased

### Added

- Containerized browsers: `plugins/qa/references/browsers.md` makes the
  `selenium/standalone-*` family (Chrome, Firefox, Edge, Chromium) the
  execution environment for e2e journeys and screenshots — Selenium
  WebDriver by default on greenfield, Playwright kept where the project
  already uses it, host driver installs banned. WebKit has no standalone
  flavor and stays a documented Playwright-container exception.

### Changed

- `qa:e2e`, `qa:strategy`, `frontend:build` screenshots, `project:docs`
  e2e and the `devcontainer:infra` catalog point at the containerized
  browsers; CI runs the same images as local.

## 1.3.0 — 2026-09-26

### Added

- OpenCode V2 support: `setup.sh` writes the native `permissions` array
  (`shell` instead of the legacy `bash`) and registers `opencode/guard/`, an
  OpenCode V2 plugin that enforces the git and db guards mechanically — it can
  raise `ask` or `deny` and checks the current branch, which a shell pattern
  cannot. Its rules are a port of the shell guards, kept identical by a parity
  test (`opencode/guard/test/rules.test.mjs`, run by `scripts/check.sh`).
- `skill` denies for each plugin skill's short name, so OpenCode V2 shows only
  the namespaced `<plugin>-<skill>` skills, not the colliding IDs it also finds
  in `~/.claude/skills` — which V2 has no switch to disable.

### Changed

- `setup.sh` drops the `OPENCODE_DISABLE_CLAUDE_CODE_SKILLS` shell-profile edit
  (the variable no longer exists in OpenCode V2) and the `jq` config path; the
  OpenCode configuration is edited by `scripts/opencode-config.py`.
- The README verifies OpenCode with `opencode debug config` and
  `opencode plugin list` (`opencode debug skill` was removed in V2).

## 1.2.0 — 2026-09-25

Container-first development, three skills for existing codebases, LLM-
readable docs, automated releases — and fixes from a dogfood run of
`project:init` → `project:architecture` → `project:spec` on a sample
product.

### Added

- `setup.sh` offers, asking `[Y/n]`, to merge permission rules that mirror
  the git and db guards into `~/.config/opencode/opencode.json` (with `jq`,
  else `python3`; only missing keys, appended after the user's, with a
  backup; a file with comments is left alone), and to add
  `OPENCODE_DISABLE_CLAUDE_CODE_SKILLS=1` to the shell's rc file.
  `--yes` applies both, `--no-config-edits` skips both, `--uninstall`
  removes only what was added.
- `frontend:tui` — terminal interfaces, built only when the user asks for
  one: a TUI and the CLI it sits on (subcommands, `--json`, exit codes),
  or a CLI alone. Stack guide for Go/Bubble Tea (default), Rust/Ratatui,
  Python/Textual, Java/Lanterna, TypeScript/Ink and .NET/Spectre.Console or
  Terminal.Gui; snapshot, keystroke and `vhs` checks; terminal hygiene;
  packaging. `frontend:uiux`, `frontend:spec` (`tui.md`),
  `project:architecture`, `qa:e2e` and `project:docs` cover the terminal
  surface when one was requested.
- `db` plugin — `db:connect`, `db:inspect`, `db:review`, `db:investigate`
  for PostgreSQL, MySQL/MariaDB, SQL Server, Oracle and SQLite, all through
  the `dbrun` runner: remote databases are `SELECT`-only in every
  environment and changes to them are scripts for a person to run; only a
  proven local container of the project can be written to, after a plan,
  the user's confirmation and a backup; results are masked by default;
  every statement is logged before it runs. A guard hook denies direct
  client calls with writing SQL and asks before any other.
- `project:introspec` — reverse-engineers an existing codebase into the
  pipeline's artifacts (brief, as-is architecture, domain model, Spec Kit
  features with evidence-based statuses, rebuilt OpenAPI contract, SBOM),
  labelling every claim Observed, Inferred or Assumed; may read a dev
  database, never production.
- `project:retrofit` — in-place upgrades with behavior frozen behind
  characterization tests, at level `patch`, `minor` or `major`, with
  reachability-based CVE triage and an end-of-life table.
- `project:refactor` — redesign that keeps the core invariants, records
  each business change for approval (`docs/product/bcr/`), and migrates
  in strangler-style slices.
- `shared/containers.md` — container-first rules shared by `devcontainer`,
  `devsecops` and `project`: Podman first with Docker as fallback, a thin
  `tools` stage as the reference environment for agents and CI, Docker
  Hardened Images pinned by digest, multi-stage builds with bind and cache
  mounts, `.dockerignore`, determinism, resource limits, registry logins.
- `project:docs` emits `llms.txt`, `llms-full.txt` and a Markdown page per
  page and locale, with maturity labels in the text.
- Automated releases: `release.yml` runs the checks on every push to
  `main` and releases when the commits warrant it (`scripts/release.py`,
  tested by `scripts/test-release.sh`).

### Changed

- `devcontainer:setup` builds two layers per module — the tools layer
  (Containerfile, `compose.tools.yml`, task recipes, limits) and the human
  devcontainer, fed by one version source and a `doctor` drift check.
- `devcontainer:workflow` runs tasks through the tools recipes first, and
  every command uses the detected engine instead of `docker`.
- `devcontainer:infra` sets limits, profiles and digest-pinned images on
  every service.
- `devsecops:pipeline` runs CI through the same recipes and documents
  registry logins; `devsecops:supply-chain` hardens images per the shared
  rules.
- `check.yml` is reusable and runs on pull requests; pushes to `main` run
  it through `release.yml`.

### Fixed

- `db` guard hook denies every tool (Bash, Read, Grep, Glob, Edit, Write)
  that touches the credentials file, and checks each command on a line on
  its own — a `dbrun` call no longer exempts the rest of the line.
  `dbrun` redacts the profile's host, user and password from messages and
  log entries.
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
