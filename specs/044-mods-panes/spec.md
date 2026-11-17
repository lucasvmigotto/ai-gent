# Feature Specification: Mods panes say what the system knows (Workers, Worktrees, Pipeline)

**Feature Branch**: one branch per slice: `feat/mods-panes-workers`, `feat/mods-panes-worktrees`, `feat/mods-panes-pipeline`

**Created**: 2026-11-15

**Status**: Draft (awaiting the owner's approval, per CLAUDE.md "Propose first")

**Input**: the owner's brief `~/Downloads/claude-code-mods-tui-implementation.md` (14 Nov 2026, with four mock-ups) and the owner's decisions of 2026-11-14 (listed under Decisions). Code read: `plugins/project/hooks/workers-model.ts`, `workers-draw.tsx`, `pipeline-view.ts`, `pipeline-draw.tsx`, `plugins/agents/hooks/worktrees-model.ts`, `worktrees.ts`, `register.tsx`.

**Depends on**: 022 (Worktrees board), 029 (Pipeline pane), 041 and 042 (Workers pane and its capacity block), all in the optional mods.

**Amends, by pointer, without editing them**: 022 (its `+ahead/-behind` text and the narrow row `branch +a/-b ●`: FR-020 here replaces them), 029 (its section order, header to Next action last, and the always-open Artifacts table: FR-030 to FR-034 here replace them), 041 FR-017 (the ATTENTION and WORKERS lists: FR-001 here). Spec 042 is kept as it is (its `N in use (limit unknown)` wording stays).

## Context and ground rules

- The aim is an operational console: what needs attention, why progress stopped, what is safe next. The brief is a proposal, not a claim that anything is applied. Its screenshot numbers (43 features, `+8/-0`, PR #444, and so on) are fixtures for tests, never hardcoded.
- **No invented host API.** Today the Worktrees pane draws plain `Text` only (`register.tsx`, `ui.render` for `Pane`). `Button` works in a `Pane` (the Workers and Pipeline panes already use it). A `Box` is not drawn inside a `Button`; a Button carries a label or `Text` children only [OBSERVED: `workers-draw.tsx` draws segments in a `Button`; the nesting limit is the owner's rule, host behaviour not verified here].
- **Refresh stays read-only.** No pane writes a file, runs a mutating command or relaunches work when opened or refreshed. Interaction writes `$.state` atoms only (029 FR-013).
- **Next action only fills the prompt** (029). `wt.py remove <branch>` stays a hint in the text, never a button (022).
- Colour never stands alone: every state keeps a glyph or a word, and an ASCII set exists for each new glyph (`↑ ↓ ▸ ▾` have `^ v > v`).
- Unknown is shown as unknown: never `0`, never blank, never a guess.
- A change to a mod needs `/reload-plugins` after the merge (the plugin cache serves the old module until then); each slice's PR says so.
- **Out of scope** (see the end): an events or idle-reason reader, retry buttons, a cross-pane projection.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Workers: a failure appears once (Priority: P1)

A person with 2 failed workers, 32 done and 1 taken back opens the Workers pane and sees the failures under ATTENTION only, not again in the WORKERS list; the counts and the history are unchanged.

**Independent Test**: `workers-model.test.ts` (lines, counts, row budget) and `workers.test.tsx` (drawn rows still press).

**Acceptance Scenarios**:

1. **Given** failed and stalled workers, **When** drawn, **Then** each is a row under `ATTENTION` and in no row of the active list; `WORKERS`, when it has nothing else, says `No running workers`; `countsOf`, the header issue count and the History line are unchanged; `/workers` text still opens every worker once under its section.
2. **Given** a worker with state `failed` and verdict `accepted`, **When** its row is opened, **Then** `State: Failed` and `Verdict: accepted` are separate lines; neither is merged into the other and no success wording appears.
3. **Given** a worker with no `verdict.reason`, **When** opened, **Then** `Cause: not recorded`; the pane invents no cause. A missing `tokens` line is omitted, never `0`.
4. **Given** nothing running and nothing queued, **When** drawn, **Then** `Idle reason: not recorded` (no events reader exists in this effort) and never `all done` or `capacity free`.
5. **Given** a cap of null and 0 in use, **When** drawn, **Then** 042's `0 in use (limit unknown)` is unchanged (no `0/?`, no `capacity unverified`).
6. **Given** a failed row, **When** drawn, **Then** only the glyph and the state word take the error tone; the track, tasks and duration stay neutral or dim, and the row stays one Button (one press target). The tone ranges use `Line.marks` from 042.
7. **Given** two workers whose tracks share a long prefix, **When** collapsed, **Then** the names may be cut; **When** opened, **Then** the `Track:` line shows the full name, wrapped, never cut.
8. **Given** a `done` count of 32 and a row budget, **Then** the budget applies to rows actually shown (the ATTENTION rows no longer use the active list's budget).

### User Story 2 - Worktrees: ahead and behind read as ahead and behind (Priority: P1)

**Independent Test**: `worktrees.test.tsx` (the model's lines at 100 and 50 columns, ASCII).

**Acceptance Scenarios**:

1. **Given** a row with ahead 2 and behind 0, **When** drawn wide, **Then** `↑2 ↓0`; in ASCII `^2 v0`; with either count null, `?` as today. The notes line says once what the arrows mean (`↑ ahead of, ↓ behind the base branch`; ASCII `^ ahead of, v behind the base branch`; the missing-base note reads `no base branch found`).
2. **Given** a narrow pane (under 60), **Then** the row is `branch ↑a ↓b ●`, `branch ? ●` when unknown.
3. **Given** an open pull request whose checks fail, **Then** one `CI failing` signal per row (the `checks failing` cell of `prCell` is dropped, the mark stays); the summary counts it once (`1 failing`). A branch with no GitHub status shows no CI word.
4. **Given** a row, **When** its `▸` Button is pressed, **Then** the row opens in place to full branch, path, dirty, ahead/behind, PR and checks as `Text` lines; pressing again closes it. Rows keep their expansion across refreshes, keyed by branch; a vanished branch drops its key.
5. **Given** a refresh in which `wt.py` fails, **Then** the last rows stay, `wt.py failed: <error>` shows (as today) and `local: cached` shows beside the last good time. **Given** a GitHub read that fails after a success, **Then** the PR cells stay and the note reads `GitHub: cached, read failed at <time>; last good <time>`.
6. **Given** `gh` unusable or a non-GitHub origin, **Then** today's notes stay and no PR or CI state is shown as good.
7. **Given** more rows than `MAX_ROWS`, **Then** `+N more` is exact.

### User Story 3 - Pipeline: exceptions and the next step first (Priority: P1)

**Independent Test**: `pipeline-view.test.tsx` (model: order, counts, auto-open) and `pipeline.test.tsx` (the drawn tree and the press).

**Acceptance Scenarios**:

1. **Given** any snapshot, **When** drawn in the full layout, **Then** the order is header, summary, blockers, **Next action**, Implementation, then Artifacts, Stage readiness, Features, Open items (each collapsed unless FR-032 opens it).
2. **Given** 7 applicable artifacts all ready and 1 not applicable, **Then** the collapsed Artifacts line reads `7/7 applicable ready · 1 N/A`; the denominator already excludes N/A (`notApplicable`), the calculation is unchanged.
3. **Given** a missing or stale artifact, **When** first drawn, **Then** Artifacts is open and the line reads `5/7 applicable ready · 1 missing · 1 stale`; the person's own toggle wins afterwards (an explicit `false` stays closed).
4. **Given** a blocked feature, **Then** it stays in the blockers block with details closed.
5. **Given** the Implementation block, **Then** its percentage is `implemented / total` and the readiness counts are a separate labelled line (`Readiness: 2 ready · 1 blocked · 8 waiting`); the bar segments equal the legend.
6. **Given** Next action with a command, **When** its button is pressed, **Then** the prompt is filled and nothing runs (029).
7. **Given** a blocker and a next action, **Then** the next action shows a dependency link only when the snapshot supports it (`Feature.deps` holds it); otherwise the line is neutral. It never says a dependency is resolved.

### Edge Cases

- Width under 60 and ASCII: every new line has a short form and an ASCII form; the failed indicator is never dropped.
- A bad worker entry (parse failure) is already counted as `skipped`; nothing here may let one entry throw the whole pane (016-style fallback, 029 FR-016).
- The compact Pipeline layout (`pipelineLayout: compact`) and `/pipeline-pane` text keep their text, apart from the reorder of FR-030 where the compact text lists the same sections.
- Idle reason, handoff state, failure cause beyond `verdict.reason`: always `not recorded` until a follow-up spec adds a reader.

## Requirements *(mandatory)*

### Workers (slice 1, `plugins/project/hooks/workers-model.ts`, `workers-draw.tsx`)

- **FR-001**: Workers with `needsAttention` (failed, stalled) are drawn in ATTENTION only; the active list excludes them. `countsOf`, the header tone, the History group, `slotsUsed` (a stalled delegated track still holds a slot) and the totals line do not change.
- **FR-002**: The row budget (`maxRows`) counts shown rows of each group separately; the `+N more` line adds both groups' hidden rows.
- **FR-003**: An opened worker shows `State` and `Verdict` on separate lines, `Cause: not recorded` when `verdict.reason` is null and the state is failed, and omits `Tokens` when null. No other line is added.
- **FR-004**: When no worker is running or queued, an `Idle reason: not recorded` line follows the header block; its wording is one constant, replaced by the follow-up reader.
- **FR-005**: Status tone applies to the glyph and state word only, through `Line.marks` ranges (042 FR-004); the row remains one `Button` of `Text` segments.
- **FR-006**: Spec 042's capacity wording (`N in use (limit unknown)`, `+N subagents`) is kept as it is.

### Worktrees (slice 2, `plugins/agents/hooks/worktrees-model.ts`, `worktrees.ts`, `register.tsx`)

- **FR-020**: Divergence is drawn `↑a ↓b` (ASCII `^a vb`), `?` when unknown; this replaces 022's `+a/-b` and its narrow `branch +a/-b ●`. A note explains the arrows once.
- **FR-021**: One CI-failure signal per row: the `markOf` `failing` mark is the signal; `prCell` no longer adds `checks failing`. `markOf` itself and the `removable` hint (`wt.py remove <branch>`, text only) are unchanged.
- **FR-022**: Rows open and close in place, keyed by branch, through `Button` rows (label text only, no `Box` in a `Button`) and a `$.state` atom `worktreesExpanded` declared in `plugins/agents/types/index.d.ts`. The pane's render becomes a mix of `Text` and `Button`; the plain-text `/worktree-board` output prints every row open.
- **FR-023**: Freshness is kept per source: `attemptedAt`, `okAt` and `error` for the local read and for the GitHub read, in the board. `okAt` moves only on a read that parsed; today's `localAtAtom`/`remoteAtAtom` are written when a refresh starts and are not shown as a last successful read. A failed read keeps the last rows and labels them `cached`.
- **FR-024**: Refresh stays read-only; the polls and their intervals (15 s, 120 s) do not change; one batched GitHub query as today.

### Pipeline (slice 3, `plugins/project/hooks/pipeline-view.ts`, `pipeline-draw.tsx`, `pipeline.ts`)

- **FR-030**: Full-layout order: header, summary, blockers, Next action, Implementation, Artifacts, Stage readiness, Features, Open items. `compute`, `recommend`, `Snapshot` and the poll are untouched (029 FR-001).
- **FR-031**: Artifacts is collapsed by default behind one line `N/M applicable ready · K N/A` (plus `· X missing · Y stale` when any).
- **FR-032**: Artifacts opens by itself when any applicable artifact is missing (`isProblem`) or stale (`staleSince`); a stored toggle (`true` or `false`) overrides the default. Default and override are resolved in the pure view model from the `expanded` map; no write on render.
- **FR-033**: Implementation and readiness are two labelled lines; no figure is computed from the other.
- **FR-034**: Stage readiness keeps its checks; at narrow widths the strip is the one line `Current stage: <name>` with the existing Readiness button.
- **FR-035**: Open-item categories stay as they are (clarifications, assumptions, gaps); no severity is invented.

### Cross-cutting

- **FR-040**: No new dependency; no change to `shared/pipeline.md`; no new `userConfig` key.
- **FR-041**: Each slice's tests cover Unicode, ASCII, narrow (59), medium and wide widths, zero data and a long name.
- **FR-042**: Each slice ends with `claude plugin test` and `claude plugin validate` for its plugin, `scripts/check.sh --quick --quiet` while working and the full `scripts/check.sh` once before the last commit.

## Success Criteria

- **SC-001**: A failed worker appears in exactly one section of the default Workers pane; mutation (revert FR-001) fails a test.
- **SC-002**: The strings `+2/-0`, `checks failing` beside `CI failing`, and `7 of 7 ready` appear nowhere in the three panes' output (grep in the tests).
- **SC-003**: A failed read never moves `okAt` (test with a fake clock).
- **SC-004**: Every new `Button` is pressed in a test and changes only an atom; no test finds a spawn or write during render or refresh.
- **SC-005**: `scripts/check.sh` passes at each slice's end.

## Decisions (owner, chat 2026-11-14)

1. One new spec 044; it amends 022's `+a/-b` text and 029's section order by pointer.
2. Ahead and behind as `↑2 ↓0`, ASCII `^2 v0`.
3. Artifacts collapsed by default as `7/7 applicable ready · 1 N/A`, open when something is missing or stale.
4. Spec 042's `N in use (limit unknown)` wording is kept (the brief's `0/? capacity unverified` is not used).
5. Failed or stalled workers leave the WORKERS list and show in ATTENTION only; counts and history unchanged.
6. No events or idle-reason reader in this effort; the pane says `not recorded`; a follow-up spec adds the reader.
7. Slice order: Workers, Worktrees, Pipeline. Builder: a Sonnet subagent, with an independent Codex review of each slice.

## Out of scope

- An events or idle-reason reader, handoff milestones, `Inspect failure` or `Prepare retry` buttons, token coverage (`7/9 runs`), a cross-pane projection (brief sections 6 and WRK-06/07): a follow-up spec once the producer exists.
- Open-item severity, a stage-strip redesign, new bars, cards, themes.
- Editing specs 022, 029, 041 or 042.

## Open questions for the owner

- Q1: the brief's `Slots 0/? · capacity unverified` conflicts with decision 4; this spec keeps 042's words. Confirm.
- Q2: Worktrees row expansion needs a `Button` per row in a pane that draws `Text` today. Button works in a Pane (Workers, Pipeline) but a Worktrees pane with Buttons is **not verified** in the host; slice 2's first task is a one-test spike, and if it fails the rows stay non-interactive and print open (FR-022 falls back, no inert `▸`).
- Q3: should the Worktrees summary count `CI failing` and `removable` as now, or drop `removable`? This spec keeps both.
