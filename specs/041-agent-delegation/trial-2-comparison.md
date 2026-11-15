# Trial 2: Claude-profile builds with Codex review, compared with trial 1 (SC-003)

SC-003: "Two consecutive trials with the second at least as clean as the first on the checklist (`trial-1-codex.md` is the baseline)."

Written 2026-11-03. Sources: `trial-1-codex.md` (trials 1 to 5), `git log` and `gh pr view` for PRs #388, #392, #397, #399 and #400, and the contracts in this folder. Per-run timings and token counts of the Claude-profile builds were not recorded anywhere I could cite, so none are claimed.

## The checklist

`trial-1-codex.md` has no section named "checklist". Its checklist is the nine problems of trial 1, plus the review lessons of trials 3 to 5 that it calls checklist items. The items reproduced and scored here:

1. Delegate can commit (resolved by design: the supervisor commits, FR-007).
2. Worker never waits on stdin; fails fast with no first event.
3. Worker's own `check.sh` or sandbox failures never become prose in the repository (FR-011).
4. Worker never deletes what it did not create.
5. Scope held by `allowed_paths` and the automatic path check, not by discipline (FR-011).
6. No marker or forbidden trailer in a commit message.
7. Time and tokens recorded per run (FR-013).
8. Disk floor and parallelism counted across all tracks.
9. No supervisor-side kill of its own shell (PID files, FR-014).
10. Review by a different agent than the builder (FR-010), against the acceptance criteria, reviewer not shown the supervisor's corrections.
11. Any claim that decides a verdict is re-checked by the supervisor against its sources, and a reviewer's numbers are recounted (trials 3 and 5).
12. Builder-fix rounds stop after one try; the supervisor finishes (trial 4).

## Scoring

Key: clean (no recurrence found), partial, not shown (no evidence either way), regressed.

| # | Trial 1 (Codex, spec 038 T001 and T004) | Second trial (Claude-profile builds, Codex review) | Evidence |
|---|---|---|---|
| 1 | Failed: sandbox `.git` read-only | Clean: supervisor commits | trial 2 of `trial-1-codex.md`; #388 to #400 commit histories show one supervisor-made commit stream |
| 2 | Failed: waited on stdin | Clean: adapter `run` closes stdin and judges by the first event | #388 `fix(delegate): Claude runs report tokens, are not stalled early...` (2a51e04) |
| 3 | Failed: 13 and 16 sandbox failures written into the README | Not shown: a Claude profile has no OS sandbox, so the failure class differs; no such prose in the five PRs' diffs that I found, but no automatic FR-011 prose check run is cited | #388 body ("A Claude profile has no OS sandbox") |
| 4 | Failed: deletion rule conflict | Not shown: no deletion incident recorded | none |
| 5 | Partial: discipline only | Clean as reported: `allowed_paths` and path check in use from #388 on; the Codex-only trial 2 already showed it. No out-of-scope diff is recorded in the five PRs | `delegate check` named in the task for every PR; no per-PR check output is quoted, so this rests on the PR bodies |
| 6 | Failed: marker in a commit message | Clean in the five PRs' commit messages | `git log` of #388, #392, #397, #399, #400 (no marker lines) |
| 7 | Failed: nothing recorded | Partial: the adapter now records usage and a log line (FR-013); #388 added token reporting for Claude runs, #392 per-step usage for OpenCode. This document cites no per-run figures for the five builds | #388 2a51e04, #392 body |
| 8 | Noted | Not shown: no parallel disk-floor incident | none |
| 9 | Failed: self-kill with `pkill` | Not shown: PID files exist; no recurrence recorded | trial 1 fix, FR-014 |
| 10 | Not exercised (first review came in trial 4) | Met in practice: Codex read-only reviewed every PR; #400 added `delegate review`, which refuses reviewer == builder | #388, #392 (8 findings), #397 (6), #399 (3), #400 (5, 4, 3) |
| 11 | n/a | Partial: the #400 findings were all fixed, but no PR records a supervisor recount of a reviewer's claim | #400 body |
| 12 | n/a | Not shown: the PRs record fix rounds, not who made each | #388 had three passes; #400 three review rounds |

## Review findings per round

