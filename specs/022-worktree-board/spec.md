# Feature Specification: Worktree board

**Feature Branch**: `docs/mod-specs-2` (specification only; built on its own branch)

**Created**: 2026-10-31

**Status**: Accepted

**Input**: A Claude Code mod that keeps `agents:parallel`'s worktrees in view: one row per `.worktrees/<branch>` with ahead/behind against the chain's dev, whether it has uncommitted changes, and, when `gh` is signed in, its pull request and CI state. Grounded in `plugins/agents/scripts/wt.py list --json` and `plugins/agents/skills/parallel/SKILL.md`.

**Depends on**: 012 (checkpoint mod) — module layout, `check.sh` wiring, versioning and install; 013 (pipeline pane) — the pane pattern (open state in `$.state`, `$.clock.every` poll, a command that opens it). The module is `agents`' (shared with 014).

## Context and ground rules

- A mod is an optional, Claude Code-only layer. `wt.py` and `agents:parallel` stay the authority on worktrees; the board reads and never adds, removes, merges, pushes or lands anything. When a row looks removable it says which command would remove it (`wt.py remove <branch>`) and does not run it.
- Owner: the `agents` plugin. Module `plugins/agents/hooks/register.tsx`, whose `hooks.json` 014 created; the board's code sits beside it.
- Where there are no worktrees (no `.worktrees/`), the board has nothing to show and never opens by itself.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - See every track at a glance (Priority: P1)

`/worktree-board` opens a pane with one row per worktree: branch, `+ahead/-behind` against the dev branch, a dirty mark, and the age of the last commit.

**Why this priority**: during a wave the coordinator and the user ask "where is each track" every few minutes; today it is a command and a manual read.

**Independent Test**: a `claude plugin test` case mocks `$.process.run` to answer `wt.py list --json` with three rows and finds each row's text by key, on `terminal` and `desktop`.

**Acceptance Scenarios**:

1. **Given** `wt.py list --json` rows with `branch`, `path`, `dirty`, `ahead`, `behind`, **When** the board opens, **Then** each branch is listed with its counts and a mark for `dirty`, in the script's order.
2. **Given** a row with `ahead` and `behind` null, **When** drawn, **Then** it shows `?` for the counts and says once `no dev branch found`. `wt.py list --json` reports no `base` key (`cmd_list` computes `base` and prints it only in the text form), so the board cannot name the dev branch or give any other reason; it says only that the counts are unknown.
3. **Given** a row with `ahead` 0 and `dirty` false, **When** drawn, **Then** it is marked "nothing of its own". **Given** a row whose pull request (story 2) is `MERGED` and `dirty` is false, **When** drawn, **Then** it hints `wt.py remove <branch>` once. The hint rests on the merged state, not on `ahead`: after a rebase merge `ahead` stays above 0, which is why `wt.py remove` itself decides with `git cherry` (`wt.py` `cmd_remove`); `wt.py remove` still refuses a branch with changes the dev branch lacks, so a wrong hint cannot lose work. With no `gh`, no hint.
4. **Given** no worktrees, **When** the command runs, **Then** it answers "no worktrees under .worktrees/".

---

### User Story 2 - Pull request and CI state per track (Priority: P2)

When `gh` is signed in and the remote is GitHub (read from `git remote get-url origin`, a fourth read-only spawn, FR-001), each row gains its pull request (number, draft/open/merged) and its check rollup (passing, failing, pending).

**Why this priority**: it turns "is track 77 done" into a glance, and shows a red check before the coordinator tries to land the branch.

**Independent Test**: `$.process.run` answers `gh auth status` (exit 0) and one `gh pr list --json …`; the test asserts each row's PR text and that exactly one `gh pr list` ran.

**Acceptance Scenarios**:

1. **Given** `gh auth status` succeeds, **When** the board refreshes, **Then** one `gh pr list --state all --limit 100 --json number,headRefName,state,isDraft,statusCheckRollup` is run and rows are matched by `headRefName`.
2. **Given** a branch with no pull request, **When** drawn, **Then** its PR cell reads `no PR` (a branch not pushed yet shows `not pushed`).
3. **Given** a failing check, **When** drawn, **Then** the row is marked failing and the pane's summary line counts failing tracks.
4. **Given** `gh` is missing or not signed in, **When** the board opens, **Then** PR and CI columns are omitted and one line says `gh auth login` would add them; local columns still show.

---

### User Story 3 - The board follows the wave (Priority: P2)

While open, the local columns refresh on a poll and after any Bash call that runs `wt.py`, `git commit` or `git merge`; the PR and CI columns refresh more slowly.

**Independent Test**: advance the mocked clock past each interval and assert which commands ran.

**Acceptance Scenarios**:

