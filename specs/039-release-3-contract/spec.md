# Feature Specification: The 3.0.0 release contract

**Feature Branch**: `docs/specs-rust-core` (specification only; built on its own branches)

**Created**: 2026-11-01

**Status**: Draft

**Input**: D1, D2, D7, D8, D13, D16, D18, D20, D21 of `docs/product/rust-decisions.md` and `docs/product/release-3-notes.md`. This spec is the checklist for the 3.0.0 release: what it contains and removes, what breaks and how an existing user migrates, the measurable gates that 2.27, 2.28 and 3.0.0 must pass, how to go back, what tier 3 means, and the documentation changes (as tasks, not edits). The version is written 3.0.0 or 3.x; tags carry no `v` (D21).

**Depends on**: 032, 033, 034 (written with this spec) and 035-038, 040 (written by another builder: the ported tools, the dbrun spike (038) and the docs-site v2; this spec reads their status, not their content). Read by nobody else; it closes the series.

## Context and ground rules

Verified on 2026-11-01 (commit 7c09c56):

- 2.26.0 shipped the release package as the default install and git clone as a flagged dev mode (`AI_GENT_MODE=git`, `AI_GENT_BRANCH`, `AI_GENT_REPO`, `AI_GENT_DIR`). `install.sh` (header line 22 and the notice near line 238), `scripts/test-install.sh` (line ~322, which asserts that notice) and `README.md` (line 52) announce that mode is going away using the old spelling of the major version, which D21 retires; correcting the wording is task T010/T011/T041.
- `install.sh` refuses when the install root is a git clone and tells the person to move it aside and run `current/setup.sh --migrate`; `setup.sh --migrate` (setup.sh:121-139, 437) relinks skills that point into another ai-gent tree and does not touch the old tree.
- Releases are automated: a `feat!:`/`type!:` subject or a `BREAKING CHANGE:` footer makes `scripts/release.py` plan a major bump (`shared/releasing.md`). The release job's `chore(release)` commit and tag are made by the bot; nobody tags by hand (CLAUDE.md).
- The README lists seven tier-3 agents without a tier concept; guard hooks are "instructions only" on every agent but Claude Code and opencode (README:124, 396); `AGENT_IDS` in `setup.sh:103` has nine agents.
- Python and shell tools referenced from skills (decisions file counts): `wt.py` 73 times, `token_report.py` 52, `reporun.py` 18, `compact.py` 18, `dbrun.py` 11, `specsync.py` 6. D19 keeps `dbrun` last or never.
- D24 puts a native Windows installer `install.ps1` in 3.0.0; D25 keeps docs hosting on R2 and puts the SHA256 of `install.sh`, `install.ps1` and the packages in each release's notes; D26/D27 make the `dbrun` migration the last 3.x step and not a 3.0.0 gate.
- Docs: `README.md`, `AGENTS.md`/`CLAUDE.md`, `docs/site/` (React/Vite), `shared/*.md` (releasing, secrets, token-tracking, containers, release-platforms, ...), `CHANGELOG.md`.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - The owner can say "3.0.0 is ready" from a checklist, not a feeling (Priority: P1)

**Independent Test**: `contracts/release-gates.md` has an evidence cell for every gate; `python3 scripts/release.py --dry-run` on the release candidate plans 3.0.0; `scripts/check.sh` is green.

**Acceptance Scenarios**:

