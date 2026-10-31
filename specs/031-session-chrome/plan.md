# Implementation Plan: Session chrome

**Spec**: `spec.md` | **Tests**: `claude plugin test plugins/<name>`, run per plugin and by `scripts/check.sh`

## Files

| Path | What |
|---|---|
| `shared/band-priority.ts` (+ four copies) | `Summary`, `Tone`, optional `Band.summary`, `isFoldable`, `split`, `listing(..., fold)`, `parseBands` keeps a valid summary |
| `plugins/{project,git,repo,devcontainer}/hooks/bands.ts` | `folding($)` reads `project/workspaceLeader`; `plan` and `/bands` use `split`; project adds `declareLeader` |
| `plugins/repo/hooks/ci-model.ts`, `ci.ts` | `summaryOf(runs)`, recorded with the band |
| `plugins/devcontainer/hooks/containers-model.ts`, `containers.ts` | `summaryOf(findings)` (none under a low disk) |
| `plugins/git/hooks/hygiene-model.ts`, `hygiene.ts`, `release-model.ts`, `release.ts`, `bands.ts` | `summaryOf`, `setBand(..., summary)` |
| `plugins/project/hooks/workspace-model.ts`, `workspace.tsx` | the pure line/warnings/details model and its drawing |
| `plugins/project/hooks/context-model.ts`, `context.ts` | levels, wording, the atom and the tail |
| `plugins/project/hooks/handoff.ts` | `nudgeAtTurnEnd` takes the figure already read |
| `plugins/project/hooks/register.tsx` | AbovePrompt composes holds + workspace line; `PromptHint` tail; leader and context recording |
| `plugins/*/types/index.d.ts` | `summary`, `workspaceLeader`, `workspaceExpanded`, `contextPercent` |
| `plugins/project/.claude-plugin/plugin.json`, `README.md` | `workspaceLine`, `contextMeter`, `chromeAscii` |

## Order (tests first)

1. Pure tests: `workspace-model.test.ts`, `context-model.test.ts`, `summaryOf` in each plugin's model tests.
2. Shared contract and copies (`consistency.py --sync-shared-code`).
3. Hook tests: `workspace.test.tsx` (several plugins, critical stays, details, widths, surfaces, headless, ASCII), `context.test.tsx` (levels, zero, max, missing, invalid, refresh, one read per turn, widths), a fold test per band plugin.
4. Implementation, then `bash scripts/check.sh --quick`.

## Not built (API)

Agent activity, auto-mode row, meter colour: see the spec's table.
