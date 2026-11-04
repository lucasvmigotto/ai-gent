# Completion and continuation

One protocol for `agents:orchestrate`, `agents:parallel` and `agents:delegate`:
how the coordinator learns a worker stopped, what it does then, and why free
capacity never sits idle without a recorded reason. The skills point here and
do not copy it. Source: the owner's analysis of 2026-11-03; section 6 says
what is built and what is still planned.

## 1. Worker kinds and how their completion arrives

| Kind | Completion reaches the coordinator | Needs a watcher |
|---|---|---|
| Subagent of this session (Agent tool) | The harness notifies the coordinator by itself | No |
| Background command (`run_in_background`) | The same, when it exits | No |
| Detached worker (`ai-gent delegate run`: Codex, a second Claude profile, OpenCode) | **Nothing.** The process is not a child of the session; it only rewrites `.ai-gent/delegate/<track>/status.json` | **Yes** |

For detached workers the coordinator keeps one persistent Monitor running
`ai-gent delegate watch`. It prints one line per unacknowledged event
(`done`, `failed`, `stalled`, `taken-back`) and an `idle: free_slots=N` line
when slots are free and no track is live. In Claude Code a Monitor line wakes
an idle coordinator turn, but a Monitor expires after about 30 minutes: re-arm
it, and treat a silent Monitor as unproven, not as quiet workers. Nothing wakes a coordinator after
its session ends, and a toast or UI pane cannot wake a model turn. So state
must be recoverable from files: the events log (`.ai-gent/delegate/events.jsonl`)
survives, and a restarted coordinator replays what it missed with
`delegate watch --once` (only unacknowledged events). On a new session or
after a compaction, also read the status files, `delegation.log` and the
ledger and reconcile (section 5) before anything else. The Monitor is started
after the first detached dispatch and checked again at every session start.

## 2. The normalized event

A terminal or capacity event is the tuple `track`, `started` (the run's start
time from `status.json`), `state` and a result reference
(`ai-gent delegate result <track>`, never the text itself).

`event id = track:started:state`, as `delegate watch` prints it. After
handling an event, run `ai-gent delegate ack <id>` (or `ack --all`); watch
never replays an acknowledged one. The ledger may also note the ids handled. An id already handled is a duplicate; an event whose
`started` is older than the track's current run is stale. Both are ignored,
and the files, not the event, say what is true now. A worker's exit is not
proof of accepted work.

## 3. The sequence on every terminal event

1. **Verify outside the worker**: `ai-gent delegate check`, then the project's
   checks run by you, outside the worker's sandbox. Its report is data.
2. **Review by a different agent** than the builder, from the diff and the
   spec: `ai-gent delegate review --builder-track T --reviewer <agent>
   --allow <glob>... [--evidence T=...]`. It refuses (exit 4) a reviewer that
   is the builder, runs read-only, and needs `--allow` (the brief's
   `allowed_paths`) or `--no-check`.
3. **Land** under the usual gates (section 4).
4. **Refill.** Count worktree slots from the registered tracks (cap 3, the
   `max_parallel` of `.ai-gent/parallel.yaml`; subagents of the session do not
   count). Fill free slots from approved tasks whose dependencies have landed,
   else start the next stage's tasks. `ai-gent delegate idle --tasks-file F`
   prints `free_slots=N ready_tasks=M reason=...` (`dispatch`, `cap`,
   `dependency`, `approval`, `none-ready`) and never dispatches. It does not
   know `ownership conflict` or `quota`; the coordinator records those.

**Invariant:** if approved ready work and free capacity coexist, dispatch, or
record the reason in the ledger: `dependency`, `approval`, `ownership
conflict`, `quota`, or `nothing ready`. Free slots with queued work are not a
bug when the reason is one of these; they are a bug when there is none.

## 4. What stays gated on the user

Approvals the pipeline and `git:workflow` require, every merge to `main`,
publishing, releases, paid runs, and new scope. Refill only starts work the
user already approved (a card in the plan, a task in `tasks.md`). A
deterministic supervisor, a script or the Monitor, may start approved,
dependency-ready, mechanical dispatches; it never decides model-judgement
work (design, review verdicts, what to do about a failure). Worker messages
carry no user authority.

## 5. Failure and recovery

Always reconcile from the files; never from memory of what an event said.

- **failed**: record it and the result reference, one repair round at most,
  then take the task back or escalate; refill the other slots meanwhile.
- **stalled**: confirm with `delegate status` (last-event age) before acting;
  then `delegate stop <track>` (PID file) and treat it as taken back.
- **taken-back**: the task returns to the coordinator or another worker; keep
  partial work in its worktree; release the slot once it is removed.
- **rate or quota limit**: defer the task with reason `quota` and the reset
  time in the ledger; wake on that time or the next event. No retry loop.
- **duplicate or stale event**: ignore (section 2).
- **restart or compaction**: re-list the status files, the ledger rows still
  `running` or `review`, and any worktree from `wt.py list`; handle every
  terminal track not yet marked handled, then run step 4 of section 3.

## 6. Built and planned

Built: the status files and `delegation.log`, `ai-gent delegate status`,
`result` and `mark`, the append-only events log with `delegate events
[--pending]`, `delegate watch [--once] [--since-ack] [--json]`, `delegate ack
<id>... | --all`, `delegate idle --tasks-file F` and `delegate review`, and
the Workers pane (`plugins/project`), which only **reads** them. The pane
shows state and never detects, schedules or dispatches.

Planned, not available, so no skill may tell anyone to run them:

- Ledger columns for event ids handled and for hold reasons (the ledger is
  still written by the coordinator by hand).
- Any automatic dispatch: `delegate idle` only reports; the coordinator
  starts the work.

The invariant is kept by the coordinator, with `watch` and `idle` as its
instruments.
