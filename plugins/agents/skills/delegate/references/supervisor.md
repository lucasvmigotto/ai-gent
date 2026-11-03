# Supervisor details

Read when running a delegation. Contracts: `specs/041-agent-delegation/contracts/`.

## Brief

`ai-gent delegate brief --repo <name> --branch <b> --spec <dir>
--tasks T001,T002 --task-file <verbatim text> --allow '<glob>' (repeat)
--pack shared/delegate-worker-rules.md --verify '<targeted test>' (repeat)
--tmpdir $HOME/tmp`. The brief carries hard limits and the final-message
format; the worker pack is named first. Targeted tests only: the delegate's
sandbox has no network and shows failures that are not real.

## Check and commit

`check` exits 4 when a rule refuses; pass `--evidence T010="what ran"` for
each task mark in the diff and `--text-file` for a PR description to scan.
`commit` stages only paths inside `--allow`.

## Codex reviewer (trial 4)

`codex exec --cd <wt> -s read-only -o <last> --json "<review prompt>" < /dev/null`.
The prompt names the diff, the files to check it against and the acceptance
criteria; the reviewer has no network, lists what it could not check, edits
nothing. Do not use `codex exec review`.

## Landing checklist (T021)

No runner exists; the supervisor does these in order after the delegate stops.

1. Stage by explicit path, never `git add -A` or `.`; nothing from `.ai-gent/`.
2. `ai-gent delegate check --worktree <wt> --allow ... --track <id>`; a refusal ends the track.
3. Rerun the tests, clippy and fmt (or `scripts/check.sh`) yourself, outside the worker's sandbox; its report is not evidence.
4. Review by an agent that is not the builder, read-only: `codex exec -s read-only` (above) or a Claude profile in a reviewer role (`Read Grep Glob` only, no Bash, diff text in the prompt; `contracts/adapter.md`). Give it the diff, the spec and the acceptance criteria, not the builder's report.
5. Recount every number the reviewer cites (trial 5: one of three findings was false). For a research claim, `delegate verify-sources`.
6. Apply the verified fixes yourself; one builder fix round at most (trial 4).
7. `ai-gent delegate commit ... -m "<Conventional Commit>"`: no marker or trailer of any kind.
8. Push, open the PR, and merge only when every CI leg is green.
9. Remove the worktree by name: `wt.py remove <branch>`.
10. Refill: count the free slots and start the next approved, dependency-ready track, or note the hold reason in the ledger (`../../../references/worker-continuation.md`).

## Watcher

Detached workers do not notify. Keep one persistent Monitor over `.ai-gent/delegate/*/status.json` printing a line per state change and when slots idle; handle each line once by its id (track + started + state). After a restart, reconcile from the status files first.

## Register every dispatch

A worker launched by hand (another Claude profile's `claude -p`, any other agent) is registered with `ai-gent delegate mark`, or started with `delegate run`: the workers pane and the slot count see only registered tracks.

## Failure modes

A single `apply_patch` error is not a stall. No first event within the wait
stops the run. Never read a delegate's credentials files.
