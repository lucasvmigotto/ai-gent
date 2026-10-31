# Implementation Plan: Guard log

**Branch**: `docs/mod-specs-2` (spec) | **Date**: 2026-10-31 | **Spec**: `spec.md` | **Research**: `research.md`

## Summary

`shared/guard-ux.ts` gains a pure `record(id, result, event)` that turns a recognized deny or ask into a `GuardRow` (verb only). Each of the five modules calls it beside `decorate` and appends the row to its own bounded `guardLog` atom. The `project` plugin lists the five under `dependencies`, reads their atoms with `$.state.get` (per plugin, tolerating an absent one) and answers `/guards` and draws `/guards-pane`.

## Technical Context

**Language**: TypeScript hooks modules. **Engine**: Claude Code 2.1.312. **Events**: `classic.PreToolUse` (existing hook, one more call after `decorate`), `session.start` and `command.run` (`$.command.register`), `ui.render` (`Pane`, like 013). **Storage**: `$.state` only, bounded. **Project Type**: Claude Code plugin modules.

## Files

| Path | What |
|---|---|
| `shared/guard-ux.ts` | gains `GuardRow`, `verb` (tightened rule), `record` (stores the entry `match`, never the raw reason), `append` (the ring) |
| `plugins/{git,db,secrets,repo,devcontainer}/hooks/guard-ux.ts` | copies, rewritten by `python3 scripts/consistency.py --sync-shared-code` |
| `plugins/{git,db,secrets,repo,devcontainer}/hooks/register.tsx` | the existing `classic.PreToolUse` hook also appends the row, unless `guardLog` is false |
| `plugins/{git,devcontainer}/types/index.d.ts` | exist today (`git` declares `hygiene` and `toasted`, `devcontainer` declares `toasted`); each gains the `guardLog` atom |
| `plugins/{db,secrets,repo}/types/index.d.ts` | **created** (these plugins have no `types/` directory and no `"types"` key in `plugin.json` today), with `"types": "./types/index.d.ts"` added to each `plugin.json` |
| `plugins/project/hooks/guards.ts` | pure: `summarize(rows, n)`, the `/guards` text, the pane's text |
| `plugins/project/hooks/guards.test.tsx` | `/guards`, the pane on `terminal` and `desktop`, limits and empty cases |
| `plugins/project/hooks/register.tsx` | registers `/guards` and `/guards-pane` beside 012–014's hooks |
| `plugins/<p>/hooks/guard-ux-hook.test.ts` | each plugin's: a row per deny and ask, a clean row for every secret fixture, none for another guard's reason |
| `plugins/<p>/.claude-plugin/plugin.json` | `userConfig` `guardLog` (five), `guardLogLimit` and `guardsPane` (`project`); `project` also gains `dependencies`: `git`, `db`, `secrets`, `repo`, `devcontainer` |
| `plugins/project/scripts/guard_log.py`, `test_guard_log.py` | User Story 3 only, built last |

No change to any `guard.sh`, `opencode/guard/` or the `scripts/test-*-guard.sh` suites.

## Testing

- `claude plugin test` per plugin with a hooks module (012's loop in `scripts/check.sh`); each guard plugin's test installs the `classic.PreToolUse` hook beneath that answers the decision `guard.sh` would print, as 015's do.
- The secret-fixture case builds each token from `shared/secret-cases.json`'s `vars` fragments at run time (016's rule), so no full token shape is committed, and feeds it as a first word, an argument and a URL; a case asserts the interpolated `$branch` text never reaches a row.
- Existing guard suites and the parity test run unchanged (SC-002).
- Manual, once per pin bump: R1 and R2 on `claude --debug`.

## check.sh / CI impact

- `scripts/consistency.py` already fails if a `guard-ux.ts` copy differs; the new functions ride the same check.
- Version bumps for `git`, `db`, `secrets`, `repo`, `devcontainer` and `project` come from `scripts/release.py`'s shared-file rule (the shared file changed).

## Implementation order (tests first)

1. Resolve research R1 (a `project` probe reading a dependency's atom, installed and not installed) and R2 with a probe module and a headless run; record the result in `research.md`.
2. Write `verb`, `record`, `append` in `shared/guard-ux.ts` with tests (verb rule, ring bound, secret fixtures); sync the copies.
3. Create `types/` in `db`, `secrets` and `repo`, declare `guardLog` in all five, wire the five registers behind `guardLog`; per-plugin hook tests.
4. `project`: add `dependencies`, `summarize`, `/guards` (per-plugin tolerant reads, `not reporting:`), tests including a subset of plugins present; then `/guards-pane` and its tests.
5. Docs: README mods table row and `docs/site` through `skill-docs`; mark the spec Implemented.
6. Last, if wanted: `guard_log.py` and `/guards --history`.

## Risks

- **API churn** (early access): a cross-plugin atom read of an absent plugin and call ids may behave differently than assumed (R1, R2); R1's result decides whether `dependencies` stays or the log moves to the transcript script (User Story 3's `guard_log.py`).
- **Coupling**: `project` names five plugins in `dependencies`; a plugin renamed or removed breaks the contract lay-down, which `claude plugin validate` and `check.sh` catch.
- **A log becomes a secret store**: verb-only rows and the fixture test keep it so (FR-002, FR-003).
- **Misreading counts as blame**: the text states counts only (FR-006).
- **Five modules on one event**: each appends only its own guard's rows; order doesn't matter.

## Constitution Check

N/A — no constitution.
