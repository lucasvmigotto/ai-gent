# Feature Specification: Hand-off nudge

**Feature Branch**: `docs/mod-specs-2` (specification only; built on its own branch)

**Created**: 2026-10-31

**Status**: Accepted

**Input**: A Claude Code mod that, when the session is about to run out of room (the context window filling, a context compaction about to happen, or — when `token_report.py` is present — the usage caps nearly spent), tells the person the hand-off file is stale and to run `project:checkpoint` while there is still room to write it. Grounded in `plugins/project/skills/checkpoint/SKILL.md`, `.ai-gent/checkpoint.md` (012), the engine's `$.session.usage()` and `session.compact` event, and, optionally, `plugins/agents/scripts/token_report.py budget --json` (014).

**Depends on**: 012 (checkpoint mod) — it reads the same `checkpoint` atom and module layout. Optionally 014 (token status) — the `budget --json` shape (`caps[]` with `name`, `cap`, `used`, `left`, `plugins/agents/hooks/token-model.ts:86`); without `agents` installed the cap story is off and the rest works. Both are implemented.

**Engine**: the CI pin is Claude Code 2.1.312. On 2026-10-31 a probe mod using `session.compact` (matcher `trigger`), `session.append` and `$.session.usage()` passed `claude plugin validate` under 2.1.312, so no pin bump is needed; the build adds a `claude plugin test` case that raises each event, to confirm at runtime what validation only reads.

## Context and ground rules

