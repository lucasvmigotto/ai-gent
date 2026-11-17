# Codex parity work

The owner authorized this work on 2026-11-17 after reviewing the
[audit](audit-2026-11-17.md):

> Document this excellent analysis and start development - consider that there is a second code agent catching up later on your work

This directory keeps the audit, baseline evidence, implementation scope, and
handoff together so the next agent can continue without a transcript. The
audit describes main at `0cca79b`; implementation starts from develop at
`0a61fc4`. Its historical findings are not current completion claims.

## Current slice: dependency correctness

Branch: `fix/delegate-dependencies`. Worktree:
`.worktrees/delegate-dependencies`. The initial slice is locally validated
and ready for independent review; publication and landing remain pending.
Read [handoff.md](handoff.md) before editing these files or claiming a task.
The 3.0.0 development at home remains separate.

This slice implements dependency validation/order from F03 and the first
conservative part of F02:

- Reject cyclic and self-dependent task graphs with a diagnostic.
- Order tasks within existing tracks by their dependencies, breaking ties
  by input order. Preserve the current grouping, ownership policy, and track
  IDs (derived from the first declared task, not the new execution order).
- Require a non-reviewer run to finish with a supervisor's accepted verdict
  before it satisfies dependencies in `delegate idle`.
- Use the newest attempt for each task; a new failure or take-back must not
  inherit an older accepted result. Ambiguous equal-start attempts must all
  satisfy the completion gate.
- Add deterministic Rust regressions and run repository checks.

The accepted verdict currently means supervisor checks accepted, **not**
independent review, integration, or evidence bound to a revision. F02 remains
partially open until those concepts are represented and checked explicitly.
Task IDs are also not yet scoped by feature in status files. This slice does
not claim to solve that identity gap. F03's proposed separation of dependency
edges from ownership grouping also remains planned; existing track grouping
is deliberately preserved here.

Compatibility: older `done` tracks with a `pending` verdict now keep their
consumers waiting. The coordinator must perform the original scoped checks
and record the result (for example, `delegate check --track` with that
track's worktree, allowlist, and required evidence). Do not bulk-accept old
tracks just to free dependencies. An accepted researcher can satisfy a task;
a reviewer completing the review cannot satisfy the task it reviewed.

## Follow-up slices

| Slice | Audit findings | Status |
|---|---|---|
| Dependency validation/order and acceptance gate | F02/F03 (partial) | Locally validated; review/landing pending |
| Honest source-match outcome | F11 | Planned; changes public outcome/exit semantics |
| Neutral coordinator and independent review identity | F01, F06 | Planned |
| Dispatch authorization and explicit execution settings | F05, F07 | Planned |
| Codex event translation, hook trust, and host integration | F04, F12 | Planned |
| Recovery, history, and usage | F08, F09, F10 | Planned |
| Lifecycle instructions and generated evidence | F13, F14 | Planned |

The detailed correction plans and acceptance criteria are in the audit.
No finding is complete merely because a task has been added to this table.

## Validation and baseline

[baseline-evidence.json](baseline-evidence.json) records the original nine
synthetic probes against installed ai-gent 2.28.0, with temporary paths
normalized. [probes.py](probes.py) reruns the observations on Linux with the
`ai-gent` found on PATH; it prints results rather than asserting that the
original defects should persist. It never dispatches an agent, executes a
patch, or accesses a database or credential.

[dependency-slice-evidence.json](dependency-slice-evidence.json) reruns the
same probes against the binary rebuilt from this worktree. It shows corrected
ordering, rejected cycles, and rejected predecessors keeping consumers
blocked. The unrelated source, assignment, and patch-event findings remain
visible rather than being counted as fixed by this slice.

Use the targeted Rust tests for implementation verification. Full
`scripts/check.sh` is required before every commit. The handoff records exact
commands, results, limitations, and which binary was tested.

## Boundaries

No change in this slice to skills, plugins, hooks, shared references, agent
models, historical owner policy, status-file schema, release gates, or the
3.0.0 migration. Propose those changes and obtain approval as `AGENTS.md`
requires. Do not allocate a numbered specification that the parallel agent
may already be using without first reconciling the current remote work.

Pushes, PRs, integration merges, and releases are not authorized by this
implementation request. Keep the work on its branch and record what remains.