1. **Given** the 2.27 gate list, **When** every row has evidence, **Then** 2.27 may be cut; the same for 2.28 and 3.0.0.
2. **Given** a gate row without evidence, **Then** the release PR is not opened (the owner's rule, recorded in `tasks.md`).
3. **Given** the release candidate on `dev`, **When** `python3 scripts/release.py --dry-run --json` runs, **Then** `version` is `3.0.0` and `bump` is `major`.

### User Story 2 - An existing user upgrades without losing a working setup (Priority: P1)

**Independent Test**: `scripts/test-migrate.sh` builds fixture homes (a 2.26 package install, a 2.28 install, a git clone install with links, a mixed state) in a throwaway `HOME` and upgrades each to a 3.0.0 candidate over `file://`.

**Acceptance Scenarios**:

1. **Given** a 2.28 package install, **When** the person runs `ai-gent update` (or the installer), **Then** 3.0.0 is installed, `previous` is 2.28.x, the agent links point into `current/`, `ai-gent doctor` is green, and `--rollback` returns to 2.28.x.
2. **Given** a 2.26 or 2.27 install, **When** the person runs its own `scripts/ai-gent-update.sh`, **Then** nothing is written and the person is told to update to 2.28 first or to reinstall with the one-line installer (`contracts/migration-guide.md`).
3. **Given** a git-clone install, **When** the person runs the 3.0.0 installer, **Then** it refuses with the move-aside instructions and `setup.sh --migrate` relinks the skills; the old clone is untouched.
4. **Given** `AI_GENT_MODE=git` set in the environment, **When** the installer runs, **Then** it stops with "git mode was removed in 3.0.0" and the steps (no silent fallthrough to package mode).
5. **Given** a user who called `python3 .../wt.py` from their own scripts, **Then** the migration guide maps each old call to `ai-gent <sub>`.

### User Story 3 - Contributors still edit live (Priority: P1)

**Acceptance Scenarios**:

1. **Given** a checkout, **When** the contributor runs `./setup.sh` from it, **Then** skills link into the checkout (no install root involved) and `scripts/dev-bin.sh` provides `bin/ai-gent` (build or download).
2. **Given** a checkout without a binary, **When** a guard or tool needs it, **Then** it fails with the install command, naming `scripts/dev-bin.sh` (D8).

### User Story 4 - The tier policy is stated once, honestly (Priority: P1)

**Acceptance Scenarios**:

1. **Given** `contracts/tier-policy.md`, **Then** it has a table per feature for Claude, Codex and others, and the README and the docs-site compatibility matrix are generated from or checked against the same table (T031).
2. **Given** a tier-3 agent breaks upstream, **Then** the policy says what the project promises (a fix by PR is welcome, no SLA, not a release blocker) and what it never does (remove an agent in 3.0.0).

### User Story 5 - If 3.0.0 is bad, there is a way back (Priority: P1)

**Acceptance Scenarios**:

1. **Given** a user on 3.0.0, **When** they run `ai-gent update --rollback` (or `sh install.sh --rollback`), **Then** they are on the previous version with working links.
2. **Given** 3.0.0 is defective for new installs, **When** the owner follows the project rollback, **Then** `releases/latest` points at the last 2.x again and 3.0.0 is a pre-release until a 3.0.1 fixes it (`contracts/release-gates.md` rollback section).

## Requirements *(mandatory)*

- **FR-001**: 3.0.0 contains exactly the items in `contracts/breaking-changes.md` under "Added" and removes exactly those under "Removed"; a change not in that file does not ship in 3.0.0 as breaking.
- **FR-002**: The breaking changes are listed with an id (B1-B10), who is affected, what they see, and the migration step; the release notes in `CHANGELOG.md` `## Unreleased` reproduce the list verbatim before the merge to `main`, and the merge includes a `BREAKING CHANGE:` footer so `release.py` plans the major.
- **FR-003**: The package is the only install path: `install.sh` has no git mode and exits with a clear message on `AI_GENT_MODE`, `AI_GENT_BRANCH`, `AI_GENT_REPO`, `AI_GENT_DIR`; links in agent skill directories made by `install.sh` always lead into `.../current/`; `setup.sh` run from a checkout still links the checkout (contributors).
- **FR-004**: The tier policy of `contracts/tier-policy.md` is in force: tier 1 Claude Code, tier 2 Codex, tier 3 opencode, Cursor, Antigravity, GitHub Copilot, Amp, Kiro, Windsurf (best-effort and frozen, D2). Nothing is removed in 3.0.0.
- **FR-005**: `ai-gent` replaces the shell guards (033), the update scripts (034) and the Python tools ported by 035-037 (D7 amended): the subcommands ship in 2.27/2.28 beside the old tools with parity proven in the package, and at 3.0.0 the skills switch to `ai-gent <sub>` and every replaced Python and shell file is deleted in the same release. `dbrun` is not part of that set: its migration is the last step of the roadmap within 3.x (D26, D27; spec 038 and its spike), so `dbrun.py` and the need for `uv` remain in 3.0.0 unless the spike says otherwise.
- **FR-006**: The gates of `contracts/release-gates.md` are measurable (a command, a file or a CI run each) and checked in order: G27 (before 2.27), G28 (before 2.28), G30 (before 3.0.0).
- **FR-007**: Migration: `setup.sh --migrate` keeps its 2.26 behavior and additionally reports leftover removed variables, links the `ai-gent` binary into `~/.local/bin`, and relinks hooks-related paths; `contracts/migration-guide.md` is the user-facing text, shipped in the README and the docs site (D18's Install page).
- **FR-008**: The rollback plan (user-level and project-level) is rehearsed before the 3.0.0 merge: a scripted rehearsal installs 3.0.0 over 2.28.x in a throwaway home and rolls back.
- **FR-009**: Tier-3 support policy (below, and `contracts/tier-policy.md`): best-effort, frozen, outside required CI, tested by a non-required job, never a release blocker.
- **FR-010**: Documentation changes are tasks (Phase 3 of `tasks.md`) against these files: `README.md`, `AGENTS.md`, `CLAUDE.md`, `docs/site/`, `shared/releasing.md`, `shared/secrets.md`, `shared/token-tracking.md`, `shared/containers.md`, `shared/release-platforms.md`, `docs/product/release-3-notes.md`, `CHANGELOG.md`, every `plugins/**` reference to a deleted tool. A grep gate (G30-DOC) proves none remains.
- **FR-011**: Naming (D21): in every file written for the release, the version is 3.0.0 or 3.x and the old short spelling of the major (the letter v followed by the digit) does not appear; a check in `scripts/consistency.py` (T040) enforces it with an allowlist for the D21 text and changelog history; tags have no `v`.
- **FR-012**: The required CI set at 3.0.0 is: the Linux check job, the macOS installer job, a Windows binary/guard job, the six-target build, docs CI. Tier-3 install tests move to a separate, non-required job.
- **FR-013**: `docs/product/rust-decisions.md` open items are all answered before the 3.0.0 merge: signing (032 de-risk), minimum OS, `--version` versus package `VERSION` (answered in 032 data-model), telemetry (none, confirm).

### Key entities

See `data-model.md`: Gate, Breaking change, Tier, Migration path, Doc task.

## Edge cases

- A user on 2.26 skips 2.28: the old updater's download of the 3.0.0 content asset fails before anything is written (the installer writes nothing until the checksum passes); the failure message is a curl error, so the 2.28 notice and the README say "go through 2.28 or reinstall". See open question 2 for a bridge asset.
- The Intel macOS runner label is retired before 3.0.0: the leg becomes cross-built from arm64 (rustup target) with a Rosetta smoke test; recorded as a gate waiver.
- Windows arm64 runner flakiness: a gate allows one retry per leg, not a waiver.
- A plugin's own version after a major commit: `release.py` bumps each plugin by the strongest commit touching its paths, so plugins touched by the breaking commit also move to a new major. Expected; the notes say so.
- Users who pinned `VERSION=2.x`: stay silent and unchanged; `ai-gent update --to latest` clears the pin.

## Out of scope

- The content of the ported tools (035-038) and the docs-site v2 (040); only their gate status is tracked here.
- Removing any tier-3 agent.
- Code signing itself (decided in 032).
- Rewriting history or renaming existing tags (all existing tags are `X.Y.Z`).

## Success criteria *(mandatory)*

- **SC-001**: Every row of `contracts/release-gates.md` for 3.0.0 has evidence before the merge to `main`.
- **SC-002**: `scripts/test-migrate.sh` passes for all fixture homes in CI.
- **SC-003**: `python3 scripts/release.py --dry-run --json` on the candidate prints `"version": "3.0.0"`.
- **SC-004**: `grep -rEn 'wt\.py|token_report\.py|reporun\.py|specsync\.py|compact\.py' plugins shared README.md AGENTS.md docs/site/src` finds only mentions inside the migration guide and the changelog.
- **SC-005**: the check of T040 (`python3 scripts/consistency.py`) is green: the old spelling appears only in the allowlisted D21 text and past changelog entries.
- **SC-006**: The rollback rehearsal restores 2.28.x with green `ai-gent doctor`.
- **SC-008**: On a Windows 11 machine without Git Bash, `irm .../install.ps1 | iex` installs and `ai-gent doctor` is green (032 US7, T058); the release notes of 3.0.0 carry the checksum block (D25).
- **SC-007**: After release, `releases/latest` serves a package that installs on all six targets (the 032 SC-007 check, repeated for 3.0.0).

## Open questions (for the owner)

1. **Answered (D7 amended)**: package installs are decided by the binary from 2.28; clones keep the shell guard; the shell logic, `rules.mjs`, the Python tools and git mode all go at 3.0.0.
2. **Bridge for old updaters.** A 2.26-2.27 install running its own updater against 3.0.0 fails at download (safe, nothing written). Recommended: no bridge asset; rely on the 2.28 notice (`ai-gent update --check` says "3.0.0 is a major release") and the README. Alternative: publish a stub `ai-gent-3.0.0.tar.gz` that only prints the reinstall command; rejected as more surface for little gain.
3. **Support window for 2.28.x.** Recommended: none beyond one patch if the 3.0.0 installer is found broken (solo project, `release.py` releases from `main` only); stated in the README.
4. **Codex tier-2 guard enforcement.** Unverified whether Codex can run hook commands. Recommended: T032 verifies; if yes, Codex gets enforced guards through `ai-gent guard` and tier 2 means "enforced"; if no, tier 2 means "skills and tools tested in CI, guards advisory".
5. **Is `uv` still a prerequisite in 3.0.0?** Yes while `dbrun` stays Python (D26, D27). The README says so plainly ("only the db plugin needs uv and Python") unless spec 038's spike lands a port earlier; the sentence is a G30 gate (G30-16).
6. **Docs-site v2 (040) must ship with 3.0.0** (D18 says "built after the Rust pieces"). Recommended: it is a G30 gate, but a minimal Install page and the compatibility matrix suffice; search can slip to 3.1.