- A mod is an optional, Claude Code-only layer. `project:checkpoint` stays the only writer of `.ai-gent/checkpoint.md` (`checkpoint.py` its tool); this mod never writes it, never runs the skill, and never submits a prompt on the person's behalf. It says what to run.
- It exists for the case `project:recap` handles after the fact: a session that ran out of tokens or crashed without a hand-off. The nudge moves that warning to before the stop.
- Owner: the `project` plugin. Its code sits beside 012's in `plugins/project/hooks/` and is composed from `register.tsx`.
- The mod measures nothing itself: the context figure is `$.session.usage().context.percent` (the engine's own, the status line's `context_window`), and the cap figures, when present, are what `budget --json` says is left (014's `Budget.caps`). `token_report.py` lives in the `agents` plugin; `project` does not depend on it (the optional check looks for it beside the plugin and is silent when absent).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Warned when the context is nearly full (Priority: P1)

When the context window passes the configured percent, a toast says how stale the checkpoint is: `Context 82% full. The checkpoint is 3 commits older than HEAD — run /project:checkpoint before the conversation is compacted.`

**Why this priority**: a compaction summary loses the user's exact requests and standing rules that the checkpoint keeps verbatim; the context figure needs no script and no sibling plugin.

**Independent Test**: a `claude plugin test` case mocks `$.session.usage()` (context percent 82), `git rev-list --count` and `git status --porcelain`, with a fixture `checkpoint` atom, completes a turn and asserts one toast with the figures.

**Acceptance Scenarios**:

1. **Given** `context.percent` at or above `handoffNudgeContextPercent` (default 80) and a checkpoint last written before three later commits, **When** a turn completes, **Then** a toast gives the percent and "3 commits since the checkpoint" and names `/project:checkpoint`.
2. **Given** the same state, **When** the next turns complete, **Then** no second toast for the context condition (once per session).
3. **Given** no checkpoint at all, **When** the percent passes, **Then** the toast says there is no checkpoint and to run `/project:checkpoint`.
4. **Given** the `checkpoint` atom's `isStale` is false and `git status --porcelain` is empty, **When** the percent passes, **Then** the toast says the checkpoint is current and nothing more is needed (informational, once). `isStale` alone only says a commit is newer than the file (`plugins/project/types/index.d.ts`); the working-tree check is a separate read-only `git status --porcelain`.
5. **Given** `$.session.usage()` fails, **When** a turn completes, **Then** nothing shows and nothing retries within that turn.

---

### User Story 2 - Warned before a compaction (Priority: P1)

When a context compaction is about to happen, the same toast fires once, because the summary loses what the checkpoint keeps.

**Independent Test**: a test raises `session.compact` with `trigger: 'auto'` and asserts the toast, and with `trigger: 'precompute'` asserts none.

**Acceptance Scenarios**:

1. **Given** the engine announces a compaction (`session.compact`, `trigger` `manual` or `auto`), **When** it fires, **Then** the toast fires once per session with story 1's wording, without the percent, and the hook passes the event on unchanged (`next(e)`, never `skip`).
2. **Given** `trigger` is `precompute` (the engine computing ahead; it installs nothing) or `plugin`, **When** it fires, **Then** no toast.
3. **Given** the story-1 toast already fired this session, **When** a compaction follows, **Then** no second toast.

---

### User Story 3 - Warned before the usage cap runs out, when `agents` is present (Priority: P2)

When a cap in `.ai-gent/budget.yaml` has less than the configured share left and `token_report.py` is found, a toast says so and says how stale the checkpoint is: `Session cap nearly spent (0.3M of 4M left). The checkpoint is 3 commits older than HEAD — run /project:checkpoint.`

**Independent Test**: `$.process.run` mocked to answer `budget --json` (a cap at 7% left), a fixture checkpoint, asserts one toast with the figures; and with the script absent asserts no spawn.

**Acceptance Scenarios**:

1. **Given** `caps` with `session_billable` 7% left, the default `handoffNudgeCapPercent` of 10, and a checkpoint last written before three later commits, **When** a turn completes, **Then** a toast names the cap, what is left, and "3 commits since the checkpoint".
2. **Given** the same state, **When** the next turns complete, **Then** no second toast for that cap (once per cap per session).
3. **Given** no `.ai-gent/budget.yaml`, or `token_report.py` not found beside the plugin, **When** a turn completes, **Then** nothing runs and nothing shows.

---

### User Story 4 - Tune or silence it (Priority: P3)

`handoffNudge` (boolean, default true), `handoffNudgeContextPercent` (number, default 80) and `handoffNudgeCapPercent` (number, default 10, the share of a cap left) are `userConfig` values shown in `/config`. The two thresholds mean different things (context used versus cap left), so they are separate.

**Acceptance Scenarios**:

1. **Given** `handoffNudge` off, **When** the context is full or a cap is low, **Then** the module spawns nothing and reads nothing.
2. **Given** `handoffNudgeContextPercent` 60, **When** the context is at 65%, **Then** the toast fires.
3. **Given** `handoffNudgeCapPercent` 25, **When** a cap has 20% left, **Then** the toast fires.

### Edge Cases

- **Several caps** (`session_billable`, `week_billable`, the names `parse_budget` documents): each is checked and toasted once; the toast names the tightest first.
- **Cost**: `$.session.usage()` is cheap and runs at turn end. `budget` reads a week of transcripts (`shared/token-tracking.md`), so the cap check is detached and throttled (default every 5 minutes, never on `session.start`), and skipped when 014's own refresh already has a verdict in its atom less than that old (014's atoms are the `agents` plugin's; the mod reads them only if `agents` is installed and declares them as a dependency, otherwise it runs its own throttled call).
- **Commit count**: commits since the file's modification time (`git rev-list --count --after=<mtime ISO> HEAD`, a new read-only call); the checkpoint's `Updated` line is not used, because the file's `mtimeMs` is what 012 already trusts. The atom's `isStale` is only a boolean (a commit is newer than the file), not a count, and carries no working-tree state, which is why `git status --porcelain` is also called.
- **No git / not a checkout**: the commit count is dropped; the toast still says the checkpoint's age.
- **Checkpoint under a hold**: unchanged; holds are 012's concern, the toast does not list them.
- **Sibling plugin not found** (`agents` not installed, or installed elsewhere): the cap check is off for the session, silently; stories 1, 2 and 4 are unaffected. The script path is `${$.plugin.root}/../agents/scripts/token_report.py`, probed with `$.fs.exists`; a marketplace install that does not place plugins side by side simply has no cap check.
- **Reload**: which toasts fired lives in a `$.state` atom, so a hot reload does not repeat them.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The mod MUST NOT write `.ai-gent/checkpoint.md` or any other file, MUST NOT run `project:checkpoint`, and MUST NOT submit a prompt.
- **FR-002**: The context figure MUST be `$.session.usage().context.percent`; cap figures, when the cap check runs, MUST come from `token_report.py budget --json --projected=0` (the form 014 uses for "caps alone"). The mod computes no token figure of its own.
- **FR-003**: The mod MUST reuse 012's `checkpoint` atom (path, `mtimeMs`, `isStale`) instead of reading the file again, and MUST get the commit count and the working-tree state from read-only `git rev-list --count` and `git status --porcelain`.
- **FR-004**: Each condition (the context, a compaction, each cap) MUST toast at most once per session; the record lives in a `$.state` atom.
- **FR-005**: The `budget` run MUST be detached from `session.start` and throttled (`$.clock.after(0, …)`), as 014's first read is, and MUST NOT run unless `token_report.py` is found and `.ai-gent/budget.yaml` exists.
- **FR-006**: The toast MUST name the command to run (`/project:checkpoint`) and MUST NOT claim the checkpoint is wrong, only that commits came after it.
- **FR-007**: `handoffNudge` MUST turn the whole feature off without touching 012's or 013's parts of the module.
- **FR-008**: The `session.compact` hook MUST answer `next(e)` unchanged on every trigger, MUST NOT return `skip`, and MUST toast only for `manual` and `auto`.
- **FR-009**: `project` MUST NOT list `agents` under `dependencies` for this feature.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: `claude plugin test plugins/project` passes with stories 1–4 covered on `terminal` and `desktop`, including a case that raises `session.compact` and one that mocks `$.session.usage()`.
- **SC-002**: In a session with the context at 85% and a checkpoint two commits old, exactly one toast appears, naming `/project:checkpoint`; a cap at 5% left adds no second toast for the context condition.
- **SC-003**: With no `budget.yaml` or no `token_report.py`, the module spawns no `token_report.py` process in a 10-minute session.
- **SC-004**: `claude plugin validate plugins/project` reports no refusal.

## Out of scope

- Writing or refreshing the checkpoint (that is `project:checkpoint`; an automatic write was rejected: the file is a deliberate hand-off, rewritten at milestones).
- Estimating context size or tokens from transcripts (`$.session.usage()`, `token_report.py` and the engine own measurement).
- Other agents (opencode, Codex, …) and non-Claude-Code surfaces beyond what a toast already reaches.

## Assumptions

- [ASSUMPTION: Claude Code 2.1.312's `$.process.run`, `$.clock.after`, `$.ui.toast`, `$.state`, `$.session.usage()` and the `session.compact` event stay as declared; the API is early access. Validation under 2.1.312 passed on 2026-10-31; the `claude plugin test` cases confirm runtime behavior.]
- [ASSUMPTION: `budget --json --projected=0` prints the caps with no wave, as 014's fallback in `plugins/agents/hooks/tokens.ts` relies on.]

## Decisions (2026-10-31, with the maintainer)

- The kit has what the first draft doubted: the `session.compact` event (with a `trigger`: `manual`, `auto`, `plugin`, `precompute`) and `$.session.usage()` with `context.percent` (types of Claude Code 2.1.316; a probe passed `claude plugin validate` under the CI pin 2.1.312, so no CI bump). The nudge triggers on context percent and on `session.compact`; the compaction story is buildable and is now P1.
- The usage-cap check is optional: it runs only when `token_report.py` is found beside the plugin, and is silent otherwise. No dependency on `agents` (FR-009).
- `handoffNudgePercent` is split into `handoffNudgeContextPercent` (context used, default 80) and `handoffNudgeCapPercent` (cap left, default 10), because the two thresholds measure opposite things. Defaults (80 and 10) accepted by the maintainer 2026-10-31.
- The first toast of a session is not suppressed by an early checkpoint; a current checkpoint gets the informational toast once (decided 2026-10-31 (the maintainer accepted the default): the alternative is silence when `isStale` is false and the tree is clean).
- Review note: `caps[].left` and the cap names `session_billable` and `week_billable` exist (`token-model.ts:86`, `parse_budget`'s docstring), contrary to the first review's doubt.
- Verified 2026-10-31 by running it: `token_report.py budget --json --projected=0` prints `verdict`, `caps` (empty with no caps set), `cards` and `batches` and exits 0, so the optional cap check has the shape it assumes. The 2.1.312 binary contains the `session.compact`, `session.usage` and `precompute` strings.
