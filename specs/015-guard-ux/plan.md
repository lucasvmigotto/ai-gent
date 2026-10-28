# Implementation Plan: Guard UX

**Branch**: `docs/mod-specs` (spec) | **Date**: 2026-10-28 | **Spec**: `spec.md` | **Research**: `research.md`

## Summary

Five small hooks modules (git, db, secrets, repo, devcontainer), each hooking `classic.PreToolUse`: call `next(e)`, and when the result is a deny or ask whose reason carries this plugin's guard prefix, enrich it — a toast/notice for the person and `additionalContext` for the model on a deny, a fuller reason on an ask. The explanation table and logic are one shared file.

## Technical Context

**Language**: TypeScript hooks modules. **Engine**: Claude Code 2.1.312. **Events**: `classic.PreToolUse` (gating; registered with a `.catch` that replays `next`). **Storage**: none. **Project Type**: Claude Code plugin modules.

## Files

| Path | What |
|---|---|
| `shared/guard-ux.ts` | the explanation table (prefix → rule, skill, alternative), `decorate(result, prefix)`, the `.catch` helper |
| `plugins/{git,db,secrets,repo,devcontainer}/references/guard-ux.ts` | relative symlinks to `../../../shared/guard-ux.ts` (R4, open question) |
| `plugins/{git,db,secrets,repo,devcontainer}/hooks/register.tsx` | one per plugin; `git` and `secrets` and `devcontainer` share theirs with 017 and 016 |
| `plugins/{git,db,secrets,repo,devcontainer}/hooks/hooks.json` | gain `"modules": ["./register.tsx"]` beside the existing command hooks |
| `plugins/<p>/hooks/guard-ux.test.ts` | deny, ask, fail-safe, reason coverage (reads `hooks/guard.sh`) |
| `plugins/<p>/.claude-plugin/plugin.json` | `userConfig` `guardUx` (boolean, default true), `guardDialog` (boolean, default false) |

No change to `plugins/*/hooks/guard.sh`, `opencode/guard/`, or the `scripts/test-*-guard.sh` suites.

## Testing

- `claude plugin test` per plugin (012's loop in `scripts/check.sh`). Each test installs a `classic.PreToolUse` hook beneath that answers the decision `guard.sh` would print, so the test does not depend on R1.
- Reason coverage: the test reads `hooks/guard.sh` from the plugin, extracts every `decide deny|ask "…"` reason and checks the table.
- Existing guard suites and the parity test run unchanged (SC-002).
- Manual, once per Claude Code pin bump: R1 and R2 checks with `claude --debug`.

## check.sh / CI impact

- Five more plugins with a module: 012's loop covers them.
- If R4's fallback is needed: `scripts/consistency.py` gains an identical-copies check.

## Risks

- **API churn** (early access): `classic.PreToolUse`'s position in the chain and its result shape are the whole design. R1 is re-checked at every pin bump.
- **A decorating hook that changes a decision** would be a security bug: the `.catch` replay, FR-002/003 tests and "never compute a decision" keep that out.
- **Reason drift**: guard reasons are written for the model; the table translates them for people. Coverage test catches new reasons, not reworded ones — the table keys on stable fragments.
- **Five modules on one event**: each runs `next` once; order between them doesn't matter because each only reads.

## Constitution Check

N/A — no constitution.
