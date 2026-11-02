# Feature Specification: Agent delegation (`agents:delegate`)

**Feature Branch**: `docs/spec-041-agent-delegation` (specification only; built in 3.x)

**Created**: 2026-11-02

**Status**: Draft

**Input**: the owner's request (2026-11-02), issue #364, and the first Codex trial (`trial-1-codex.md`).

**Approved**: the owner approved building it on 2026-11-02 ("Go ahead with Rust implementation"). The tools are built in the Rust binary as `ai-gent delegate` (adapters, brief builder, supervisor checks, status files, log), with a thin skill on top; milestone stays 3.x for the release, work proceeds on `dev` behind no hook or skill switch (D7).

**Depends on**: `agents:orchestrate`, `agents:parallel`, `plugins/agents/scripts/wt.py` (spec 036 for its Rust port), the usage meters, `shared/token-tracking.md`. Milestone: 3.x (after 3.0.0).

## Context and ground rules

- Claude Code stays the one place where development is planned, decided, reviewed and landed. Other code agents the user already has (Codex first) **extend the work force**: they take tasks Claude would otherwise do, so more gets built at once and Claude's tokens last longer. Claude is also a worker: it builds, and it keeps the judgment-heavy work.
- **Optional and off by default.** With no other agent detected, or no opt-in, the skill says nothing and nothing changes.
- **No credentials are read, copied or stored.** Each agent keeps its own login; the skill only checks that the agent runs.
- **No markers.** Delegated work carries no trailer, signature or "built by" line in commits or pull requests (owner decision, 2026-11-02). Who built what lives in the local delegation log, not in git history.
- Delegates run **no ai-gent guards** (Codex: "instructions only", README). Enforcement is the brief's hard limits plus the supervisor's checks on the diff; neither is a substitute for the other.
- Only the supervisor (Claude) pushes, opens pull requests, checks CI and merges.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Offer and opt in (Priority: P1)

A user with Claude Code and Codex installed asks for a feature, or is close to a usage limit, or has a task list with independent tracks. Claude offers delegation once, says which agents it found and what they would do, and the user opts in per project.

**Independent Test**: with only Claude present, the skill never offers. With Codex detected and no opt-in, it offers at the three triggers (a request, a usage meter near its limit, parallel-safe tasks) and then records the answer in `.ai-gent/delegate.yaml`.

**Acceptance Scenarios**:
1. **Given** no other agent is detected, **Then** the skill is silent.
2. **Given** Codex is detected and the project has no opt-in, **When** a trigger fires, **Then** one offer names the agent, the tasks it would take and what it cannot do (no guards, no network).
3. **Given** the user declined, **Then** the skill does not offer again in that project until asked.

### User Story 2 - Plan the pool (Priority: P1)

Given a task list (a spec's `tasks.md` or the user's list), Claude splits it into tracks that touch disjoint files, assigns each to the best-fit worker (Claude, a Claude subagent or a delegate), and keeps judgment-heavy work (hooks, shared references, cross-cutting changes) for itself.

**Independent Test**: a fixture task list with overlapping files yields tracks with no shared path; a task needing credentials, containers or an owner decision is never assigned to a delegate.

### User Story 3 - Run a delegate safely (Priority: P1)

Claude prepares a worktree, writes the brief from the template, starts the adapter, watches it, and gets back a diff, not a decision.

**Independent Test**: the adapter's preflight proves commits work and stdin is closed before the task starts; a delegate that touches a path outside `allowed_paths` is rejected before review.

### User Story 4 - Verify, review and land (Priority: P1)

Claude runs the verification checklist outside the delegate's sandbox, has a **different** agent review the diff, and lands it with the project's merge rules.

**Independent Test**: a diff with a trailer, a touched protected file, new prose about environment failures, or a failing check is refused with the reason.

### User Story 5 - Take work back (Priority: P2)

A delegate that fails, hits a limit or stalls loses the task to Claude (or another worker). When Claude is the one rate-limited, queued tasks move to the delegates.

### User Story 6 - See what it cost (Priority: P2)

One line per delegation in the local log: worker, task, wall time, tokens when the agent reports them, result.

### User Story 7 - Follow the workers in a pane (Priority: P3)

A Claude Code user opens `/workers-pane` (or `/workers` for text) and sees one row per worker: agent (Claude, a Claude subagent, Codex), role (builder, reviewer, researcher), task and track, state (queued, running, stalled, done, failed, taken back), elapsed time, last-event age and worktree. A header shows the pool: worktree slots used against the cap, the disk-floor margin and the usage meters. Selecting a row shows its brief, its last events and the supervisor's verdict; cost lines come from the delegation log. The pipeline pane also gets one compact line, for example `Workers: 2 running · 1 done · slots 1/1`, marked when a worker is stalled or failed; the full list stays in the workers pane so the pipeline pane keeps its rows. It is a mod in the `project` plugin, built like the guards and pipeline panes, optional, and never opens by itself.

**Independent Test**: a pure model (status files in, rows and the compact line out) and a draw function with golden fixtures, then `claude plugin test plugins/project`; with no status files the pane says "No workers" and `/workers` prints the same text where no pane can dock.

## Requirements *(mandatory)*

