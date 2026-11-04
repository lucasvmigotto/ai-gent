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

## Review runner

`ai-gent delegate review --builder-track <t> --reviewer codex|claude|opencode
[--worktree <wt>] [--spec <file>]... --allow <glob>... [--no-check] [--base <rev>]
[--reviewer-profile <dir>] [--reviewer-model <m>] [--out <file>] [--dry-run]`.
`--allow` is required: without it (and without `--no-check`) the command exits 2
and launches nothing, because a change is checked (FR-011) before it is reviewed.
`--no-check` reviews without the check; the prompt then says it was skipped on
request.
It reads the builder's agent from `.ai-gent/delegate/<t>/status.json` and
refuses with exit 4 when the reviewer is that agent in the same profile or
config: Codex never reviews Codex; Claude profiles are compared by config
directory (`claude` builder = `AI_GENT_CLAUDE_CONFIG_DIR`, `claude-subagent` =
the supervisor's own); OpenCode by model. The prompt is built from the
worktree: the standing instructions (read-only, severity-ranked findings with
file:line and a failure scenario, recount every number, no credentials), the
`check` findings, the spec files and the diff text
against `--base` (default: merge base with dev, main or master); the whole prompt
is capped at 100 KiB (one argument cannot exceed the OS limit), the spec and
diff blocks are cut with a "[truncated: N bytes left out]" note, and a prompt
that cannot fit even with minimal context is refused. The builder's
report is never read. The run is the track `review-<t>` in role `reviewer`;
the final message goes to `--out` (default
`.ai-gent/delegate/review-<t>/review.md`). Try `--dry-run` first.

## Landing checklist

The supervisor does these in order after the delegate stops.

1. Stage by explicit path, never `git add -A` or `.`; nothing from `.ai-gent/`.
2. `ai-gent delegate check --worktree <wt> --allow ... --track <id>`; a refusal ends the track.
3. Rerun the tests, clippy and fmt (or `scripts/check.sh`) yourself, outside the worker's sandbox; its report is not evidence.
4. Review by an agent that is not the builder, read-only: `ai-gent delegate review --builder-track <id> --reviewer <agent> --spec <spec> --allow ...` (above). It builds the prompt with the diff text, the spec and the `check` findings, never the builder's report.
5. Recount every number the reviewer cites (trial 5: one of three findings was false). For a research claim, `delegate verify-sources`.
6. Apply the verified fixes yourself; one builder fix round at most (trial 4).
7. `ai-gent delegate commit ... -m "<Conventional Commit>"`: no marker or trailer of any kind.
8. Push, open the PR, and merge only when every CI leg is green.
9. Remove the worktree by name: `wt.py remove <branch>`.
10. Refill: count the free slots and start the next approved, dependency-ready track, or note the hold reason in the ledger (`../../../references/worker-continuation.md`).

## Watcher

Detached workers do not notify. Keep one persistent Monitor running `ai-gent delegate watch` (a line per unacknowledged event `track:started:state`, and `idle: free_slots=N`); handle each once, then `ai-gent delegate ack <id>` for that id only (never `--all` while workers run). An event of a `review-<track>` reviewer track is the review finishing: read its file and decide on the builder's track; do not start another review. A Claude Code Monitor expires after about 30 minutes: re-arm it. After a restart, `delegate watch --once` replays what was missed; then reconcile from the status files. `delegate idle --tasks-file F` gives the hold reason; for the review step use `delegate review --builder-track T --reviewer <agent> --allow ...`.

## Register every dispatch

A worker launched by hand (another Claude profile's `claude -p`, any other agent) is registered with `ai-gent delegate mark`, or started with `delegate run`: the workers pane and the slot count see only registered tracks.

## Failure modes

A single `apply_patch` error is not a stall. No first event within the wait
stops the run. Never read a delegate's credentials files.
