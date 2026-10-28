# Feature Specification: Checkpoint holds in the session (project mod)

**Feature Branch**: `feat/checkpoint-band`

**Created**: 2026-10-28

**Status**: Draft

**Input**: "Project and implement #1": the first Claude Code mod in ai-gent. It surfaces and enforces the holds `project:checkpoint` records, because today they are only text the next agent has to remember.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - See the holds in force (Priority: P1)

While `.ai-gent/checkpoint.md` lists holds, a band above the prompt shows them, so neither the user nor the agent loses track of a decision in force.

**Independent Test**: `the band lists the holds, on every surface`, `the band stays quiet without holds`, `the band stays quiet without a checkpoint` (`plugins/project/hooks/checkpoint.test.tsx`).

**Acceptance Scenarios**:

1. **Given** a checkpoint whose `## Holds` has top-level bullets, **When** the session starts or a turn completes, **Then** the band shows `Holds`, the file and its `Updated` time, and up to three holds, with `+N more` for any others.
2. **Given** no checkpoint, an empty `## Holds` or only `- none`, **When** the prompt is drawn, **Then** no band is drawn.
3. **Given** a commit newer than the file, **When** the band is drawn, **Then** it says the file is older than the last commit (the warning `checkpoint.py show` gives).
4. **Given** the user presses Hide, **When** the prompt is drawn again, **Then** the band stays hidden for the rest of the session.

---

### User Story 2 - Ask before a step a hold may cover (Priority: P1)

A go-ahead never carries over from an earlier session (`project:recap`). While any hold is listed, a push, merge, tag or release asks the user first, naming the holds.

**Independent Test**: `a push under a hold asks, and Stop refuses it`, `a push under a hold runs once the person says so`, `without holds, or for a read-only command, nothing is asked`.

**Acceptance Scenarios**:

1. **Given** holds, **When** the agent runs `git push`, `git merge`, `git tag <name>` (or `-a`/`-s`/`-f`/`-d`), `gh pr merge` or `gh release create`, in any segment of a compound command, **Then** the user is asked whether to stop or run it, and the question lists the holds.
2. **Given** the answer Stop, a dismissed dialog, or a run with nobody to ask, **When** the gate answers, **Then** the command is refused, and the agent reads that holds are in force and that it should ask the user.
3. **Given** no holds, or a read-only command (`git tag`, `git tag -l`, `git merge --abort`, `gh pr list`), **When** it runs, **Then** nothing is asked.

---

### User Story 3 - Turn either part off (Priority: P3)

`holdsBand` and `holdsGate` are `userConfig` booleans (default true), shown in `/config`.

**Independent Test**: `the gate can be turned off`.

### Edge Cases

- The session runs in a subdirectory: the file is looked up at the git top level, as `checkpoint.py` does. Outside git, it is looked up in the session's directory.
- The file is rewritten during a turn: the gate re-reads it before asking, so a hold lifted mid-turn no longer asks.
- A hook reload resets the module's variables. The parsed checkpoint lives in `$.state` with a shape tag, so a reload keeps the band.
- The gate fails (git missing, unreadable file): it lets the command through. The `git` guard still asks on every push.
- The `git` guard also asks before a push, so a push under a hold asks twice. This is deliberate: the two questions are about different things.

## Requirements *(mandatory)*

- **FR-001**: The mod reads `.ai-gent/checkpoint.md` only. It never writes it; `checkpoint.py` and the skill stay its only writers.
- **FR-002**: Holds and rules are the top-level `- `/`* ` bullets of `## Holds` and `## Rules in force`. Indented lines continue the bullet above, and HTML comments and empty markers (`none`, `n/a`, `—`) are ignored.
- **FR-003**: The mod is Claude Code-only and optional. Nothing in `shared/pipeline.md`, `project:checkpoint` or `project:recap` depends on it.
- **FR-004**: It lives in the plugin it serves: `plugins/project/hooks/register.tsx`, named under `modules` in `hooks/hooks.json`, with its state contract in `plugins/project/types/index.d.ts`. It installs through the existing `setup.sh` link.

## Success Criteria

- **SC-001**: `claude plugin validate plugins/project` and `claude plugin test plugins/project` pass in `scripts/check.sh` and in CI.
- **SC-002**: In a live session with a hold in the checkpoint, the band shows the hold, and `git push` asks before the `git` guard's own question.

## Out of scope

Writing the checkpoint, matching a hold's text to a particular command (every listed hold gates every gated command), and opencode or the other agents.

## Conventions this feature settles for later mods (013–017)

The mod's folder is its owning plugin. `scripts/check.sh` runs `claude plugin test` for every plugin whose `hooks.json` names `modules`. CI pins Claude Code to 2.1.312 or later. `.gitignore` ignores `plugins/*/.claude-plugin/types/`. Opt-outs are `userConfig` booleans. Functions that take `$` are declared at the module's top level, as the validator requires. A pure helper (`hooks/checkpoint.ts`) carries the parsing so it can be unit-tested.
