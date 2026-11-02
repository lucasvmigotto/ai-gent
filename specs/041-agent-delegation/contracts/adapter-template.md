# Further-agent adapter template

Use this template for `<agent>`; replace placeholders with trial evidence.
An adapter is a small script or module the skill calls; it never decides what to build.
Follow `adapter.md`, `../spec.md` FR-003 and `../../../shared/delegate-worker-rules.md`.
All facts about an agent not yet trialed are **unverified**; do not infer support from CLI help.

## Operations and pass criteria

| Operation | Pass criterion |
|---|---|
| `detect` | Finds the CLI on `PATH` or a verified install path and runs its version command; never reads auth files. |
| `ready` | A cheap, non-interactive call proves it can start with the user's login; auth failure is reported as "not logged in", never worked around. |
| `preflight <worktree>` | Proves the worktree is writable, the agent starts with stdin closed, and network behavior is as expected; does not require a commit or writable `.git`. |
| `run <worktree> <brief>` | Starts in the worktree in the background with stdin closed; records a PID file, events and final message under `.ai-gent/delegate/<track>/`; survives the supervising shell call ending. |
| `status <track>` | Reports last event time, event count and running or exited from the stable status file (`status.md`); exposes no prompt, credentials or command output. |
| `stop <track>` | Stops the run and its process group/tree by the PID file, never by pattern; unrelated processes survive. |
| `result <track>` | Returns final message, exit code and usage when reported; preserves failure or missing-usage evidence rather than inventing success or zero tokens. |

Stdin closure is proved through the run path: a fake agent reads stdin to EOF.
Write status atomically per `status.md`; record start, end and usage for FR-013.
Delegates leave changes in the working tree and stop; the supervisor stages and commits.

## Real-trial checklist

For every item record the exact command/config, observation and evidence path, or **unverified**.
Never read, copy or log credentials, including the agent's own login files.

- [ ] Non-interactive command: binary/version, model, role, flags, working directory and a cheap ready probe; no interactive approval required.
- [ ] Stdin closed: explicit `< /dev/null` or equivalent; first event arrives without waiting for additional input.
- [ ] Output destinations: stdout, stderr, event stream and final message files; PID ownership and background lifetime verified.
- [ ] Usage: exact event/JSON fields for input, cached input, output, reasoning and cost where reported; final total versus sum over steps, with no double counting; unavailable fields stay unknown.
- [ ] `.git`: whether the sandbox permits writes to index, refs, object store or common git dir; record the restriction without giving the worker commit rights or writing real repository metadata.
- [ ] Network: distinguish model transport from worker tools; verify permitted/denied access and report exceptions to the no-network worker rule to the supervisor.
- [ ] Sandbox/permissions: OS sandbox versus tool permissions, scope of writes, prompts/auto-approval and explicit denies; never silently relax limits or guards.
- [ ] Enforcement kind: an OS sandbox (Codex) or only a tool allowlist (Claude profile, trial 5 review). A tool allowlist cannot stop a subprocess (a build script, `git diff --output=<file>`) or a read of the profile directory by pattern alone: record what is enforced and what is only requested, and compensate with supervisor checks (FR-011), no credentials in the worker's environment, and read-only roles without a shell.
- [ ] Reviewer: exact read-only invocation and enforced tools/permissions; custom acceptance prompt works and edits are denied. A reviewer reads the diff and spec, not the builder's report.
- [ ] Progress: event types, last-event time and count, completion/limit signals and stall detection; a single recovered error event is not a stall.
- [ ] Exit codes: observed success, failure and limit behavior; compare final event and diff, never assume the code alone proves completion.
- [ ] Config/profile isolation: per-run override or separate profile; user's config untouched, credentials never inspected or shared, shared service/session behavior recorded.
- [ ] Hosted-model privacy: provider terms/evidence for prompt/completion retention and training, or **unverified**. For free hosted models give the privacy notice; public content only, refusing private repositories, credentials, customer data and `.ai-gent/`.
- [ ] Scope and acceptance: only `allowed_paths`, targeted checks in the worker; supervisor verification outside the sandbox and review by a different agent. Re-fetch sources for decisive claims and recount quantitative reviewer claims.

## Trial record format

Keep a dated record beside `../trial-1-codex.md`; link it from the agent's contract.
Use the existing trial's Setup, Result, Problems and fixes, and What it changed format.

| Field | Record |
|---|---|
| Setup | Agent/CLI version, model, role, OS, worktree/task, brief and allowed paths; exact invocation and non-secret config/profile selection. |
| Checklist | Each item above: observation, evidence and pass/fail or unverified; no unsupported agent claims. |
| Time | Adapter's own start/end clock, wall seconds, ready-probe and reviewer time separately. |
| Usage | Reported input/cached/output/reasoning tokens and cost, source fields and aggregation; missing values explicitly unavailable. |
| Result | Exit code, final event/message, event/tool counts, changed paths, targeted checks, supervisor checks, reviewer findings and acceptance. |
| Problems and fixes | Problem, observed evidence, fix and spec reference; distinguish a proposed fix from one verified by a rerun. |
| Comparison | Compare time, tokens, scope and problems with `../trial-1-codex.md`; record remaining unverified facts and permitted task kinds. |

Emit one JSON log line per run in `.ai-gent/delegation.log` (FR-013), git-excluded.
Runtime blockers and environment failures belong in the final report to the supervisor, never in task output docs.
The supervisor records trial findings deliberately; workers do not add environment prose to repository files.

## Required fake-agent tests

Tests run without the real agent, credentials or external network; assert each operation's pass criterion.

- Detect found/missing CLI and version failure; ready success/auth failure without reading auth files.
- Preflight writable/missing/unwritable worktree, start failure and expected network policy; read-only `.git` is not a commit-preflight failure.
- Run receives the intended cwd, brief, model/profile and role permissions; stdin reads zero bytes to EOF; background watcher outlives the launch call.
- Events, stderr and final output land in the track files; progress updates atomic status with time/count/state and no brief or secrets.
- Result parses representative recorded event fixtures, final versus per-step usage, absent usage, malformed/incomplete output, non-zero exit and limit signals.
- Stall/no-first-event handling returns the task to the pool; stop uses the PID file and stops children while a same-name decoy survives.
- Config/profile override leaves the user's config untouched; reviewer invocation denies edits and forbidden operations are refused without bypass.

## Listing rule

List `<agent>` as supported only after its adapter passes the seven operations and fake-agent tests,
the checklist is completed with evidence and explicit restrictions, and at least one passing real trial is recorded (FR-003).
Unverified capabilities remain labelled **unverified** and unavailable for assignment; a CLI name or planned adapter is not support.
Support does not opt a project in: its allowlist and permitted task kinds remain that project's choice (FR-002).