- **FR-001**: Off by default; offers only when another agent is detected (the `setup.sh` probes) and at one of three triggers: the user asks, a usage meter is near its limit, or parallel-safe tasks exist.
- **FR-002**: The opt-in is per project, in `.ai-gent/delegate.yaml` (git-excluded): the agents allowed and the task kinds each may take. **ai-gent's own development uses Codex, and from 2026-11-02 18:50 also a second Claude Code profile as a worker** (owner decision): this repository's allowlist is `[codex, claude-profile]`, whatever the feature supports. A Claude profile runs headlessly with its own config directory (`CLAUDE_CONFIG_DIR=<profile> claude -p`); ai-gent never reads or copies its credentials.
- **FR-003**: The feature supports **every code agent that can be run non-interactively in a worktree**, through the adapter contract (`contracts/adapter.md`). The first two adapters are **Codex** (verified in trials 1, 2 and 4) and **OpenCode** (facts verified in trial 3, `contracts/opencode.md`); further agents (for example Amp, Cursor's CLI, Gemini CLI, Copilot's CLI, Kiro, or Claude Code under another profile) are added by writing an adapter and passing its trial, and are listed as supported only after that. Which agents may be used in a project is that project's choice (FR-002).
- **FR-004a**: **Assignment policy (owner, 2026-11-02).** Rust development and anything with real design judgment (core libraries, the binary's subcommands, hooks, shared references, cross-cutting changes) stays with Claude. Delegates get **simple, well-bounded tasks**: research and surveys, documentation drafts, fixtures and test data, small scripts, mechanical edits, first-pass reviews. Trial evidence supports it: delegates produced sound work on bounded tasks (trials 1-3) and each needed review for something a bounded checklist would catch. The scheduler classifies each task (simple or judgment) and never assigns a judgment task to a delegate unless the owner says so.
- **FR-004**: A delegate never receives a task that needs credentials, database or container tools, an owner decision, or an edit to a skill, plugin, hook or shared reference.
- **FR-005**: Every delegation uses a brief built from `contracts/brief-template.md`, with `allowed_paths`, hard limits, the verification commands, the worker pack and the final report format.
- **FR-006**: Worktrees come from `wt.py`; the scheduler counts Claude's and delegated tracks together against the worktree cap and the disk floor. It never overrides the floor.
- **FR-007**: Delegates edit the working tree and stop; they have no network and, for Codex, cannot write `.git`. The supervisor stages by explicit path, commits, pushes and opens the PR.
- **FR-008**: The supervisor commits (or accepts the delegate's commits) only with the project's message rules and **no marker** of any kind.
- **FR-009**: Verification runs outside the delegate's sandbox: the full `scripts/check.sh` and every CI leg. A delegate runs only the task's targeted tests.
- **FR-010**: Review by a **different agent** than the builder is mandatory for delegated work; the reviewer reads the diff and the spec, not the builder's report.
- **FR-011**: The supervisor rejects, automatically and before review, a diff that touches paths outside `allowed_paths`, adds prose about environment or sandbox failures, adds a forbidden trailer, or changes tests to make them pass.
- **FR-012**: A delegate's own credentials files are never read, copied or logged; the skill never pools or shares accounts.
- **FR-013**: Usage is read from the agent's final event where it reports it (Codex: `turn.completed.usage`) and wall time from the adapter's own clock. The delegation log (`.ai-gent/delegation.log`, git-excluded) holds one JSON line per run (FR in US6); token counts are tagged with the existing token vocabulary (`shared/token-tags.json`, a `delegate` tag added together with `shared/token-tags.md`).
- **FR-014**: A delegate that stalls (no event for the configured time), exits non-zero or reports a limit is stopped by its PID file (never by pattern), and its task returns to the pool.
- **FR-016**: Every track has a small, stable **status file** under `.ai-gent/delegate/<track>/status.json` (schema in `contracts/status.md`), written by the adapter for delegates and by the supervisor for Claude's own tracks, so Claude appears as a worker too. The adapter's `status` operation and the workers pane both read only this file and the delegation log.
- **FR-018**: The pipeline pane shows only the compact workers line (counts, slots, a stalled or failed marker); it reads the same status files, adds no rows when there are no workers, and never lists individual workers.
- **FR-017**: The workers pane and `/workers` command live in the `project` plugin's mod, follow the guards and pipeline panes (pure model and draw modules, polling like the pipeline pane, counts and states only, never opening by itself, text fallback where no pane can dock), and show no prompt, brief secrets or credentials.
- **FR-015**: Worker rules for agents without hooks are one shared reference (`shared/delegate-worker-rules.md`) stating each guard's rule as a plain instruction, symlinked into the plugin; the brief names it first.

## Success criteria

- **SC-001**: With Codex absent or not opted in, the skill adds zero output and zero tokens to a session.
- **SC-002**: In a trial, a delegated track lands on `dev` with no check failing that the delegate could not see, no path outside `allowed_paths`, and no marker in history.
- **SC-003**: Two consecutive trials with the second at least as clean as the first on the checklist (`trial-1-codex.md` is the baseline).

## Open questions (owner)

1. Where the delegation log lives if the user wants it kept across machines (default: local only).
2. Whether a delegate may run containers once the disk floor allows more than one track (default: never, FR-004).
3. ~~OpenCode free-model privacy and per-run permissions~~ **Decided (owner, 2026-11-02):** the privacy caveat is told to the user whenever they ask to use delegation with a free hosted model; public content only after that notice; private repositories, credentials, customer data and `.ai-gent/` refused. Delegated OpenCode runs use a per-run permission set and never edit the user's own OpenCode config; installing ai-gent's rules and guard plugin stays the user's explicit choice.
4. Whether the mod API exposes Claude's running subagents and tasks (the pane would read them directly); until verified, the supervisor writes the status files for Claude's own tracks.
5. Which further agents get adapters after Codex and OpenCode, and in what order.
