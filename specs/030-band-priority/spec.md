# Feature Specification: Band priority

**Feature Branch**: `docs/mod-specs-2` (specification only; built on its own branch)

**Created**: 2026-10-31

**Status**: Accepted

**Input**: The mods draw bands above the prompt: the checkpoint holds (012, `project`), the repo hygiene band (017, `git`), and the planned CI watch (023, `repo`), container and disk (025, `devcontainer`) and release readiness (028, `git`). Together with the token status line and toasts the prompt area gets crowded. At most two bands are visible, chosen by a declared priority, and the rest are one command away.

**Depends on**: 012, 017. Read by 023, 025, 028.

## Context and ground rules

- A band is a `ui.render` hook on `{ component: 'AbovePrompt' }` (`plugins/project/hooks/register.tsx:100`, `plugins/git/hooks/register.tsx:54`). Today each draws independently; plugins cannot see each other's trees, so the cap cannot be enforced by one drawer.
- Decision (2026-10-31): the rule is enforced the way `guard-ux.ts` is shared: one code file in `shared/`, copied into each hooks module that draws a band, with `scripts/consistency.py --sync-shared-code` and a check that fails if a copy differs (CLAUDE.md, layout rules).
- The mod stays optional and read-only. A band's own wording and data are unchanged; only whether it is drawn changes.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - At most two bands, the most urgent first (Priority: P1)

**Why this priority**: five bands would push the prompt down; holds and failures must never be the ones hidden.

**Independent Test**: a `claude plugin test` case per plugin with fixtures declaring bands of each priority and asserting which draw.

**Acceptance Scenarios**:

1. **Given** three active bands of priorities `hold`, `normal` and `info`, **When** the prompt area draws, **Then** the `hold` and `normal` bands draw and the `info` band does not.
2. **Given** a `failure` band and a `hold` band and a `normal` band, **When** drawn, **Then** `hold` and `failure` draw.
3. **Given** two bands of the same priority, **When** drawn, **Then** the one that became active earlier draws first (stable order by activation time).
4. **Given** one active band, **When** drawn, **Then** it draws as today.

### User Story 2 - See what was hidden (Priority: P2)

**Independent Test**: command test.

**Acceptance Scenarios**:

1. **Given** a hidden band, **When** the person runs `/bands`, **Then** the text lists every active band with its plugin, priority and whether it is shown.
2. **Given** a hidden band, **When** the prompt area draws, **Then** one dim line reads `+1 more band (/bands)`.

## Requirements *(mandatory)*

- **FR-001**: Priorities, highest first: `hold` (a checkpoint hold or anything that asks before an action), `failure`, `normal`, `info`. Each band declares exactly one.
- **FR-002**: Each plugin records its active bands in its own `$.state` atom (`bands`: id, priority, since). A plugin that depends on the others reads them through `dependencies`, as 024 does; a plugin not installed is simply absent from the count.
- **FR-003**: A shared `band-priority.ts` (Planned, `shared/`) holds the pure rule: `visible(bands, cap)` returns the ids to draw. It is copied into each hooks module that draws a band; `scripts/consistency.py` fails if a copy differs.
- **FR-004**: `userConfig` `bandCap` (default 2, range 1–5) and `bandsCommand` (default on).
- **FR-005**: Never hide a `hold` band: if more than `bandCap` `hold` bands exist, all of them draw.
- **FR-006**: A band that is hidden still raises its toasts; the cap applies to the persistent area only.

## Out of scope

- Reordering by the person; a combined single band; the status line and toasts' own limits.

## Decisions (2026-10-31, with the maintainer)

- A cap of two; the rule is a shared code file copied into each module, not a new plugin. The `info` band (028) is the first to give way.
- Cross-plugin visibility uses the `dependencies` contract as in 024 (R1 there: what `$.state.get` does for an absent plugin is probed first).
- Open for the build, not for the maintainer: the order of two bands drawn by different plugins is the engine's, so the cap is advisory across plugins unless every band plugin reads the others' atoms; the plan's first task probes this.
