# Implementation Plan: CI watch band

**Branch**: `docs/mod-specs-2` (spec) | **Date**: 2026-10-31 | **Spec**: `spec.md`

## Summary

In the `repo` plugin's hooks module: after a successful Bash `git push`, find the commit's runs through `reporun.py cli github -- run list --commit <sha> --json …` (only when `detect` says `gh` is signed in, and, because `reporun cli` decrypts a pass entry that exists, only when `token_in_pass` is false or `ciBandUsePass` is on), keep them in `$.state`, draw an `AbovePrompt` band with a priority for the band-priority rule, and toast once per failing run. A bounded back-off timer refreshes it; nothing is a model turn.

## Technical Context

**Language**: TypeScript hooks module. **Engine**: Claude Code 2.1.312. **Events**: `tool.call` (after `next`, Bash), `ui.render` (`AbovePrompt`), `session.start`. **Storage**: `$.state` atoms (`ciRuns`, `ciToasted`, `ciHidden`, `ciTimer`, `ciDetect`; `data-model.md`), shape-tagged. `plugins/repo` has no `types/` directory and no `types` key in `plugin.json` today, so the plan creates both. **Project Type**: Claude Code plugin module.

## Read-only calls

| Purpose | Call |
|---|---|
| platform, CLI state | `python3 ${$.plugin.root}/scripts/reporun.py detect --json` (`platforms.<p>.token_in_pass`, `.cli.installed`, `.cli.signed_in`; cached in `ciDetect`) |
| pushed commit and branch | `git rev-parse HEAD`, `git rev-parse --abbrev-ref HEAD` |
| the runs | `python3 …/reporun.py cli github -- run list --commit <sha> --json databaseId,name,status,conclusion,workflowName,headSha` (no `--write`); stderr is merged into stdout by `cmd_cli`, so the parser takes the array from the first `[` line |
| CI configured at all | `$.fs.exists` on `.github/workflows` |

Nothing else: no `run view`, `--log`, `rerun`, `cancel`, `workflow run`, `secret list`, `variable list`.

## Files

| Path | What |
|---|---|
| `plugins/repo/hooks/register.tsx` | module entry (shared with 015's guard-ux part); restructured so each part is gated on its own option (`guardUx`, `ciBand`), replacing the early `return` at line 12; gains the composing lines |
| `plugins/repo/hooks/ci-model.ts` | pure: the push-command pattern, the run JSON, states, marks, the band's wording, the toast text (Planned) |
| `plugins/repo/hooks/ci.ts` | the calls, the back-off timer, the atoms (Planned) |
| `plugins/repo/hooks/ci.test.tsx` | stories 1–3, `$.process.run` mocked, a recorder asserting FR-001 (Planned) |
| `plugins/repo/types/index.d.ts` | the `ci*` atoms; created (the directory does not exist today) |
| `plugins/repo/.claude-plugin/plugin.json` | gains `"types": "./types/index.d.ts"` and `userConfig`: `ciBand`, `ciBandPollSeconds`, `ciBandMinutes`, `ciBandUsePass` |

No change to `reporun.py`, `devsecops:watch` or `shared/secrets.md`.

## Testing

- `claude plugin test plugins/repo`; `$.process.run` mocked with recorded `reporun detect --json` (including `token_in_pass` true and false) and `run list` outputs (queued, in progress, success, failure, cancelled, empty), one with a `gh` warning line before the JSON array, and a test that `guardUx: false` leaves the CI band working and the reverse.
- A recorder test: every command the mod spawns in a push scenario is in the read-only table.
- Manual: a push to a throwaway branch of this repository, once green and once with a failing step (never `main`).

## Implementation order (tests first)

1. Create `plugins/repo/types/index.d.ts` and the `types` key; atoms, `userConfig`; the push pattern and run parsing (with the stderr-merge rule and the `detect` gate) in `ci-model.ts` with unit tests; restructure `register.tsx`'s gating with 015's tests green.
2. Failing tests for stories 1–3 and the recorder.
3. `ci.ts`: detect gate, `run list`, back-off timer with the hard stop.
4. `AbovePrompt` band composing with the others; the Hide button; the toast.
5. Record GitLab and `/ci-status` as follow-ups (out of the first cut).
6. `scripts/check.sh`, docs row (`skill-docs`), manual push.

## check.sh / CI impact

`claude plugin test` for `repo` joins 015's run (if 015 already added the module, the loop already covers it). One more test file.

## Risks

- **API rate and cost**: one `run list` per poll with back-off and a 15-minute stop; none without a push.
- **A silent mis-read of "no runs"**: the wording says the cause may be paths, branches or a bot commit; never "CI is broken".
- **Platform drift**: `gh run list --json` field names (checked against `gh` on 2026-10-31); mitigated by a pinned recorded fixture, by the defensive array extraction, and by degrading to no band when parsing fails.
- **Pinentry mid-session**: `reporun cli` decrypts an existing pass entry; mitigated by the `token_in_pass` gate and `ciBandUsePass` (default off), and documented.
- **Band clutter**: this band yields to the band-priority rule (at most two visible, failures first).
- **Overlap with `devsecops:watch`**: the band never explains; the toast points to the skill.

## Constitution Check

N/A — no constitution.
