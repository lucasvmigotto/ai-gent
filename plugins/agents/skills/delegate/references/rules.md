# Delegation rules

Read this before choosing what to delegate and before writing a brief. The
rules come from the recorded delegated units in
`specs/041-agent-delegation/comparison.md`; the numbers there are the
evidence.

## What to delegate

1. **Security-shaped work stays with the supervisor.** Anything that decides
   allow or deny (permission sets, redaction, guards) is not "simple". Build
   it yourself, or require two independent reviews.
2. **Small and bounded, with a reference implementation, delegates well.**
   Do not delegate a task that needs more than one pass of design judgement.
3. **Free hosted models (OpenCode):** keep them to new untracked docs or
   research, with the privacy acknowledgement. Do not give one a task that
   edits tracked files in a worktree until a second real OpenCode run proves
   the isolation fix.

## What the brief must say

4. **A port or parity task lists the original's behavior** (timeouts, counts,
   caps, formats) and asks for one test per listed item, against the original.
5. **Every brief carries the defect-class lines:** guards and fingerprints
   cover creation, change, deletion and revert; every probe, read and child
   process has a bound; every failure path fails closed; the exact case the
   code must refuse is named.
6. **Each test names an input the code must reject** and asserts both the
   rejection and that no secret appears. The reviewer checks for tests that
   cannot fail.

## Before and after the run

7. **Review is mandatory for every unit**: a different agent from the
   builder, read-only (findings appeared in every recorded unit).
8. **One builder fix round.** After it, the supervisor finishes the work.
9. **Verify before the PR:** `cargo test` for the touched crates and a
   Windows-aware check (path separators, command-line length). Four escapes
   reached CI without it.
10. **Recount any number a reviewer cites** (one of three findings was false
    in trial 5), and re-fetch the sources behind a verdict-deciding claim
    (`ai-gent delegate verify-sources`).
11. **Record per run** in the delegation log or the handoff: builder agent and
    model, wall time, tokens, who made each fix (builder or supervisor) and how
    many findings were rejected. Without these the comparison cannot be
    measured.
