# Implementation Plan: Release readiness band

**Branch**: `docs/mod-specs-2` (spec) | **Date**: 2026-10-31 | **Spec**: `spec.md`

## Summary

A prior change adds `--json` to `scripts/release.py` and `shared/release-single.py` together. Then, in the `git` plugin's hooks module: an `AbovePrompt` band fed by `release.py --dry-run --json`, run once per distinct key (HEAD plus the tags merged into it) on a chain branch, through the call `releasePreview` already makes, parsed into version, bump, base tag and plugin count. The band names `/release-preview` for the full plan and obeys the band-priority rule (priority `info`).

## Technical Context

**Language**: TypeScript hooks module; Python for the prior change. **Engine**: Claude Code 2.1.312. **Events**: `ui.render` (`AbovePrompt`), `session.start` (registers nothing slow), `turn.complete`, `tool.call` (after `next`, Bash, `MOVES_REFS` plus `tag`). **Storage**: `$.state` atoms (`releasePlan`, `releasePlanFor`, `releaseBandHidden`; `data-model.md`), shape-tagged. **Project Type**: Claude Code plugin module.

## Read-only calls

| Purpose | Call |
|---|---|
| the plan | `python3 <top>/scripts/release.py --dry-run --json` (exit 0 plan, 3 none, 1/2 error; the same call as `releasePreview` in `plugins/git/hooks/hygiene.ts` plus `--json`) |
| HEAD | `git rev-parse HEAD` |
| the current branch | `git rev-parse --abbrev-ref HEAD` (must be in `CHAIN`) |
| the merged tags (half of the key) | `git tag --list --merged HEAD` (what `release.py`'s `last_tag` reads, `scripts/release.py:90`) |
| the repository root | `git rev-parse --show-toplevel` |
| is there a script | `$.fs.exists("<top>/scripts/release.py")` |

## Files

| Path | What |
|---|---|
| `scripts/release.py` | `--json` (needs `--dry-run`, else `parser.error`, exit 2); prints the object in `data-model.md` (Planned, prior change) |
| `shared/release-single.py` | the same flag and keys; its single `source` target (Planned, prior change) |
| `scripts/test-release.sh`, `scripts/test-release-single.sh` | cases for `--json`: keys, exit 3 with no stdout, `--json` without `--dry-run` refused (Planned) |
| `shared/releasing.md` | documents `--json` beside the exit contract (Planned) |
| `scripts/consistency.py`, `scripts/test_consistency.py` | the key lists of both scripts and of `release-model.ts` are equal (replaces the text-pattern check this plan first proposed) |
| `plugins/git/hooks/register.tsx` | 017's module; gains the band's lines |
| `plugins/git/hooks/release-model.ts` | pure: JSON to plan, the band's wording (wide and narrow), the chain-branch rule, the plugin count (Planned) |
| `plugins/git/hooks/release.ts` | the throttled run, the key, the atoms; factors `releasePreview`'s script call into one function both use (Planned) |
| `plugins/git/hooks/hygiene.ts` | `releasePreview` calls the shared runner; behavior unchanged |
| `plugins/git/hooks/release.test.tsx` | stories 1–3, recorded JSON (Planned) |
| `plugins/git/types/index.d.ts` | the `release*` atoms |
| `plugins/git/.claude-plugin/plugin.json` | `userConfig`: `releaseBand` |

No change to `git:workflow`, the guard or the bump rules. The plugins that ship `shared/release-single.py` copies (set up in other projects by `devsecops:pipeline`) refresh their script when the skill runs again.

## Testing

- `claude plugin test plugins/git`; `$.process.run` mocked with recorded `--dry-run --json` outputs (a minor plan with three `plugin` targets, a patch plan, the first release with `since: null`, a `source`-only plan, exit 3, exit 1, exit 2 for an old script) and with branches in and outside `CHAIN`.
- `scripts/test-release.sh` and `scripts/test-release-single.sh` extended for `--json`; the existing text-plan cases unchanged.
- `scripts/consistency.py` and a case in `scripts/test_consistency.py`: the key lists are equal.
- Manual: on this repository on `dev` before and after a `feat` commit, on a feature branch (no band), and after a pull that brings in a release.

## Implementation order (tests first)

1. Prior change on its own branch: failing `--json` cases in both release test scripts, then `--json` in `release.py` and `release-single.py`, `shared/releasing.md`, the consistency check. Lands on `dev` before the mod (it is a `feat`: a minor bump of the plugins that link the shared file, as `release.py` computes).
2. Atoms, `userConfig`; `release-model.ts` JSON parse, plugin-count rule and wording, with unit tests on recorded JSON.
3. Factor the script call out of `releasePreview`; keep 017's tests green.
4. Failing story 1–3 tests (band, chain-branch gating, once per key, hide, off, source-only plan).
5. `release.ts`: key, detached run, the atoms; compose the band in `register.tsx` with its `info` priority.
6. `scripts/check.sh`, docs row (`skill-docs`), manual run.

## check.sh / CI impact

`claude plugin test` already runs for `git` (017). `scripts/consistency.py` gains one check and a test case; the two release test scripts gain cases.

## Risks

- **Two scripts, one shape**: `release.py` and `release-single.py` drift; mitigated by the consistency check on the key lists and by changing both in the prior change (CLAUDE.md "change the bump rules in both").
- **Old script copies** in other projects exit 2 on `--json`; the band shows its failure line once and says how to refresh.
- **A wrong plan on a stale or feature branch**: gated to `CHAIN` and annotated when local lags origin.
- **Cost**: one run per distinct key, 60 s timeout, detached from session start.
- **Two bands for one condition** (017's unpulled release and this): they say different things and compose; the priority rule keeps at most two visible and this one yields first.

## Constitution Check

N/A — no constitution.
