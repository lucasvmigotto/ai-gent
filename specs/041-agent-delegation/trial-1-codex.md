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

---

# Trial 3: OpenCode (`opencode/flint-40b-free`) on spec 038 T002 (2026-11-02)

Setup: a read-and-research task writing one file (`engine-matrix.md`), `allowed_paths` of that file only, per-run permissions from `OPENCODE_CONFIG_CONTENT` (ai-gent's rules with every `ask` turned into `deny`, plus denies for commits, pushes, `gh`, `sudo` and credential directories), `--standalone --auto --format json`, stdin closed, public content only after the privacy notice.

## Result

- Wall time 835 s; 27 steps, 55 tool calls (23 reads, 23 fetches, 3 searches, 1 write); about 115k input, 47k output tokens (1.39M cached read); cost 0. Exit code 1 although the file was complete.
- Scope respected: only the allowed file, no git writes, no markers, no permission denials needed.
- Versions, repositories and dates of all ten crates were correct against crates.io. **One decision-relevant conclusion was wrong**: it called `tiberius` unmaintained with an archived repository, citing RUSTSEC-2020-0010; that advisory covers 0.3.x only and the repository is not archived (pushed 2026-09-25). It also cited a non-existent path (`dbun-spike-report.md`) seven times. The supervisor corrected both before committing.

## What it changes

| # | Finding | Change |
|---|---|---|
| 1 | A free model is a good first-draft researcher with tools of its own, but can state a wrong conclusion with sources that, read fully, say otherwise. | Mandatory review stays; add a supervisor step that re-fetches the cited sources for any claim that decides a verdict (checklist item). |
| 2 | Exit code 1 despite a finished file. | Adapter judges by the final `step_finish` reason and the diff; the exit code is advisory. |
| 3 | Usage is per step. | Adapter sums `step_finish.tokens` and `cost`. |
| 4 | Time is long (14 min) for a research task. | Record time per run (FR-013); consider a per-task time budget before taking the task back (FR-014). |
| 5 | `--auto` auto-approves every `ask`. | Per-run config converts `ask` to `deny` (done in this trial). |

---

# Trial 4: cross-agent review loop on the OpenCode survey (2026-11-02)

Codex (read-only, `codex exec -s read-only` with a custom prompt) reviewed OpenCode's `engine-matrix.md` three times; OpenCode fixed round 1's findings once; the supervisor fixed the rest.

| Round | Reviewer time and cost | Result |
|---|---|---|
| 1 | 28 s; 98,625 input (66,304 cached), 1,132 output tokens | 8 findings: wrong labelling scheme against `shared/research.md`, MySQL and MariaDB merged, missing columns the spec requires, overstated claims, unsourced claims. |
| fix by OpenCode | 1,604 s; 113k input, 87k output tokens | Relabelled and added columns, but sent MariaDB to a different crate than the plan allows and invented a new wrong file path. |
| 2 | 51 s; 164,962 input (133,632 cached), 1,690 output tokens | 8 findings including the MariaDB crate, the broken path, an overstated PostgreSQL claim and labels pointing at sources that do not establish them. Acceptance not met. |
| fix by supervisor | about 2 minutes of supervisor time | All of round 2's findings. |
| 3 | 40 s | 4 remaining findings (PostgreSQL wording, maintenance conclusion, two source rows, label coverage). Acceptance not fully met. Fixed by the supervisor. |

## What it changed

1. **The review step is worth its cost.** Three reviews cost about two minutes and 270k input tokens (mostly cached) and found spec-compliance gaps no one else had seen, including two that the supervisor's own correction introduced.
2. **Review against the acceptance criteria, not "is it good".** The prompt names the acceptance file, the labelling rules and the evidence files, and lists the previous findings so a later review can check each.
3. **Stop the builder-fix round after one try.** A delegate asked to fix review findings can make a new error while fixing the old ones (the second path typo); the supervisor finishes the work.
4. **A second reviewer finds what the first missed**, and a reviewer should not be shown the supervisor's corrections, only the previous findings.
5. **Reviewer role command**: `codex exec -s read-only` (see `contracts/adapter.md`).
6. **Scope**: the owner decided delegation is Codex only; the OpenCode findings stay as evidence for the loop, not as a feature.
