# Trial 1: Codex on spec 038 T001 and T004 (2026-11-02)

Setup: Codex CLI 0.166.0 run by the supervisor (`codex exec`, workspace-write, no network) in a `wt.py` worktree on a branch off `dev`, with the brief in `contracts/brief-template.md` in an earlier form. ai-gent skills were already visible to Codex through `~/.agents/skills` (50 entries); Codex opened `git-workflow/SKILL.md` unprompted and read `AGENTS.md` five times.

## Result

- Both tasks done, tests green outside the sandbox (4 golden tests, the sqlite journeys, 51 Python oracle tests, `check.sh --quick`). Scope respected; `tasks.md` marked with evidence; no push or override attempted; honest final report.
- Wall time and tokens were **not recorded** (problem 7 below).

## Problems and fixes

| # | Problem | Fix (spec reference) |
|---|---|---|
| 1 | Could not commit: the common `.git` object store was read-only in the sandbox. | Pass the common git dir with `--add-dir`; preflight proves a commit (adapter `preflight`). |
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
