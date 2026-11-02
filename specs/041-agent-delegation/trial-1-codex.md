# Trial 1: Codex on spec 038 T001 and T004 (2026-11-02)

Setup: Codex CLI 0.166.0 run by the supervisor (`codex exec`, workspace-write, no network) in a `wt.py` worktree on a branch off `dev`, with the brief in `contracts/brief-template.md` in an earlier form. ai-gent skills were already visible to Codex through `~/.agents/skills` (50 entries); Codex opened `git-workflow/SKILL.md` unprompted and read `AGENTS.md` five times.

## Result

- Both tasks done, tests green outside the sandbox (4 golden tests, the sqlite journeys, 51 Python oracle tests, `check.sh --quick`). Scope respected; `tasks.md` marked with evidence; no push or override attempted; honest final report.
- Wall time and tokens were **not recorded** (problem 7 below).

## Problems and fixes

| # | Problem | Fix (spec reference) |
|---|---|---|
| 1 | Could not commit: the common `.git` object store was read-only in the sandbox. | ~~Pass the common git dir with `--add-dir`~~ (failed in trial 2, see below): the supervisor commits (FR-007). |
| 2 | `codex exec` waited on stdin. | Always `< /dev/null`; fail fast if no first event (adapter `run`). |
| 3 | Its own `check.sh` reported 13 (quick) and 16 (full) failures, all sandbox artifacts, and it wrote them into the README. | Targeted tests only for the worker; supervisor runs `check.sh` and CI; a brief rule and the FR-011 diff check (new prose about environment failures). |
| 4 | "Delete nothing outside the worktree" conflicted with checks cleaning their own temp files. | Reword: never delete what you did not create. |
| 5 | Scope relied on discipline (`cargo fmt` touched unrelated files; explicit-path staging). | `allowed_paths` and the automatic path check. |
| 6 | Supervisor error: a marker line in a commit message, against the owner's "no markers" decision. | Commit template without markers and a message check. |
| 7 | No time or token record. | Adapter records start, end and usage events; one log line per run (FR-013). |
| 8 | Disk floor allows one track at a time. | Scheduler counts all tracks; parallelism needs disk. |
| 9 | Killed the run with a `pkill` pattern that matched the supervisor's own shell. | PID files (FR-014). |

## Owner decisions taken during the trial

Codex only in v1; no markers at all; offers on request, near a limit and when parallel-safe tasks exist; cross-agent review mandatory.

---

# Trial 2: Codex on spec 038 T003 and T013 (2026-11-02)

Setup: the same CLI and sandbox; brief per `contracts/brief-template.md` with `allowed_paths`, the targeted-tests-only rule, the no-environment-prose rule, the reworded deletion rule, the common git dir passed with `--add-dir`, and a pointer to `git:workflow`. T002 (web research) was not given to Codex: its sandbox has no network.

## Result

- Both tasks done and verified by the supervisor outside the sandbox: spike tests, the eight SQLite journeys through `--runner` and through the default path, default behavior unchanged. Every changed path was inside `allowed_paths`; no environment prose in repository files; no marker anywhere (the supervisor committed).
- **Cost** (from `turn.completed.usage`): trial 1: 2,680,651 input (2,583,552 cached), 15,651 output tokens; trial 2: 662,346 input (617,088 cached), 6,156 output tokens. Trial 2 was about four times cheaper for two tasks: a narrower brief and no failing `check.sh` to chase.

## What the trial changed

| # | Finding | Change |
|---|---|---|
| 1 | The commit fix from trial 1 **failed**: with the common git dir added, `index.lock` was still read-only. Codex keeps `.git` read-only regardless of `--add-dir`. | Contract changed: the delegate leaves changes in the working tree; the supervisor commits (FR-007, adapter `preflight`, brief template). |
| 2 | A delegate that cannot commit still produced clean, scoped work; letting the supervisor commit removes the marker risk and lets the path check run before anything is committed. | Kept as the design, not a workaround. |
| 3 | One `apply_patch` verification error appeared mid-run and recovered. | A single error event is not a stall (adapter note). |
| 4 | Usage is available only in the final event, so a running delegate shows progress but not cost. | The pane shows event counts while running and tokens when done. |

## Open

Time per run is not yet recorded (the adapter will keep its own clock); the second-agent reviewer step (FR-010) has not been exercised.
