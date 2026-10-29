# Implementation Plan: Repo hygiene band, session-start toasts, release preview

**Branch**: `docs/mod-specs` (spec) | **Date**: 2026-10-28 | **Spec**: `spec.md`

## Summary

In the `git` plugin's hooks module: an `AbovePrompt` band fed by a few `git` calls (`$.process.run`), a `classic.SessionStart` hook that toasts `update-check.sh`'s line, and `/release-preview`. In the `devcontainer` plugin's module: a `classic.SessionStart` hook that toasts `leftovers.sh`'s block.

## Technical Context

**Language**: TypeScript hooks modules. **Engine**: Claude Code 2.1.312. **Events**: `ui.render` (`AbovePrompt`), `classic.SessionStart`, `session.start`, `tool.call` (after `next`, Bash git commands), `turn.complete`, `command.run` (`$.command.register`). **Storage**: `$.state`. **Project Type**: Claude Code plugin modules.

## Git calls (all read-only)

| Purpose | Command |
|---|---|
| root, and whether it's a checkout | `git rev-parse --path-format=absolute --git-common-dir` |
| chain branches present | `git for-each-ref --format=%(refname) refs/heads/main refs/heads/master refs/heads/dev refs/heads/develop refs/remotes/origin/…` |
| ahead / behind | `git rev-list --left-right --count <b>...origin/<b>` |
| release subjects behind | `git log --format=%s <b>..origin/<b>` |
| last fetch age | `$.fs.stat("<common dir>/FETCH_HEAD").mtimeMs` |

## Files

| Path | What |
|---|---|
| `plugins/git/hooks/register.tsx` | module entry (shared with 015's git part) |
| `plugins/git/hooks/hygiene-model.ts` | pure: git's counts, the release subjects, the band's wording |
| `plugins/git/hooks/hygiene.ts` | the git calls, the atoms, the update relay, `/release-preview` |
| `plugins/git/hooks/hygiene.test.tsx` | stories 1–3, git mocked |
| `shared/hook-lines.ts` → `plugins/{git,devcontainer}/hooks/hook-lines.ts` | copies: read the SessionStart hook's stored row |
| `plugins/git/types/index.d.ts` | the `hygiene` and `toasted` atoms |
| `plugins/devcontainer/hooks/register.tsx` | module entry (shared with 015's containers part) |
| `plugins/devcontainer/hooks/leftovers.ts`, `leftovers.test.ts`, `plugins/devcontainer/types/index.d.ts` | the relay, its test and its state |
| `plugins/git/.claude-plugin/plugin.json`, `plugins/devcontainer/.claude-plugin/plugin.json` | `userConfig` flags (FR-007), `"types"` |
| `plugins/git/hooks/hooks.json`, `plugins/devcontainer/hooks/hooks.json` | gain `"modules"` beside the command hooks |

## Testing

- `claude plugin test plugins/git` and `plugins/devcontainer`; `$.process.run` mocked with recorded git outputs (never a real repository), and a test `classic.SessionStart` hook beneath supplying the scripts' lines.
- Existing `scripts/test-update-check.sh`, `scripts/test-containers-guard.sh`, `scripts/test-release.sh` unchanged.
- Manual: on this repository after the next release, before and after `git pull --ff-only` on `dev` and `main`.

## check.sh / CI impact

012's loop picks up two more plugins with a module (if 015 hasn't added them already), and `scripts/consistency.py` gains the release-subject check (with a case in `scripts/test_consistency.py`).

## Risks

- **API churn** (early access): `AbovePrompt` props (`hasSurvey`, `isWorking`, rows) and `classic.SessionStart`'s result shape. Validate at every pin bump.
- **Subject matching** couples the band to `.github/workflows/release.yml` and `shared/release-platforms.md`; a check in `scripts/consistency.py` compares the band's two patterns with both files' `chore(release):` subject templates, so a change fails `check.sh` (the files sit outside the plugin, out of `claude plugin test`'s reach).
- **Git cost**: four short calls per refresh, throttled; git runs with repository hooks off under `$.process.run`.
- **False comfort**: the band is only as fresh as the last fetch; the age note says so.

## Constitution Check

N/A — no constitution.
