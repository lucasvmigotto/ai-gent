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

## Failure modes

A single `apply_patch` error is not a stall. No first event within the wait
stops the run. Never read a delegate's credentials files.
