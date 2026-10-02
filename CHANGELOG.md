# Changelog

Releases are plain SemVer git tags (`1.1.0`). Each plugin also carries its
own `version` in `.claude-plugin/plugin.json`, bumped when that plugin
changes.

## Unreleased

### Changed

- **The Spec Kit contract moved to `shared/spec-kit.md`.** The five-step
  flow, its gates, its owners and the CLI-vs-pipeline divergences now live
  beside `shared/pipeline.md`, symlinked only into the four plugins that
  act on `specs/` (project, frontend, backend, qa) instead of six. An
  edit to the Spec Kit flow bumps four plugins rather than six, and the
  ten skills that read the pipeline for ownership alone stop loading a
  section they never used. `shared/pipeline.md` keeps the chain, the
  artifact owners and the rules every stage follows.
- Removed `db`'s unused `references/pipeline.md` link — no db skill ever
  read it, so a pipeline edit was bumping that plugin for a contract it
  never consults.

## 1.6.0 — 2026-10-02

### Changed

- **Spec Kit flow is now explicit**: every feature goes through
  `specify → clarify → plan → tasks → implement`, each step with a gate
  that must hold before the next one starts. `clarify` is a step in its
  own right (a feature with an open `[NEEDS CLARIFICATION]` is not
  planned), and `implement` is named rather than implied.
- **`implement` is the build stages' step.** `frontend:build` and
  `backend:build` adopt `/speckit-implement`'s contract — context load,
  the installed template's phases, tests before code, `- [x]` as each task
  lands, halt on failure, validation at the end — scoped to their own
  `## Frontend` / `## Backend` section of `tasks.md`. The bare command is
  never run over a multi-layer `tasks.md`, since it has no scoping flag
  and would tick another stage's tasks.
- **`/speckit-converge` joins the loop.** Each build skill runs it after a
  feature's last checkpoint; anything still unbuilt is appended as
  `## Phase N: Convergence (<Layer>)` and implemented before the feature is
  marked Implemented. A feature is Implemented only once it has converged.
- `project:status` reports a feature blocked by an open clarification
  marker, and a feature claimed Implemented with unchecked Convergence
  tasks, as contradictions.
- `shared/pipeline.md` documents where the CLI and the pipeline disagree
  (phase names, "tests are optional", whole-file ticking, converge's
  heading) and which one wins.
- **README:** the pipeline flowchart now names the four steps inside
  `project:spec` and marks the build stages as *implement + converge*,
  and the status diagram's transition to Implemented reads "build
  checkpoints pass + converge", matching the pipeline's statuses.

## 1.5.1 — 2026-09-30

### Fixed

- **devcontainer:** point the cleanup rules at the shared containers reference

## 1.5.0 — 2026-09-29

### Added

- `project:survey` — first contact with a repository: stack and versions,
  layout and build/test/run commands, CI and containers, history and
  conventions, how far the main branches (`main`, `master`, `develop`,
  `dev`, `homolog`, `staging`) drift from each other and their remotes,
  work in flight, red flags; ends in the stage to run next.
- `project:recap` — back to a project after a pause or a stalled
  session: where the last session left off, read from **either** Claude
  Code or OpenCode whichever you run in (requests, unfinished work,
  unanswered questions, a stop mid-task); what changed since, by you, the
  remote and others; the conflicts between the two; one next step.
- `plugins/project/scripts/sessions.py` (session digests for both tools,
  secrets redacted, tool output left out) and `repo_state.py` (a
  read-only git snapshot), tested by `test_scripts.py` in `scripts/check.sh`.

- `scripts/test-dbrun-engines.py` — `dbrun` end to end against real
  PostgreSQL, MySQL, MariaDB, SQL Server and Oracle containers, one engine
  at a time with memory caps (`--engine` for one); run before pushing a
  `dbrun` change, not part of CI.

