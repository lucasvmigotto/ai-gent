# Implementation Plan: Mods panes say what the system knows

**Spec**: `spec.md` · **Branches**: one per slice · **Plugins**: `project` (Workers, Pipeline) and `agents` (Worktrees), both optional mods

## Approach

Change pure models first, then the drawers; keep data collection as it is. Three slices, landed on `dev` by PR one at a time, in the owner's order. Builder: a Sonnet subagent per slice; an independent Codex review of the diff before each landing (`agents:delegate` review, findings fixed on the slice branch).

### Slice 1 - Workers (`plugins/project/hooks/`)

1. `workers-model.ts` `build`: `active = sorted.filter(w => !isHistory(w) && !needsAttention(w))`; budgets per group; `Idle reason: not recorded` constant; `details()` gains the `Cause` line and keeps `State` and `Verdict` separate.
2. `Line.marks` (from 042) on ATTENTION and WORKERS rows: the ranges cover the glyph and the state word. `workers-draw.tsx` already draws marks as `Text` segments; the toggle row stays one `Button` whose children are those `Text` segments (the 042 pattern, no `Box` in a `Button`).
3. Tests in `workers-model.test.ts` and `workers.test.tsx`; fixtures extended in `workers-fixtures.ts`. Goldens that listed a failed worker twice are updated one by one, not mass-regenerated.
4. README row and CHANGELOG `## Unreleased` line; the PR body says `/reload-plugins` is needed after merge.

### Slice 2 - Worktrees (`plugins/agents/hooks/`)

1. Spike first (T010): a `ui.render` test that mounts a `Pane` with one `Button` row among `Text` lines; if the test host refuses it, stop and ask the owner (Q2).
2. `worktrees-model.ts`: `counts()` to `↑a ↓b`/`^a vb` (a `ascii` flag in `Board`, set from the existing glyph option if the agents mod has one; otherwise from a new read-only check of the same option the project mod uses, **not verified**: first read `register.tsx` and `plugins/agents/hooks/hooks.json` userConfig); drop `checks failing` from `prCell`; `Board` gains `local` and `remote` freshness records `{ attemptedAt, okAt, error }`; `view` returns lines with an optional `toggle` key, like the Workers model; `text` prints all open.
3. `worktrees.ts`: the polls write `attemptedAt` at start and `okAt` only when `parseWt`/`parseGh` return non-null; a null parse sets `error` and keeps the rows (`carryPrs` already carries PRs). The atoms (`localAtAtom`, `remoteAtAtom`) are kept or replaced by one `worktreesFreshAtom`; the type is declared in `plugins/agents/types/index.d.ts` (an atom is read only where it is declared).
4. `register.tsx`: the Pane render maps a line with a `toggle` to a `Button` (label text only) and others to `Text`; `onPress` writes `worktreesExpanded`.
5. Tests and fixtures as in the tasks; 022's tests that assert `+3/-0` are updated by hand to the arrow form.

### Slice 3 - Pipeline (`plugins/project/hooks/`)

1. `pipeline-view.ts` `build`: the `expanded` map gains the `artifacts` key; `artifactsOpen = ctx.expanded.artifacts ?? hasProblem` where `hasProblem` is missing-or-stale among applicable artifacts; the summary gains `missing`, `stale`, `na` counts (`notApplicable` already exists).
2. `pipeline-draw.tsx`: move the `next` block above `progress`; the `artifacts` block draws a head with a `Button` and the table only when open; the Readiness and narrow `Current stage:` line as in FR-034.
3. `pipeline.ts`: no new atom (`pipelineExpanded` is a free-key map). The compact text in `text()` follows the same order.
4. Tests update 029's goldens deliberately: each changed golden is read before it is accepted.

## Risks

- A Button in the Worktrees pane is unverified in the host (Q2); the spike is the first task of slice 2 and the fallback is a printed-open list.
- `Line.marks` ranges break if a line is re-cut after marking (042 risk): the rows are fitted before the marks are computed; a test checks the segments concatenate to the text.
- Moving Next action up changes every Pipeline golden: reviewed one by one.
- Two refresh sources in the Worktrees pane: a failure of one must not erase the other's rows (test: wt ok and gh fails, and the reverse).
- `↑ ↓` are single-width in common terminals; **not verified** on every font, hence the ASCII set and the legend note.
- Plugin cache: an installed mod serves its old module until `/reload-plugins`.

## Verification

Per slice: `claude plugin test plugins/<name>`, `claude plugin validate plugins/<name>`, `scripts/check.sh --quick --quiet` in the loop, the full `scripts/check.sh` once before the last commit, a mutation pass (revert the attention filter; keep `+a/-b`; keep always-open Artifacts; move `okAt` on failure; each must fail a test). Then the Codex review and the PR. Real-terminal screenshots are not produced by this spec (not verified); the golden texts are the evidence.
