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
asks. No opt-in file, no delegation. This repository's allowlist is Codex and
a second Claude profile (FR-002), so OpenCode is not used for ai-gent's own work; other projects choose their own. Adapters that exist today: Codex
(`--agent codex`, the default), a second Claude profile (`--agent claude`)
and OpenCode (`--agent opencode`, contract in
`specs/041-agent-delegation/contracts/opencode.md`). OpenCode has no OS
sandbox: a deny-only permission set passed per run is the containment, weaker
than Codex's, so its output always gets the cross-agent review. It has been
tested only against a fake `opencode`, not a real run.

Free hosted models (OpenCode): a model whose name ends in `free` is refused
unless `run` or `preflight` gets `--privacy-ack`, and you give the notice
first: the provider may use prompts to train models. Send public content
only and refuse private repositories, credentials, customer data and anything
under `.ai-gent/`. `--model` takes `flint-40b-free` or a full `provider/model`.

## 2. Assign

Split the tasks into tracks touching disjoint files. Delegates get simple,
bounded work: research and surveys, doc drafts, fixtures, small scripts,
mechanical edits, first-pass reviews. Claude keeps design judgment (Rust
core, hooks, shared references, cross-cutting changes). Claude-profile workers may
take more, Rust work included, under supervision; Codex gets simple bounded
tasks. Never delegate a
task needing credentials, database or container tools, an owner decision, or
edits to a skill, plugin, hook or shared reference. Count delegated tracks
with Claude's against the worktree cap; subagents of this session do not count.

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
   reviewer reads the diff and the spec, not the builder's report.
   `ai-gent delegate review` runs it read-only and refuses a reviewer that
   is the builder (`references/supervisor.md`). Findings go back; done
   means the acceptance criteria pass.
5. Only Claude pushes, opens the PR, watches CI and merges (`git:workflow`).
6. Refill: a landed or taken-back track frees a slot; start the next approved,
   dependency-ready track or record why not.

A detached worker does not notify Claude when it stops. Keep one Monitor running
`ai-gent delegate watch` (it expires after ~30 min; re-arm it) and run the
steps above on each builder event, then `delegate ack <id>` with that event's
id only (not `--all`). An event of a `review-<track>` reviewer track is the
review finishing: read its file and decide on the builder's track instead. `delegate idle
--tasks-file F` gives the hold reason; the events log survives a restart. The
protocol, event id and recovery are in `../../references/worker-continuation.md`;
ledger columns for event ids and hold reasons are still planned.

## 5. Take back

A delegate that stalls, exits non-zero or reports a limit is stopped with
`ai-gent delegate stop <track>` (PID file, never by pattern), marked
`taken-back`, and its task returns to Claude or another worker. When Claude
is the one near its limit, queued tasks move to the delegates. When a track lands or is taken back, remove its worktree by name with `wt.py remove <branch>` (label-scoped, never a prune; `../../references/containers.md` §9a). `ai-gent
delegate log` prints the delegation log (`.ai-gent/delegation.log`).

## Also built

- `ai-gent delegate plan --tasks-file <json|-> [--allow-agent <id>]...
  [--min-free-gb N]` is the scheduler: it forms tracks with disjoint paths,
  classifies each task, assigns a worker and admits tracks against the cap and
  the disk floor. Input `[{id, kind, paths, needs, depends}]`; JSON out.
- `ai-gent delegate verify-sources --claim <text>|--claim-file <f>
  --source <url|path>...` re-fetches the cited sources and compares the
  claim's checkable terms; exit 4 unless confirmed. Run it on any research
  claim that decides a verdict.
- The Claude-profile adapter: `--agent claude` on `detect`, `ready`,
  `preflight`, `run`, `status`, `stop`, `result`. It needs
  `AI_GENT_CLAUDE_CONFIG_DIR` (a separate profile; never the current one).

- `ai-gent delegate review --builder-track <t> --reviewer
  codex|claude|opencode [--worktree <wt>] [--spec <f>]... --allow <glob>...
  [--no-check] [--out <f>] [--dry-run]` is the review step (FR-010): it
  refuses (exit 4) when the reviewer is the builder's agent in the same
  profile or config, builds the prompt from the worktree (diff text, specs,
  `check` findings, never the builder's report; capped at 100 KiB, cuts are
  noted) and runs the reviewer read-only as the track `review-<t>`. `--allow`
  is required (exit 2 without it): the FR-011 check runs first and findings
  stop the review; `--no-check` skips it on request and the prompt says so.
  `--dry-run` prints the prompt and the decision.
  Details in `references/supervisor.md`.

## Not built yet

- No subcommand reads `.ai-gent/delegate.yaml`: `plan` takes `--allow-agent`,
  and Claude reads and writes the file.
- Token tagging of delegated runs is limited to what `result` reports.

Say so if asked; do not invent flags.
