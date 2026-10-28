# Implementation Plan: Pipeline pane (project:status, live)

**Branch**: `docs/mod-specs` (spec) | **Date**: 2026-10-28 | **Spec**: `spec.md`

## Summary

A `Pane` drawn by the `project` plugin's hooks module: a snapshot computed from `$.fs` reads (artifacts, `specs/README.md`, markers) plus `git log -1` dates for staleness, kept in `$.state`, refreshed by a `stat` poll and by completed Edit/Write calls. A `/pipeline` command opens it.

## Technical Context

**Language**: TypeScript hooks module (types from `'claude-code'`, no Node, no DOM). **Engine**: Claude Code 2.1.312 (the CI pin 012 sets). **Events**: `ui.render` (`Pane`), `command.run` (via `$.command.register`), `tool.call` (after `next`, Edit/Write paths), `session.start`. **Storage**: `$.state` only; nothing persisted. **Project Type**: Claude Code plugin module.

## Files

| Path | What |
|---|---|
| `plugins/project/hooks/register.tsx` | 012's module; gains one line composing `pipelinePane(on, options)` |
| `plugins/project/hooks/pipeline-pane.tsx` | pane rendering, command, refresh wiring |
| `plugins/project/hooks/pipeline-model.ts` | pure functions: README parse, artifact list, staleness, markers, next-stage rules |
| `plugins/project/hooks/pipeline-pane.test.tsx` | stories 1–4 on `terminal` and `desktop`, fixtures through the kit's `$.fs` |
| `plugins/project/hooks/fixtures/` | small pipeline trees (no pipeline, partial, full, blocked) |
| `plugins/project/types/index.d.ts` | 012's state contract, plus the `pipeline` atoms |
| `plugins/project/.claude-plugin/plugin.json` | `userConfig`: `pipelinePane` (boolean), `pipelinePollSeconds` (number) |

No change to `plugins/project/skills/status/SKILL.md` or `shared/pipeline.md`.

## Testing

- `claude plugin test plugins/project` (wired into `scripts/check.sh` by 012).
- Contract test: the module's artifact constant equals the `Path` column of the pipeline's artifact table that §1 checks, read through `$.fs` from `plugins/project/references/pipeline.md` (the plugin's link to `shared/pipeline.md`).
- Parity test (FR-003): fixture READMEs plus their expected JSON, generated once from `python3 plugins/repo/scripts/specsync.py features --json` and `python3 plugins/agents/scripts/wt.py plan --json` and committed under `fixtures/`; `test_specsync.py` and `test_wt.py` keep the script side honest. Regenerate the expected JSON when either script's parsing changes.
- Manual: `claude --plugin-dir plugins/project --debug` on this repository and on a repo with no pipeline.

## check.sh / CI impact

None beyond 012: `check.sh` already runs `claude plugin test` for each plugin with a module, and `.github/workflows/check.yml` installs the pinned Claude Code. The run grows by one test file.

## Risks

- **API churn**: early access; `Pane` props, `$.ui.open` placement widths and `$.clock.every` may move. Mitigation: `claude plugin validate` in check.sh, pin bumped deliberately, the pure model kept separate from drawing.
- **Third README parser** (after `specsync.py` and `wt.py`): drift if the README format changes. Mitigated by the parity fixture; a shared script was considered and declined (decided 2026-10-28: parse in TypeScript).
- **Cost on big repos**: marker scanning is capped; staleness spawns at most one `git log` per artifact per refresh, and only when the file's `mtimeMs` changed.
- **Two features in one module** (012 and 013): a failing 013 hook must not take 012 down — each feature registers its own hooks, and a render hook that throws is skipped alone.

## Constitution Check

N/A — no constitution.
