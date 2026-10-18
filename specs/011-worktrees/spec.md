Reconstructed by project:introspec on 2026-10-18 from 384c771

# Feature Specification: Worktrees for parallel tracks (wt.py)

**Feature Branch**: `docs/introspec-site-runners` (reconstructed; the code already exists on `main`)

**Created**: 2026-10-18

**Status**: Draft

**Input**: Reverse-engineered from `plugins/agents/scripts/wt.py` (241 lines) and `plugins/agents/scripts/test_wt.py`.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - See which features can start in parallel (Priority: P1)

`wt.py plan` reads `specs/README.md` and lists features with a side still below Implemented whose dependencies are all Implemented, the ones waiting and on what, and how many are done.

**Independent Test**: `test_plan`.

**Acceptance Scenarios**:

1. **Given** README rows with a 3-digit first cell, a `Depends on` column and side columns (`frontend`, `backend`, `status`, `qa`), **When** `plan` runs, **Then** each row is ready, blocked or done [OBSERVED: plugins/agents/scripts/wt.py:87-147].
2. **Given** a side marked N/A or —, **When** planned, **Then** it isn't a side to build [OBSERVED: wt.py:110-115].
3. **Given** no `specs/README.md`, **When** `plan` runs, **Then** it fails with "name the tracks yourself" [OBSERVED: wt.py:89-90].

---

### User Story 2 - Start a track in its own worktree (Priority: P1)

`wt.py add <branch>` creates a new branch off the chain's dev branch in `.worktrees/<slug>/`, excluded from git locally, and prints a compose project name and a port offset for that track.

**Independent Test**: `test_add_list_remove`, `test_from_inside_a_worktree`.

**Acceptance Scenarios**:

1. **Given** exactly one of `dev`/`develop` (local or origin), **When** `add` runs without `--base`, **Then** it is the base; with both or neither it fails asking for `--base` [OBSERVED: wt.py:52-58,150-152].
2. **Given** an existing branch or path, **When** `add` runs, **Then** refused [OBSERVED: wt.py:154-157].
3. **Given** a new track, **When** added, **Then** `/.worktrees/` and `compose.worktree.yaml` are appended to `.git/info/exclude` (no project file changes) [OBSERVED: wt.py:61-68].
4. **Given** the n-th worktree, **When** added, **Then** `COMPOSE_PROJECT_NAME=<repo>-<branch>` and port offset `n × 100` are printed [OBSERVED: wt.py:159-166].
5. **Given** it runs inside a worktree, **When** any command runs, **Then** the main checkout is used as root [OBSERVED: wt.py:42-45].

---

### User Story 3 - List and remove tracks (Priority: P2)

`wt.py list` shows each worktree under `.worktrees/` with ahead/behind against dev and whether it's dirty; `wt.py remove <branch>` removes a clean, fully landed worktree and deletes its branch.

**Independent Test**: `test_add_list_remove`, `test_refusals`.

**Acceptance Scenarios**:

1. **Given** uncommitted changes, **When** `remove` runs, **Then** refused [OBSERVED: wt.py:204-205].
2. **Given** commits not on dev, **When** `remove` runs, **Then** refused: "land it first" [OBSERVED: wt.py:207-208].
3. **Given** a clean, landed track, **When** `remove` runs, **Then** worktree removed, branch deleted with `-d`, worktrees pruned [OBSERVED: wt.py:209-213].

### Edge Cases

- The port offset is the count of current worktrees + 1, so after a removal a new track can reuse a live track's offset (3 tracks, remove the 1st, add → offset 300 twice) [INFERRED: wt.py:158 → port collisions between parallel stacks; confirm with add ×3, remove first, add].
- `plan` reads `specs/README.md` from the main checkout, not from the worktree it runs in: run from this branch's worktree (which has the README), it failed with "no specs/README.md" because the main checkout has none [OBSERVED: `python3 -I plugins/agents/scripts/wt.py plan` on 2026-10-18; cause wt.py:42-45,88]. Specs that exist only on a branch are invisible to it.
- Only worktrees under `.worktrees/` are listed; agent worktrees elsewhere (such as `.claude/worktrees/`) are invisible to `list` and `remove` [OBSERVED: wt.py:71-82].

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: A track MUST start on a new branch; existing branches are refused.
- **FR-002**: wt.py MUST NOT change tracked project files.
- **FR-003**: `remove` MUST refuse dirty or unlanded tracks.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: `test_wt.py` passes [OBSERVED: `scripts/check.sh --quick` 2026-10-18 → "agents wt.py (Ran 4 tests)" ok].

## Assumptions

- [ASSUMPTION: `agents:parallel` is the only caller.]
