# Feature Specification: Guard UX

**Feature Branch**: `docs/mod-specs` (specification only; built on its own branch)

**Created**: 2026-10-28

**Status**: Implemented

**Input**: Better presentation of what the git, db, secrets, repo and devcontainer guards decide — the rule behind a deny, the safe alternative, and a clearer question on an ask — through Claude Code mods, with each `plugins/<name>/hooks/guard.sh` still the only decider. Includes a research decision on importing `opencode/guard/rules.mjs` (see `research.md`).

**Depends on**: 012 (checkpoint mod) — module layout, `check.sh` wiring, versioning and install, reused unchanged.

## Context and ground rules

- The shell guards decide; `opencode/guard/rules.mjs` mirrors them for OpenCode and `opencode/guard/test/rules.test.mjs` (run by `scripts/check.sh`) proves the two agree. The mod changes neither and adds no third decision path: it reads the decision the guard already made and presents it.
- A mod MUST never turn a deny into anything else, and MUST never turn an ask into an allow unless the person answered allow in a dialog (and only if research R2 shows that is safe; otherwise asks stay the engine's own prompt).
- Owners: each guard's plugin — `git`, `db`, `secrets`, `repo`, `devcontainer`. One module per plugin (`plugins/<name>/hooks/register.tsx`), each handling only reasons its own guard writes (`git:workflow guard:`, `db guard:`, `secrets guard:`, `repo guard:`, `containers guard:`). The common code is one file, `shared/guard-ux.ts`, symlinked into each of the five plugins.
- Without the mod (other agents, or the opt-out off), every guard behaves exactly as today.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A deny says what to do instead (Priority: P1)

When a guard denies, the person sees a short card naming the guard, the rule, the skill that explains it and the command to use instead, and the model receives the guard's reason unchanged plus one line of context pointing at the alternative.

**Why this priority**: today a deny is one dim line; the person often doesn't see why the agent changed course.

**Independent Test**: a `claude plugin test` case raises `$.tool.call({ tool: 'Bash', command: 'git push --force' })` with a test `classic.PreToolUse` hook beneath answering the guard's JSON decision as `{ deny: 'git:workflow guard: force pushes use --force-with-lease, …' }`; it asserts the deny still ends the call, the reason text is unchanged, and a toast or transcript notice names `git:workflow` and `--force-with-lease`.

**Acceptance Scenarios**:

1. **Given** `secrets guard: … pass show …` denies, **When** the mod sees the result, **Then** the person is shown "secrets guard — values never reach the agent; ask the user to run `pass insert ai-gent/<area>/<name>` in their terminal (`secrets:pass`)", and the call stays denied.
2. **Given** a deny whose reason has no entry in the explanation table, **When** shown, **Then** a generic card with the guard's own reason is shown (never nothing, never a guess).
3. **Given** two guards answered (say db asks, secrets denies), **When** the result reaches the mods, **Then** the deny stands and each plugin's mod presents only its own guard's part.

---

### User Story 2 - An ask is a clear question (Priority: P1)

When a guard asks (`git push`, `gh pr create`, `git branch -D`, a direct `psql`, `pass rm`, …), the permission prompt carries a fuller reason: what the command would do, why the guard asks, and the request the person must have made for "allow" to be right.

**Independent Test**: test `classic.PreToolUse` beneath answers `{ ask: 'git:workflow guard: git push publishes; …' }`; assert the result is still an `ask` whose text carries the rewritten reason, with the original reason included verbatim.

**Acceptance Scenarios**:

1. **Given** `git push` on a feature branch, **When** the git guard asks, **Then** the prompt reads "Publish `feat/x` to `origin`? The git:workflow guard asks because a push publishes; allow only if you asked for this push." followed by the guard's reason.
2. **Given** the opt-in dialog is on (`userConfig` `guardDialog`, default false) and research R2's conditions hold, **When** a guard asks, **Then** `$.ui.ask` offers `Allow once`, `Deny`, `Deny and tell Claude why`; Allow returns allow for this call only, Deny returns the guard's reason as the deny, the third adds the person's typed text.
3. **Given** the dialog is dismissed, or the session is headless (`$.ui.ask` rejects), **When** that happens, **Then** the mod returns the guard's original ask unchanged.

---

### User Story 3 - The same text everywhere (Priority: P2)

The explanation table lives once, in `shared/guard-ux.ts`, keyed by guard prefix and reason, so the five plugins say things the same way and a guard reason change shows up in a test.

**Acceptance Scenarios**:

1. **Given** a new `decide deny|ask "<reason>"` line in any `plugins/*/hooks/guard.sh`, **When** `claude plugin test` runs, **Then** a test reading that plugin's `hooks/guard.sh` fails until the reason has a table entry or is listed as generic.

### Edge Cases

- **Guard doesn't run** (not Bash, matcher misses): the mod sees no decision and does nothing.
- **Guard times out or errors** (no decision): passed on untouched; the mod never fills in a decision.
- **A mod hook fails**: with a `.catch` that answers `next.called ? next(e) : …` replaying what the guards beneath settled, so a failing mod never changes a decision (it can only lose its decoration).
- **Headless (`claude -p`) and the SDK**: no dialog; only the reason text is enriched.
- **Narrow terminal**: the card text is at most three lines of 60 characters; longer guidance goes to the transcript notice.
- **Non-terminal surfaces**: toasts and the engine's own permission dialog exist on all; no custom drawing is used.
- **Hot reload**: nothing kept in module variables; the table is code, decisions are per call.
- **Several guard plugins installed but only some mods enabled**: each plugin's opt-out is separate (`userConfig` `guardUx`).

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The decision MUST come from the guards themselves: the mod reads what `next(e)` returns at `classic.PreToolUse` (research R1); if the engine doesn't expose command-hook results there, the fallback is to run the plugin's own `hooks/guard.sh` with the call's JSON on stdin via `$.process.run` and use only its printed decision (research R1, fallback). Never `rules.mjs` (research R3).
- **FR-002**: A deny MUST stay a deny with the guard's reason text unchanged for the model; the mod MAY add `additionalContext` with the alternative and MUST show the person a toast or notice.
- **FR-003**: An ask MUST stay an ask, with the guard's reason included verbatim in the enriched text, unless FR-004 applies.
- **FR-004**: The dialog path (`$.ui.ask`) MUST be off by default and MAY only be enabled if R2 confirms an `allow` returned from `classic.PreToolUse` does not skip permission rules (`deny` rules in settings, managed policy) the person has set.
- **FR-005**: Each plugin's module MUST act only on reasons with its own guard prefix.
- **FR-006**: Every reason string in `plugins/{git,db,secrets,repo,devcontainer}/hooks/guard.sh` MUST map to an explanation entry or be marked generic, enforced by test.
- **FR-007**: The shared file MUST be `shared/guard-ux.ts`, copied into each of the five plugins' `hooks/` and imported as `./guard-ux`; `scripts/consistency.py` fails if a copy differs or if a guard reason has no entry (FR-006); CLAUDE.md's layout rule is amended with this feature (shared hooks-module code is copied into `hooks/`, not symlinked).
- **FR-008**: No change to any `guard.sh`, `rules.mjs`, the shell guard tests or the parity test.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: `claude plugin test` passes for all five plugins, each with a deny case, an ask case, a fail-safe case (hook throws) and the reason-coverage case.
- **SC-002**: `scripts/test-guard.sh`, `test-db-guard.sh`, `test-secrets-guard.sh`, `test-repo-guard.sh`, `test-containers-guard.sh` and `opencode/guard/test/rules.test.mjs` pass unchanged.
- **SC-003**: With every mod disabled, the `--debug` log shows the same decisions for the parity vectors as with them enabled.

## Out of scope

- New rules, or any change in what is denied or asked.
- OpenCode's guard UX (`opencode/guard/index.ts`).
- Making `rules.mjs` the Claude Code decider.

## Decisions (2026-10-28, with the maintainer)

- Asks keep the engine's own permission prompt with a fuller reason; the opt-in Allow/Deny dialog (FR-004) is out of the first cut and waits for research R2.
- The shared module `shared/guard-ux.ts` is symlinked into each owning plugin's `hooks/` (imported as `./guard-ux`). CLAUDE.md's layout rule is amended with this feature: shared files go into `references/`, except shared hooks-module code, which goes into `hooks/`.

- Found while building (R4, verified): the engine refuses an import through a symlink that leaves the plugin (`cannot import … no such file under <plugin>`), so the shared file is copied, not linked, and `--sync-shared-code` plus a consistency check keep the copies identical. This is the fallback research R4 named.
- Found while building (R1, verified on 2.1.312 with a probe mod and a headless run): at `classic.PreToolUse`, `next(e)` returns the guard's decision: `{"ask":"git:workflow guard: git push publishes; …"}`, and for a deny `{"deny":"PreToolUse:Bash hook error: git:workflow guard: hooks must not be bypassed …"}`. The reason is wrapped on a deny, so the module finds the guard's marker anywhere in the text. A module's event carries `tool` and `command` (not `tool_name`).
- Found while building: the test kit shows a decorated deny but not a decorated ask (tool.check carries no reason), so the ask wording is covered by exact-text tests of `decorate`, and the hook-level test checks only that an ask still goes through. A hook that throws is covered by the `.catch` replay and the validator, not by a test (a toast failing does not propagate).
- R2 (a dialog's allow) was not needed: the dialog is out of this cut by decision, so asks stay the engine's own prompt.

## Assumptions

- [ASSUMPTION: Plugin command hooks (`hooks/hooks.json` `"command"` entries) run beneath hooks modules in the `classic.PreToolUse` chain, as `ClassicEventOf`'s doc in the 2.1.312 types describes for "the other settings hooks"; R1 verifies it.]
- [ASSUMPTION: The guard reason prefixes stay as they are in the five `guard.sh` files.]
