Reconstructed by project:introspec on 2026-10-18 from 384c771

# Feature Specification: Installer (install.sh and setup.sh)

**Feature Branch**: `docs/introspec-site-runners` (reconstructed; the code already exists on `main`)

**Created**: 2026-10-18

**Status**: Draft

**Input**: Reverse-engineered from `install.sh` (POSIX sh, 84 lines), `setup.sh` (bash, 862 lines) and `scripts/test-install.sh`.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - One-line install and update (Priority: P1)

A developer runs `curl -fsSL …/install.sh | sh`. The installer clones the toolkit (or updates the existing clone) and runs `setup.sh`, which links every skill and plugin for Claude Code.

**Why this priority**: the toolkit's only install path.

**Independent Test**: `scripts/test-install.sh` "clones into the default directory", "links point into the clone", "re-run updates in place".

**Acceptance Scenarios**:

1. **Given** no clone, **When** `install.sh` runs, **Then** it clones `AI_GENT_REPO` into `AI_GENT_DIR` (default `${XDG_DATA_HOME:-~/.local/share}/ai-gent`) and runs `setup.sh` with the same arguments [OBSERVED: install.sh:41-79; scripts/test-install.sh:231-233].
2. **Given** a clean clone on a branch, **When** it runs again, **Then** it fast-forwards and reports `updated A → B` or `already at A` [OBSERVED: install.sh:53-64; test-install.sh:235].
3. **Given** local changes in the clone, **When** it runs, **Then** it doesn't update and links what's there [OBSERVED: install.sh:47-48; test-install.sh:238].
4. **Given** `AI_GENT_DIR` exists and isn't a clone, **When** it runs, **Then** it stops with an error [OBSERVED: install.sh:66-67; test-install.sh:240].
5. **Given** a partial download, **When** `sh` runs it, **Then** nothing executes, because all work is in `main()` called on the last line [OBSERVED: install.sh:16-17,84].

---

### User Story 2 - Pin a release (Priority: P2)

A user sets `AI_GENT_BRANCH=<tag>` to pin the clone; later runs keep the pin until a new value moves it.

**Independent Test**: `test-install.sh` "clones at the tag", "stays pinned without AI_GENT_BRANCH", "moves the pin to a newer tag".

**Acceptance Scenarios**:

1. **Given** `AI_GENT_BRANCH=9.9.0`, **When** installing fresh, **Then** the clone is at that tag [OBSERVED: install.sh:71-72; test-install.sh:254].
2. **Given** a detached (pinned) clone and no `AI_GENT_BRANCH`, **When** it runs, **Then** it stays pinned and says so [OBSERVED: install.sh:51-52; test-install.sh:256].
3. **Given** a pinned clone and a newer tag in `AI_GENT_BRANCH`, **When** it runs, **Then** it fetches and checks out that tag detached [OBSERVED: install.sh:26-31; test-install.sh:258].

---

### User Story 3 - Install for other agents (Priority: P1)

A user picks agents with `--target` (or in the selector): opencode gets symlinks, generated entry files and `/<plugin>-<skill>` commands; Codex, Cursor, Gemini CLI, Copilot and Amp share generated entries in `~/.agents/skills`; Kiro and Windsurf get their own.

**Independent Test**: `test-install.sh` lines 60-126.

**Acceptance Scenarios**:

1. **Given** `--target claude`, **When** setup runs, **Then** each `skills/<n>/` and `plugins/<n>/` is symlinked into `CLAUDE_SKILLS_DIR` [OBSERVED: setup.sh:365-381; test-install.sh:63].
2. **Given** opencode is chosen, **When** setup runs, **Then** each plugin skill gets a generated `<plugin>-<skill>/SKILL.md` pointing at its source and a command file; forked skills (`context: fork`) become `subtask: true` commands [OBSERVED: setup.sh:389-473,621-678; test-install.sh:64-72].
3. **Given** several shared-directory agents, **When** setup runs, **Then** one pass writes one entry per skill into `AGENTS_SKILLS_DIR`, as files not symlinks [OBSERVED: setup.sh:84,834-856; test-install.sh:119-121].
4. **Given** non-Claude, non-opencode agents, **When** setup finishes, **Then** it notes that the guards are instructions only there [OBSERVED: setup.sh:706; test-install.sh:124].
5. **Given** `--target nope`, **When** setup runs, **Then** it exits 2 [OBSERVED: setup.sh:141-146; test-install.sh:132].
6. **Given** a choice was made, **When** setup runs later with `--skip-prompt` or without a terminal, **Then** the remembered choice is used (first time: Claude Code, plus opencode if an earlier run generated its entries) [OBSERVED: setup.sh:725-734,817-832; test-install.sh:126-151].

---

### User Story 4 - Safe, idempotent re-runs and uninstall (Priority: P1)