| PR | Builder | Reviewer | Findings | Notes |
|---|---|---|---|---|
| #388 Claude adapter | Claude profile, three passes | Codex read-only | not counted in the PR | follow-up fixes in 2a51e04 (tokens, early stall, `is_error`, own profile) and 178b752 (deny rules for profile directories, narrow cargo and git, no shell for read-only roles) |
| #392 OpenCode adapter | Claude profile | Codex read-only | 8, all fixed | tested against a fake opencode only; no real run, stated in `contracts/opencode.md` |
| #397 dbrun crate | Claude profile | Codex read-only | 6, all fixed | T018 stays open |
| #399 delegate events | Claude profile | Codex read-only | 3 medium, all fixed | |
| #400 delegate review | Claude profile | Codex read-only, three rounds | 5, then 4, then 3 (12 in all) | round 1 included a high finding: the Codex adapter ran reviewers with write access (fixed: `-s read-only`); others were credential and symlink handling, check findings not stopping a review, a finished OpenCode review discarded on exit code 1, prompt size, dry-run parity, OpenCode model self-review |

Baseline for comparison: trial 4 (Codex reviewing OpenCode) found 8, 8 and 4 across three rounds, and trial 5 had 1 verified finding of 3.

Total across the five PRs: at least 8 + 6 + 3 + 12 = 29 counted findings (#388 uncounted). Every one counted was fixed before merge; none was dismissed as false in the PR bodies. Findings fell with rounds in #400 (5, 4, 3) as they did in trial 4 (8, 8, 4), which is the loop converging, not the builder being clean on its first pass.

## What the supervisor had to correct

From the PR text only, not from a log of supervisor edits (none is kept):

- #388: the first pass lacked token reporting, stalled runs early, did not fail on `is_error`, and did not require a profile of its own; a second pass lacked profile-directory denies and gave read-only roles a shell. The deny-rule syntax was checked against the real CLI by the supervisor.
- #400: the high finding (reviewers with write access in the Codex adapter) was a defect in code already on `dev`, found by using the new command, not by the builder.
- Trial 4 rule 3 ("stop the builder-fix round after one try") is not shown to have been applied or broken; the PRs do not say.

## What escaped to CI

Local verification (tests, clippy, fmt, `check.sh`) did not catch these, so CI did:

| PR | Escape | Commits |
|---|---|---|
| #392 | Guard bench gate: 11.7 ms, then 15.7 ms, then 15.48 and 15.39 ms against the 15 ms FR-012 budget on the shared runner. First a retry, then a 25 ms CI budget via `bench-guard.py --budget-ms` | 5681039 (retry, replaced), 7a4baed |
| #392 | Timing test: `healthy_install_is_all_ok_and_fast` took 10.6 s against a 10 s debug limit on a loaded musl runner | 19cb442 |
| #392 | Windows: fake-attempt lookup depended on path separators | 787fe73 |
| #400 | Windows: the 100 KiB review prompt exceeds the roughly 32 KB command-line limit; the 1 MiB diff test failed on both Windows build legs. Cap is now 24 KiB on Windows | 5db07b0 |

Two of the four are test or gate flakiness on shared runners rather than worker mistakes. The two Windows escapes are real portability defects that a Linux-only local check cannot see. Trial 1 had no CI-escape data, so there is no baseline to compare against.

## Verdict on SC-003: not yet met

The second trial is not demonstrably at least as clean as the first, because the evidence is incomplete rather than because it is worse:

- On the items trial 1 failed (1, 2, 5, 6, partly 7) the later work shows no recurrence. That favors the second trial.
- Items 3, 4, 8, 9, 11 and 12 are "not shown": the PRs do not record them either way, so they cannot be scored as clean.
- The builder differs (Claude profile against Codex), so the comparison is of the supervisor loop more than of one agent. SC-003 does not forbid that, but a reader should know.
- Findings are plentiful (about 29 counted) and CI escapes are real (four, two of them Windows portability). "At least as clean" cannot be asserted when trial 1 recorded no finding counts to match.
- T021 is still unchecked: #400 was open when this was written, and its body says the supervisor must repeat the trial on the landed command. FR-010 is therefore met manually, not by a landed runner.
- "Two consecutive trials" needs the checklist scored by the supervisor during a run, which was not done for these builds.

## What is missing to close T030

1. Land #400 and run one delegated task end to end through `delegate review`, scoring items 1 to 12 live and recording wall time and tokens (FR-013).
2. Keep a per-run record of supervisor corrections and of whether each fix round was the builder's or the supervisor's.
3. Run the Windows and `cargo test` legs of the touched crates locally or in a pre-CI step, so portability and timing escapes are caught before the PR.
4. Run one real OpenCode adapter delegation (#392 was tested against a fake only).

T030 stays unchecked.