0. **Given** the board open, **When** a Bash call whose command runs `wt.py`, `git commit` or `git merge` completes, **Then** `wt.py list --json` runs once at once (outside the local interval, never overlapping a run in flight) and `gh` does not.
1. **Given** the board open, **When** the local interval (default 15 s) elapses, **Then** `wt.py list --json` runs again, and `gh` does not.
2. **Given** the board open, **When** the remote interval (default 120 s) elapses, **Then** `gh pr list` runs again.
3. **Given** the board closed, **When** time passes, **Then** nothing is spawned.

### Edge Cases

- **Many worktrees**: `wt.py list` spawns git per worktree; the interval is a floor, a run in flight is never overlapped, and the pane caps rows with "+N more".
- **A worktree on a detached HEAD**: `wt.py` prints `(detached)`; the row shows it and skips the PR lookup.
- **Not a GitHub remote** (GitLab, Azure DevOps, …): no PR columns in the first cut; the line says the platform is not read yet.
- **`wt.py` fails** (not a repository, exit non-zero): the pane says so with the script's first stderr line and keeps the last good rows.
- **Narrow terminal**: below `bodyColumns` 60, a row collapses to `branch +a/-b ●`.
- **Hot reload**: open state, last rows and last refresh times live in `$.state` atoms; the rows are read again after a reload.
- **Headless (`claude -p`)**: the command answers the rows as text.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The board MUST NOT write any file or run anything that changes a worktree, branch, remote or pull request; its only spawns are `wt.py list --json`, `git remote get-url origin`, `gh auth status` and `gh pr list`, all reads.
- **FR-002**: Local columns MUST be exactly what `wt.py list --json` reports (`branch`, `path`, `dirty`, `ahead`, `behind`); the mod computes no counts of its own.
- **FR-003**: `gh` MUST be called at most once per remote refresh (never per row), only when `gh auth status` exits 0, and its output MUST NOT be logged beyond what the pane shows; no token is read.
- **FR-004**: The board MUST register `/worktree-board` (not `/parallel`: that name belongs to the `agents:parallel` skill, as 013 found for `/pipeline`) and open it unasked never.
- **FR-005**: A `userConfig` boolean `worktreeBoard` (default true) MUST turn the feature off without touching 014's parts of the module; `worktreeBoardSeconds` sets the local interval (default 15) and `worktreeBoardRemoteSeconds` the remote one (default 120). `register.tsx` is restructured so `worktreeBoard(on, options)` is composed on its own `worktreeBoard` check, ahead of 014's `tokenStatus` return: `tokenStatus` off does not turn the board off, and the board off does not touch 014, 018, 019 or 020.
- **FR-006**: State that drawing reads MUST be in `$.state` atoms declared in `plugins/agents/types/index.d.ts`, never in module variables (`data-model.md` lists them).
- **FR-007**: The pull request's check rollup MUST map `statusCheckRollup` as `data-model.md` states (failing, pending, passing, none); the mod computes no other state.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: `claude plugin test plugins/agents` passes, with stories 1–3 covered on `terminal` and `desktop`.
- **SC-002**: On this repository during a wave, the board's rows equal `wt.py list --json` for every worktree.
- **SC-003**: With the board open for 10 minutes and no changes, `gh pr list` runs at most six times at the default interval (the run on opening, then one every 120 s).
- **SC-004**: `claude plugin validate plugins/agents` reports no refusal.

## Out of scope

- Adding, removing or finishing worktrees, landing branches, re-running CI (`agents:parallel`, `repo:pr`, `devsecops:watch`).
- PR and CI for platforms other than GitHub in the first cut; per-job CI detail (`devsecops:watch`).
- Container labels and disk (`wt.py resources`; see 025).
- Other agents (opencode, Codex, …).

## Assumptions

- [ASSUMPTION: `wt.py list --json` keeps its row keys (`plugins/agents/scripts/wt.py`, `cmd_list`); a parity fixture in the tests pins them.]
- [ASSUMPTION: Claude Code 2.1.312's `Pane`, `$.ui.open` and `$.clock.every` stay as declared (013 relies on the same).]

## Decisions (2026-10-31, with the maintainer)

- `gh` is called directly, gated by `gh auth status`, not through `reporun`: the board is in `agents`, and `reporun` is `repo`'s script. 023 (in `repo`) goes through `reporun`.
- A mod's `$.process.run` is the engine's `process.run` event, not a Bash tool call, so `plugins/repo/hooks/guard.sh` (a `PreToolUse` guard on Bash) does not see it. The calls are reads. Confirmed from the types only; the build adds a test that it works with the guards installed.
- decided 2026-10-31 (the maintainer accepted the default): the removable hint rests on the merged pull request state and a clean tree, not on `ahead` (a rebase merge keeps `ahead` above 0).
- decided 2026-10-31 (the maintainer accepted the default): the platform is read from `git remote get-url origin` (GitHub only in the first cut), so that spawn is part of FR-001.
- decided 2026-10-31 (the maintainer accepted the default): the `statusCheckRollup` mapping in `data-model.md`.
- `worktreeBoard` is independent of `tokenStatus` (FR-005).
