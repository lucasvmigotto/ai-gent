# Delegation compared with doing it directly (T030, SC-003 evidence)

Written 2026-11-07. This is the supervisor's judgement from the evidence below, not a measurement. The scores are mine (0 to 3 per axis), made after the fact from PR sizes, the review files the supervisor logged and the contracts in this folder. No unit was run twice (once delegated, once direct), so "cost more than doing it directly" is an estimate, never a measured difference.

## Sources

- `git log` and `git show --shortstat` on `origin/dev` (sizes are the final commit, so they include fixes the supervisor added before merge, not only the delegate's first diff).
- `trial-1-codex.md`, `trial-2-comparison.md`, `tasks.md` and `contracts/opencode.md` in this folder.
- The supervisor's local logs in `~/tmp` (review texts `review-*.md` and `codex-*-review.md`, briefs `brief-*.md`, Codex event logs `codex-*.jsonl`). Review texts name findings; they do not record who built the unit. Builder identity for T017, T018, T024 and the pass allowlist (Codex) and for the Claude subagent units comes from the supervisor's task statement, not from a file in the repository; the Codex model name is not recorded anywhere.

## Not recorded (stated plainly)

- Builder wall time and tokens for every unit except the OpenCode quickstart run (309 s, 465,401 in / 18,549 out) and the two Codex trial-1 runs and T010b and T015 (tokens only, no wall time).
- The Codex and Claude model names and versions for builders.
- A log of supervisor edits: how many of the fixes were made by the builder and how many by the supervisor is not kept (the PR texts say "fixed", not by whom), except trial 4 (OpenCode's own fix round, 1,604 s, 113k in / 87k out).
- A direct-work baseline (time or tokens the supervisor would have spent doing the same units).
- Whether any "real" finding count below was disputed: no log of a rejected finding exists except trial 5 (1 of 3 verified).
- The Oracle spike (a8c9419) review text: no `review-*` file for it was found; only its fix commit a3ad793 exists.
- The #388 and #397 finding counts (not counted in the PR texts).

## Delegated units

"Real" means the supervisor fixed it; no review file records a finding the supervisor rejected in these units, so real equals reported unless noted. I did not re-verify the findings against code. Reviewer is Codex read-only unless noted.

| Unit (commit) | Builder | Size (final commit) | Review findings, real | Fixes before merge | Time and tokens |
|---|---|---|---|---|---|
| Trial 1 A, spec 038 T001/T004 | Codex | not pulled | 9 process problems found by the trial | rules, adapter, FR-011 checks | builder 2,680,651 in (2,583,552 cached) / 15,651 out; wall time not recorded |
| Trial 2 B, spec 038 T003/T013 | Codex | not pulled | none against the work; 1 contract finding (cannot commit) | contract change only | builder 662,346 in (617,088 cached) / 6,156 out |
| T010b adapter template | Codex | 1 file plus tasks line | none recorded | none recorded | 240,937 in (212,864 cached) / 2,957 out |
| T015 worker rules | Codex | 1 file | none recorded | none recorded | 219,258 in (184,064 cached) / 1,306 out |
| #388 Claude adapter (178b752 and follow-ups) | Claude subagent, 3 passes | not pulled | at least 3 P1 in the first review (profile dir readable, cargo can reach git, read-only roles could write); count of later rounds not recorded | 2 follow-up passes | review 453,055 in (371,968 cached) / 3,383 out |
| T010a OpenCode adapter (fada12f, #392) | Claude subagent | 7 files, +1783/-3 | 8 (4 P1, 4 P2); all fixed | deny forms, `#variant` privacy gate, preflight flags, result write-back, relative binary; plus 3 CI escapes (bench gate, timing test, Windows separators) | review 363,954 in / 4,031 out |
| T021 `delegate review` (8f4313f, #400) | Claude subagent | 12 files, +1623/-20 | 5 + 4 + 3 = 12 over 3 rounds, one high (reviewer had write access, in code already on dev) | 3 rounds; plus Windows prompt-limit escape to CI | review tokens not recorded |
| T022 events (c92c719, #399) | Claude subagent | 7 files, +958/-2 | 3 medium | dedupe and lock fixes | not recorded |
| T015/T016 dbrun crate (24cc02c) | Claude subagent; "ran out of turns" per the review brief | 28 files, +2543/-3 | 6 (2 P1, 4 P2) | all fixed | review 576,617 in (505,216 cached) / 5,744 out |
| T017 process connector (a75bcfe) | Codex | 4 files, +539/-1 | 3 medium | fixed | not recorded |
| T018 plan core (beaaf59) | Codex | 6 files, +787/-4 | 1 medium | fixed | not recorded |
| T024 install.ps1 and doctor (feb057b) | Codex | 4 files, +355/-58 | 1 medium | fixed | not recorded |
| Pass allowlist, Rust evaluator (af3705a) | Codex | 4 files, +374/-3 | 1 medium (FIFO hang) | fixed | not recorded |
| PostgreSQL spike (14fc280) | not recorded | 6 files, +391/-2 | 2 medium | fixed | not recorded |
| MySQL/MariaDB spike (0571d27) | not recorded | 7 files, +505/-2 | 2 (1 medium, 1 low) | fixed | not recorded |
| SQL Server spike (1a30a09, fix 9633606) | not recorded | 9 files, +876/-6 | 2 (1 high, 1 medium) | fixed in 36/27 lines | not recorded |
| Oracle spike (a8c9419, fix a3ad793) | not recorded | 9 files, +1073/-5 | review text not found | fix commit 29/4 lines | not recorded |
| Quickstart + first real OpenCode run (2a7e2a0) | OpenCode `opencode/flint-40b-free`, v2.2.5 | 87 lines in the doc, 3 files | 6 (5 medium, 1 low); all real per `opencode.md` | supervisor fixed all six | 309 s, 465,401 in (378,624 cached) / 18,549 out, cost 0; Codex review about 6 findings |
| OpenCode isolation fix (e8cb8fd, 09f30b5) | not a delegated build; a defect the OpenCode run exposed | 12 files +290/-11, then 8 files +441/-56 | the second commit exists because the first guard missed deletions, reverts, commits and fail-open | 2 commits | not recorded |

## Scores (0 to 3, supervisor's judgement)

Axes: C correctness on first pass, S scope discipline, T test quality, R rework needed (3 = little or none). 0 = unusable, 1 = major problems, 2 = minor problems, 3 = clean.

| Unit | C | S | T | R | One-line justification |
|---|---|---|---|---|---|
| Codex trial 1 A | 2 | 1 | 1 | 1 | Work done, but unrelated `cargo fmt` edits, sandbox failures written into the README and a deletion-rule conflict; nine process fixes. |
| Codex trial 2 B | 3 | 3 | 3 | 2 | Both tasks verified by the supervisor, all paths in scope, no prose; could not commit so the contract changed. |
| Codex T010b, T015 | 2 | 3 | n/a | 2 | Docs only, scoped; no findings logged either way (so I give 2 where nothing is shown). |
| Claude adapter #388 | 1 | 2 | 1 | 1 | First pass let workers read the profile and gave read-only roles a shell; needed three passes. |
| OpenCode adapter T010a | 1 | 2 | 1 | 1 | 4 P1 deny gaps; permission tests prove serialization, not enforcement (finding 8); 3 CI escapes. |
| `delegate review` T021 | 2 | 2 | 2 | 1 | 12 findings over 3 rounds, converging 5/4/3; one high found in already-merged code, not the builder's. |
| Events T022 | 2 | 3 | 2 | 2 | 3 medium, all concurrency or dedupe; tests listed for restarts and locks. |
| dbrun crate T015/16 | 1 | 2 | 1 | 1 | Ran out of turns; error paths could echo secrets; a test never asserted what its name claimed. |
| T017 process connector | 2 | 3 | 2 | 2 | Three medium (success after timeout, per-batch deadline, redaction after truncation). |
| T018 plan core | 3 | 3 | 2 | 3 | One medium (30 s versus 120 s timeout regression). |
| T024 install.ps1 | 2 | 3 | 2 | 3 | One medium (unbounded probe); installer tests bypass `ai-gent update`. |
| Pass allowlist | 3 | 3 | 3 | 3 | One medium (non-regular file hang), found only because the shell guard already had the check. |
| Spikes (pg, mysql, mssql) | 2 | 3 | 2 | 2 | Two defects each, mostly parity with Python (comments, `max_rows`, datetime format, timeouts, `@@ROWCOUNT` side effect). |
| OpenCode quickstart | 1 | 3 | n/a | 1 | File landed in the main checkout (isolation defect); six real sequencing errors in a 87-line doc. |

Reading the table: no unit scored 3 on first-pass correctness except the smallest (T018, pass allowlist). Scope discipline is the strongest axis (nearly all 2 or 3) once `allowed_paths` and the path check exist; the one scope failure (OpenCode writing into the main checkout) was a platform defect, not discipline.

## Where delegation paid off

- Small, well-bounded changes against an existing reference (pass allowlist, T018, T024, T015, T010b): one review finding or none, cheap (T015 and T010b cost under 250k input tokens each, mostly cached), and the supervisor spent its own tokens on review rather than writing.
- Cross-agent review was the most productive step. Codex read-only reviews cost one to a few minutes and 100k to 580k input tokens (mostly cached) and found real defects in every unit reviewed, including a high one (a reviewer with write access) and several the builder's own tests had hidden. Trial 4 found issues in the supervisor's own corrections.
- Docs and template files by a free model (OpenCode, cost 0) are usable as a first draft: 309 s and 18.5k output tokens for a well-formed quickstart, which a read-only Codex review then repaired.
- Disk and worktree isolation gave scope safety for Claude and Codex builders; no out-of-scope diff is recorded in the PRs.

## Where it cost more than doing it directly (estimate)

- Security-shaped code (permission sets, redaction, guards): the builder produced pattern lists with gaps, and the supervisor spent three review rounds and fixes (OpenCode adapter 8 findings, Claude adapter 3 P1, dbrun redaction). I judge writing these directly would have been cheaper; not measured.
- Parity ports (Rust spike drivers and process connector against the Python original): each unit had 1 to 3 behavior drifts that only the reviewer's reading of the Python found.
- Anything with a free model on a real repository: the isolation defect needed two further commits (+731/-67 lines together) and a second real run is still owed.
- Fix rounds by the builder: trial 4's OpenCode fix round took 1,604 s and introduced a new wrong path; the supervisor then finished in about 2 minutes. Only the first trial had the "builder fixes once, then supervisor" rule applied.
- CI escapes that local checks cannot see: guard bench gate, a timing test, Windows path separators (#392), Windows command-line limit (#400): four in all (from `trial-2-comparison.md`).

## Recurring defect classes

1. Incomplete state fingerprints and guards: missing deletions, reverts and HEAD moves in the main-checkout guard (09f30b5); a fail-open path when the snapshot could not be taken.
2. Fail-open and unbounded operations: a PowerShell probe with no timeout (T024), SQL Server control batches without a timeout, reading a FIFO as an allowlist, a connector returning success after the child was killed at the deadline.
3. Deny lists with gaps: git in absolute-path and `env` forms, shell reads of credential-named files, profile files denied only as directories, the free-model privacy gate bypassed with `#variant`.
4. Tests that cannot fail or prove the wrong thing: permission tests that check serialization not enforcement, a malformed-request test that never asserts the secret is absent, an integration test that builds the adapter with the acknowledgement already set and so hides a preflight that could never pass.
5. Behavior drift from the original: timeouts (30 s versus 120 s, per statement versus per batch), row counts, `max_rows` in write mode, datetime format, comments before keywords.
6. Redaction after truncation, and error text that echoes the request or the SQL.
7. Instructions in a doc that are individually valid but wrong in sequence (output never saved, `--agent` not carried through, event id used as track id).
8. Portability: Windows path separators and command-line length.

## Recommended rules for the `agents:delegate` skill (for the owner; the skill is not edited here)

1. Keep cross-agent review mandatory and keep the reviewer read-only and never the builder; require at least one review for every unit, as the evidence shows findings in every one.
2. Give a security-shaped task (permission sets, redaction, guards, anything that decides allow or deny) to the supervisor or require two independent reviews; do not delegate it as "simple".
3. Brief a port or parity task with the original's behavior listed (timeouts, counts, caps, formats) and require a test per listed item against the original.
4. Put in every brief: guards and fingerprints must cover creation, change, deletion and revert; every probe, read and child process has a bound; every failure path fails closed; and name the exact case the code must refuse.
5. Require that each test name an input that the code must reject and assert the rejection (and the absence of any secret), and have the reviewer check for tests that cannot fail.
6. Limit builder fix rounds to one; the supervisor finishes (trial 4 already says this; apply it).
7. Do not use a free hosted model in a worktree for a task that edits tracked files until the second real OpenCode run proves the isolation fix; keep it to new untracked docs or research with the privacy acknowledgement.
8. Record per run: builder agent and model, wall time, tokens, who made each fix (builder or supervisor) and the number of findings rejected. Without these, SC-003 stays unmeasurable.
9. Run `cargo test` for the touched crates and a Windows-aware check (path separators, command-line length) before the PR, since four escapes reached CI.
10. Recount any number a reviewer cites (trial 5: 1 of 3 findings was false) and re-fetch sources behind any verdict-deciding claim (T012a).
11. Prefer delegation for small, bounded changes with a reference implementation; do not delegate when the task needs more than one pass of design judgement.

## What closes T030

Every number cited above has a source, but the sources are incomplete, and the comparison cannot support "at least as clean": builder time and tokens are missing for most units, the builder's model is not recorded, a direct-work baseline does not exist, and there is no log of who made each fix. T030 is left open; see `tasks.md`.
