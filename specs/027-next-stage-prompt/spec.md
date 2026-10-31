# Feature Specification: Next-stage prompt

**Feature Branch**: `docs/mod-specs-2` (specification only; built on its own branch)

**Created**: 2026-10-31

**Status**: Accepted

**Input**: A Claude Code mod that, when a pipeline stage writes its artifact (a brief, an architecture, a `specs/README.md`, a feature's `tasks.md`), toasts the next stage to run, reusing the pipeline pane's recommendation instead of a second set of rules. Grounded in `plugins/project/skills/status/SKILL.md` §4, `shared/pipeline.md`, and `plugins/project/hooks/pipeline-model.ts` (`recommend`, `ARTIFACTS`) and `pipeline.ts` (`compute`, whose snapshot already carries `next`).

**Depends on**: 013 (pipeline pane) — `recommend`, `compute` and the artifact list; 012 for the module. Implemented.

## Context and ground rules

- A mod is an optional, Claude Code-only layer. `project:status` stays the report of record and `shared/pipeline.md` the contract; the toast is a hint that names a stage and the reason, and it never runs the stage, fills the prompt or submits anything.
- It adds no rules. The recommendation is the snapshot's `next` (`compute($).next`, which is `recommend(artifacts, features, markers.items)` from `pipeline-model.ts`, `pipeline.ts:158`), so 013 and 027 cannot disagree; a change to the rules is a change there, with its parity test. What `recommend` returns is what the toast says, including `/project:status` when no rule fires.
- Owner: the `project` plugin; code beside 013's, composed from `register.tsx`. It works whether the pane is open or not.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Told what comes next when a stage finishes (Priority: P1)

After a stage writes `specs/README.md` with a feature that needs planning, a toast reads `specs index written — next: /project:spec (001-foo needs plan)`. After `project:init` writes only `docs/product/brief.md`, `recommend` returns no rule (rule 2's "missing" needs a later artifact to exist, `pipeline-model.ts:175-206`) and the toast reads `brief written — next: /project:status (nothing is ready to start; the full report covers QA, docs and CI)`.

**Why this priority**: a stage ends with the model saying what is next; the person who stepped away, or a headless run, never sees it. The pane shows it only when open.

**Independent Test**: a `claude plugin test` case mounts the module on a fixture tree from `pipeline-fixtures.ts`, raises an Edit/Write completion on `specs/README.md` with `$.fs` answering the new tree, and asserts one toast whose stage and reason equal the snapshot's `next`, on `terminal` and `desktop`.

**Acceptance Scenarios**:

1. **Given** a Write to a path in `ARTIFACTS` (or under `specs/NNN-*/`) that was absent before, **When** the call completes, **Then** the snapshot is recomputed with `compute($)` and, when its `next` differs from the one recorded, a toast names the artifact written, the recommended stage and its reason.
2. **Given** the recommended stage is the same as before the write, **When** it completes, **Then** no toast (nothing new to say).
3. **Given** an Edit that only changes an existing artifact, **When** it completes, **Then** no toast unless the recommended stage changed.
4. **Given** the recommendation is rule 1 (an open `[NEEDS CLARIFICATION]`), **When** a write creates it, **Then** the toast says `ask the user` with `file:line`, as the pane does.
5. **Given** no rule fires (`next.rule` is null, `stage` is `/project:status`, "nothing is ready to start"), **When** a write completes, **Then** one toast says so, once, and points to `/project:status`.
6. **Given** the session just started, **When** the first watched write completes, **Then** "the one before" is the `next` recorded at session start (`nextStageLast`, seeded by a detached `compute` that toasts nothing), so the first change is judged against something.
7. **Given** an Edit adds a `[NEEDS CLARIFICATION]` and no artifact appears, **When** it completes and `next` becomes rule 1, **Then** the toast says `ask the user` with `file:line` (maintainer's decision, 2026-10-31).

---

### User Story 2 - One toast per change, not a stream (Priority: P1)

During a build stage that writes many files under `specs/`, the person is not toasted for each.

**Acceptance Scenarios**:

1. **Given** ten Writes under `specs/004-x/` in one turn, **When** the turn completes, **Then** at most one toast, for the final recommendation, if it differs from the one before the turn.
2. **Given** a toast was shown for a stage, **When** the same stage is recommended again later with the same reason, **Then** it is not repeated within the session.

---

### User Story 3 - Off switch and surfaces (Priority: P3)

`nextStageToast` is a `userConfig` boolean (default true), shown in `/config`.

**Acceptance Scenarios**:

1. **Given** `nextStageToast` off, **When** a stage writes an artifact, **Then** nothing is computed.
2. **Given** a headless run (`claude -p`), **When** a stage writes an artifact, **Then** nothing shows (a toast has no surface); the module registers nothing that draws.

### Edge Cases

- **No pipeline project** (no `docs/product/`, no `specs/`): nothing runs; the first-write case (the brief itself) is the one that starts the pipeline and does toast.
- **Cost**: `compute` lists and stats the artifacts and reads or spawns only what changed by `mtimeMs` (`pipeline.ts:28-45`: `markersByFile`, `dateByPath`, `readme` are module-level caches, so `compute` is read-only toward the project but not free of module state; a reload empties them and the next call refills). It runs only on a write to a watched path, at most once per turn.
- **The pane is open**: 013's existing `tool.call` Edit/Write hook (`register.tsx:158-166`) already refreshes the snapshot atom on a path containing `/docs/product/` or `/specs/`; 027 extends that hook (one trigger, not a second hook) and reuses the atom's snapshot when `computedAt` is newer than the write, and recomputes otherwise.
- **Writes by another tool** (a Bash redirect, a script, `specsync.py`): not seen as tool writes; the turn-complete check compares `mtimeMs` of the watched paths and covers them (at most one `stat` per path).
- **Orchestrated runs** (`agents:orchestrate`, `agents:parallel`): many writers; the once-per-turn rule applies and the toast names only the final recommendation.
- **Hot reload**: the last recommendation and the shown set live in `$.state` atoms.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The mod MUST NOT write any file, run a stage, fill or submit a prompt; it only toasts text.
- **FR-002**: The recommendation MUST be `compute($).next` from `pipeline.ts` (that is `recommend()` from `pipeline-model.ts`); a second implementation of `project:status` §4 is forbidden, and the mod never calls `recommend` separately.
- **FR-003**: A toast MUST fire only when the recommended stage or reason differs from the last one recorded, and at most once per turn.
- **FR-004**: It MUST label itself a hint and point to `/project:status` for the full report, as the pane does.
- **FR-005**: It MUST do nothing while `nextStageToast` is false, without touching 012's or 013's parts of the module.
- **FR-006**: Triggers MUST be limited to the paths the existing Edit/Write hook matches (`e.file_path` containing `/docs/product/` or `/specs/`, 013's watched set), plus an `mtimeMs` check at turn end. The mod extends that hook rather than adding a second `tool.call` hook for the same tools.
- **FR-007**: The mod MUST seed `nextStageLast` at `session.start` with a detached `compute` that raises no toast.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: `claude plugin test plugins/project` passes, with stories 1–3 on `terminal` and `desktop`.
- **SC-002**: For each of the pane's rule fixtures (`pipeline-fixtures.ts`), the toast's stage and reason equal the snapshot's `next`, including the no-rule case (`/project:status`).
- **SC-003**: Ten writes in one turn produce one toast.
- **SC-004**: `claude plugin validate plugins/project` reports no refusal.

## Out of scope

- Running or queuing the next stage; filling the prompt (the pane's Next button does that on press; a toast has no button).
- Changing the recommendation rules, `project:status`, or `shared/pipeline.md`.
- A native notification (`$.ui.notify`): a toast is enough for a hint.
- Other agents (opencode, Codex, …).

## Assumptions

- [ASSUMPTION: `compute($)` writes nothing to the project or to atoms (it reads and refills its module caches; `refresh` writes the atom), as `pipeline.ts` has it today.]
- [ASSUMPTION: Claude Code 2.1.312's `tool.call` Edit/Write payload carries `file_path`, as 013 already reads it.]

## Decisions (2026-10-31, with the maintainer)

- Toast also on an edit that adds a `[NEEDS CLARIFICATION]` with no new artifact. No session-start toast (that is `project:status`).
- Truthful examples: the toast quotes what `snapshot.next` holds. Writing only the brief yields `/project:status`, not `/project:architecture`, because 013's rule 2 reports a missing artifact only when a later stage's artifact exists. Changing that would be a change to 013's rules and `project:status` §4 (out of scope here).
- `compute($).next` is used; `recommend` is not called separately. The trigger extends 013's existing Edit/Write hook (`register.tsx:158-166`).
- `nextStageLast` is seeded at session start without a toast (decided 2026-10-31 (the maintainer accepted the default): the alternative is to treat the first change of a session as always worth a toast).
- Should the brief-only case say `/project:architecture` anyway (a rule change in 013 and `project:status`)? decided 2026-10-31 (the maintainer accepted the default): no; keep one source of rules.