Re-running changes nothing that is already right, removes links and entries whose source is gone, never overwrites what it didn't create without `--force` (which backs it up), and `--uninstall` removes only what it installed.

**Independent Test**: `test-install.sh` lines 58-113, 153-162.

**Acceptance Scenarios**:

1. **Given** an installed toolkit, **When** setup re-runs, **Then** nothing is linked or generated [OBSERVED: test-install.sh:81].
2. **Given** `--dry-run`, **When** setup runs, **Then** nothing changes [OBSERVED: setup.sh:148-156; test-install.sh:58].
3. **Given** a real directory where a link should go, **When** setup runs without `--force`, **Then** it is kept and the skip reported; with `--force` it is moved to `<name>.bak-<timestamp>` and linked [OBSERVED: setup.sh:310-340; test-install.sh:110-113].
4. **Given** a generated entry directory without the `.ai-gent-generated` marker, **When** setup runs, **Then** it is left alone [OBSERVED: setup.sh:399].
5. **Given** `--uninstall`, **When** setup runs, **Then** links into the repo, generated entries and commands are removed, the user's own files are kept and the remembered choice is forgotten [OBSERVED: setup.sh:343-352,474-488; test-install.sh:155-162].

---

### User Story 5 - opencode guard configuration (Priority: P2)

With opencode chosen, setup offers ([Y/n]) to merge the OpenCode V2 permission rules mirroring the guard hooks, the guard plugin and `skill` denies for the short colliding IDs into `opencode.json`; `--yes` applies, `--no-config-edits`, `--dry-run` or no terminal print the snippet instead; uninstall removes only what it added.

**Independent Test**: `test-install.sh` lines 164-226.

**Acceptance Scenarios**:

1. **Given** no terminal and no `--yes`, **When** setup runs, **Then** `opencode.json` is untouched and the rules are printed [OBSERVED: setup.sh:506-518; test-install.sh:184-186].
2. **Given** the user's conflicting rule, **When** merging, **Then** the user's rule is kept and the conflict reported; missing rules are appended after the user's and a backup is kept [OBSERVED: test-install.sh:192-199].
3. **Given** a config with comments, **When** setup runs, **Then** the file is left alone and the snippet printed [OBSERVED: test-install.sh:214-215].

---

### User Story 6 - Choose agents interactively (Priority: P3)

On a terminal, setup shows an fzf selector (a numbered prompt without fzf) with Claude Code and the previous choice preselected and a "found" column per agent.

**Acceptance Scenarios**:

1. **Given** a terminal, **When** setup runs without `--target`/`--skip-prompt`, **Then** the selector opens; choosing nothing exits 1 with "no agent chosen" [OBSERVED: setup.sh:740-810]. **No test** (needs a TTY).

### Edge Cases

- `setup.sh` requires bash with associative arrays (`declare -A`, bash ≥ 4) [OBSERVED: setup.sh:71]; `install.sh` checks only that `bash` exists [OBSERVED: install.sh:39] [INFERRED: macOS's stock bash 3.2 would fail at line 71; confirm on macOS — the README's supported platforms weren't checked].
- `--target` as the last argument with no value: `shift` past the end under `set -e` ends the script silently [INFERRED: setup.sh:127,139 → exit without a message; confirm with `setup.sh --target`].
- `--uninstall --target codex` prunes the whole shared `~/.agents/skills` entry set, so Cursor, Gemini CLI, Copilot and Amp lose their entries too [INFERRED: setup.sh:680-689 prunes the directory, not per agent; confirm whether that is intended].

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The installer MUST never execute a partially downloaded script.
- **FR-002**: Setup MUST be idempotent and MUST NOT replace anything it didn't create unless `--force`, which backs it up.
- **FR-003**: Each agent's directory MUST be overridable by its environment variable (`CLAUDE_SKILLS_DIR`, `OPENCODE_SKILLS_DIR`, `OPENCODE_COMMANDS_DIR`, `AGENTS_SKILLS_DIR`, `KIRO_SKILLS_DIR`, `WINDSURF_SKILLS_DIR`) [OBSERVED: setup.sh:48-53].
- **FR-004**: The agent choice MUST persist in `${XDG_STATE_HOME:-~/.local/state}/ai-gent/targets`.
- **FR-005**: opencode config edits MUST be asked, reversible and never touch a file with comments.

### Key Entities

- **Agent**, **Generated entry**, **Targets state**, **Config record** — see `data-model.md`.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: `scripts/test-install.sh` (70 checks) passes in CI [OBSERVED: release run 39066868067 on 0d34208 ran `check.yml`, which runs it, and succeeded].

## Assumptions

- [ASSUMPTION: Linux is the supported platform; the shared-directory prune on uninstall is intended.]