- `compact` plugin — `compact:code` reads source code token-lean: a
  lexer-aware, self-verified compact view (strings, comments and
  preprocessor lines kept) and an outline with original line numbers, for
  Java, C#, C/C++, Go, Rust, JS/TS, Kotlin, PHP and more. Read-only: the
  writing and formatting side of the original skill is left out. Pygments
  comes through `uv`; tested by `test_compact.py` in `scripts/check.sh`.
  Its verifier is stricter than the original's: spacing inside any string,
  operators fused across a removed space, and (with tree-sitter) a result
  that no longer parses are all rejected; SCSS and Less files with `//`
  comments keep their lines, since Pygments misses some of those comments
  and code could otherwise be joined into one.

### Changed

- `scripts/check.sh` pins the Pygments that `compact:code`'s tests run with
  (`PYGMENTS_VERSION`) and the uv fetched through pipx (`UV_VERSION`), as
  it already pinned shellcheck, so local runs and CI agree.
- The `git` guard (and its OpenCode port) follows the branch chain: it
  also asks before commits or merges on `homolog`/`staging` and before
  commits made directly on `dev`/`develop`; merges into `dev` pass, and a
  conflicted one is concluded with `git merge --continue`.
- `git:workflow` asks for a commit by showing each commit's message with the
  files it stages, one block per commit. Inside an approved development
  workflow (a pipeline stage's phases, Spec Kit tasks, a plan the user said
  to execute) it commits as each task or phase finishes without asking, and
  lists the commits in the handoff; merges, pushes and promotions still ask.
- `git:workflow` follows a branch chain — `main`/`master` >
  `homolog`/`staging` > `dev`/`develop` > work branches: every branch
  starts from and merges into `dev` (conflicts settled there), and `dev`
  is promoted one level at a time up to `main`. `dev` is created from
  `main` when it doesn't exist, from the remote when only the remote has
  it, recreated at the remote's point when it's purely behind it, and
  used as it is when it's only behind `main`; a `dev` diverged from its
  remote stops for a question, with a backup branch suggested.
  When `dev` has fallen far behind the levels above, the skill flags it and
  asks; if the user agrees, work branches off the highest current level and
  merges back into it, leaving `dev` alone. Promotion into `main` needs its
  own explicit request.

- `shared/reading-code.md` — how skills that read code in bulk keep its
  token cost down: map first, `compact:code` for large or many
  brace-language files, `sed -n`/`cat` instead of line-numbered Reads for
  the rest, exact lines Read before an edit. Linked into `project`, `db`,
  `qa`, `backend` and `frontend`; `survey`, `introspec`, `retrofit`,
  `refactor`, `db:review`, `qa:review`, `backend:build` and
  `frontend:build` point to it.

- `setup.sh` prints a summary instead of a line per skill: on a terminal
  the current item is shown on one line rewritten in place, and each
  section ends with one line per status and its count (`[ok] [8/9]`,
  `[link] [1/9] compact`, skip reasons below), in color, bold and italics
  where the terminal has them (`NO_COLOR` turns them off). Without a
  terminal only the summary is printed; `--verbose` restores one line per
  item.

- The pipeline contract (`shared/pipeline.md`) places `project:survey`
  before `project:introspec` and `project:recap` alongside
  `project:status`, both report-only, and its versioning rule follows the
  branch chain.

### Fixed

- `project:spec` bootstraps Spec Kit with `--ignore-agent-tools`: `specify
  init --integration claude` stopped when no `claude` CLI was on PATH
  (OpenCode, a sandbox), although the agent running it is the integration.
- `dbrun`, tested end to end against MariaDB 11.4, SQL Server 2022 and
  Oracle Free 23 (and again on PostgreSQL 17 and MySQL 8.4):
  - SQL Server `apply` always rolled back: the session set `SET NOCOUNT
    ON`, which hides row counts from the driver.
  - `--explain` returned the echoed statement on SQL Server and failed on
    Oracle (`ORA-02000`); both now return the plan.
  - `restore` left behind objects the plan created. PostgreSQL, MySQL and
    MariaDB backups now recreate the whole database.
  - Plans in system databases (SQL Server `master`, which can't be
    restored; MySQL `mysql`; …) are refused.
  - On remote targets, SQL Server locking table hints and `NEXT VALUE
    FOR` / `seq.NEXTVAL` are refused as writes.
  - Without a backup (Oracle), `apply` no longer offers a `restore` that
    can't work.

## 1.4.0 — 2026-09-26

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
