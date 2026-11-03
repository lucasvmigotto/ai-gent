---
name: delegate
description: Hand simple, well-bounded tasks to other code agents the user already has (Codex first) in worktrees and supervise them - brief, checks, cross-agent review, landing, take-back. Offered only when another agent is detected and the project opted in. Use for "delegate this to Codex", "use my other agents", "I'm near my usage limit".
---

# Delegate to other code agents

Claude Code stays the one place where work is planned, decided, reviewed and
landed. Other agents the user already has take bounded tasks so more gets
built at once and Claude's tokens last. Optional and off by default; the
contract is `specs/041-agent-delegation/` (spec, `contracts/`). The tools
are `ai-gent delegate <action>` (Rust binary; `--json` for machine output).
If `ai-gent` is missing, say so with the install command from the README and
stop; do not rebuild it in shell.

Builds on `agents:parallel` (worktrees via `wt.py`, one cap and the disk
floor, never overridden) and `agents:orchestrate` (roles).

## 1. Offer (once)

Silent unless another code agent is detected (`ai-gent delegate detect
--agent <id>`, which reads no auth files) **and** one trigger fires: the user
asks, a usage meter is near its limit (`token_report.py budget`, the
status-line meters), or parallel-safe tasks exist (`agents:parallel`'s plan).

One offer: name the agent found, the tasks it would take, and what it cannot
do (no ai-gent guards, no network, cannot commit). Record the answer in
`.ai-gent/delegate.yaml` (git-excluded): the agents allowed and the task
kinds each may take. A "no" is recorded and not asked again until the user
asks. No opt-in file, no delegation. This repository's allowlist is `[codex]`
only; other projects choose their own. Adapters that exist today: Codex
(`--agent codex`, the CLI's only value). OpenCode is specified
(`specs/041-agent-delegation/contracts/opencode.md`) but has no CLI adapter yet.

Free hosted models (OpenCode): before any use, tell the user their prompts
may be used by the provider to train models; send public content only and
refuse private repositories, credentials, customer data and anything under
`.ai-gent/`.

## 2. Assign

Split the tasks into tracks touching disjoint files. Delegates get simple,
bounded work: research and surveys, doc drafts, fixtures, small scripts,
mechanical edits, first-pass reviews. Claude keeps design judgment (Rust
core, hooks, shared references, cross-cutting changes). Never delegate a
task needing credentials, database or container tools, an owner decision, or
edits to a skill, plugin, hook or shared reference. Count delegated tracks
with Claude's against the worktree cap.

## 3. Run

1. `wt.py add <branch>` for the worktree; `ai-gent delegate ready` and
   `preflight --worktree <wt>` first (not logged in: report it, never work
   around it).
2. `ai-gent delegate brief ...` builds the brief from the template (read
   `references/supervisor.md` for the flags, `allowed_paths`, the worker
   pack `shared/delegate-worker-rules.md`).
3. `ai-gent delegate run --track <id> --worktree <wt> --brief-file <f>
   --tasks T001,...` starts it in the background; `status <track>` to watch,
   `result <track>` for the final message and usage. `mark <track> --as claude
   ...` records Claude's own tracks. Every dispatch is registered with
   `ai-gent delegate mark` or goes through `delegate run`, so the workers pane
   counts it.

## 4. Supervise and land

The delegate edits the working tree and stops. Then Claude:

1. `ai-gent delegate check --worktree <wt> --allow ... --track <id>`
   (paths, markers, environment prose, weakened tests, task marks); a
   refusal ends the track with the reason.
2. Runs `scripts/check.sh`-class verification outside the delegate's sandbox.
3. `ai-gent delegate commit --worktree <wt> --allow ... -m "<Conventional
   Commit>"`: staged by explicit path, no trailer or marker of any kind.
4. **Review by a different agent than the builder, always** (FR-010): the
   reviewer reads the diff and the spec, not the builder's report. Codex
   reviews read-only (`references/supervisor.md`). Findings go back; done
   means the acceptance criteria pass.
5. Only Claude pushes, opens the PR, watches CI and merges (`git:workflow`).

## 5. Take back

A delegate that stalls, exits non-zero or reports a limit is stopped with
`ai-gent delegate stop <track>` (PID file, never by pattern), marked
`taken-back`, and its task returns to Claude or another worker. When Claude
is the one near its limit, queued tasks move to the delegates. When a track lands or is taken back, remove its worktree by name with `wt.py remove <branch>` (label-scoped, never a prune; `../../references/containers.md` §9a). `ai-gent
delegate log` prints the delegation log (`.ai-gent/delegation.log`).

## Not built yet

- The scheduler (classifying tasks, assigning tracks, counting the cap) is
  not a subcommand: Claude does it by hand from section 2.
- Nothing reads `.ai-gent/delegate.yaml`; Claude reads and writes it.
- A review runner (T021): the Codex reviewer is run by hand.
- Token tagging of delegated runs is limited to what `result` reports.

Say so if asked; do not invent flags.
