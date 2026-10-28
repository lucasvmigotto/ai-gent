# Implementation Plan: Secret mask for what the person sends

**Branch**: `docs/mod-specs` (spec) | **Date**: 2026-10-28 | **Spec**: `spec.md`

## Summary

The `secrets` plugin's hooks module hooks `prompt.submit`, `command.run` and `skill.prompt`, rewrites the text with the patterns in `shared/secret-patterns.json` before calling `next`, and toasts what kind was masked. `sessions.py` switches to the same JSON.

## Technical Context

**Language**: TypeScript hooks module; Python 3.9+ stdlib for `sessions.py`. **Engine**: Claude Code 2.1.312. **Events**: `prompt.submit`, `command.run`, `skill.prompt` (all gating; registered with `.catch`). **Storage**: none. **Project Type**: Claude Code plugin module.

## Files

| Path | What |
|---|---|
| `shared/secret-patterns.json` | the pattern list (moved from `plugins/project/scripts/sessions.py`), each `{kind, source, flags, replacement, entry?}` |
| `plugins/secrets/references/secret-patterns.json` | relative symlink to the shared file |
| `plugins/project/references/secret-patterns.json` | relative symlink, read by `sessions.py` |
| `plugins/project/scripts/sessions.py` | loads the JSON (from its plugin's `references/`) instead of the inline list |
| `plugins/secrets/hooks/register.tsx` | module entry (shared with 015) |
| `plugins/secrets/hooks/secret-mask.ts` | load patterns (`$.fs` read of `../references/secret-patterns.json` at register), pre-filter, mask, toast |
| `plugins/secrets/hooks/secret-mask.test.ts` | per-pattern fixtures on the three events; fail-safe; no value in any toast or log |
| `shared/secret-cases.json` | case templates → expected output, expanded at test time so no full token shape is committed; read by `test_scripts.py` (SC-003) |
| `plugins/secrets/hooks/secret-data.ts` | generated copy of the two shared JSON files for the module's tests (they can import only code files); `scripts/consistency.py` keeps it equal |
| `plugins/secrets/.claude-plugin/plugin.json` | `userConfig` `secretMask`, `maskAssignments` |

## Testing

- `claude plugin test plugins/secrets`.
- `python3 -m unittest plugins/project/scripts/test_scripts.py` (already in `scripts/check.sh`), plus the shared-fixture case.
- Fixture secrets are fake but shaped right, so secret scanners (the plugin-authoring scan, a CI secret scan from `devsecops:supply-chain`) may flag the fixture file; build the fixtures from fragments at test time (`"gh" + "p_" + "A".repeat(36)`) so no full token shape is committed.

## check.sh / CI impact

- `secrets` gains a module: covered by 012's loop.
- A `.json` file in `shared/` linked into plugins already exists (`shared/token-tags.json`), so the link pattern is established; confirm `scripts/consistency.py` and `scripts/release.py` treat the new link like that one.
- Release: `project` and `secrets` both change, so both versions bump through `scripts/release.py`'s shared-file rule.

## Risks

- **API churn** (early access): which submissions take a rewrite (`command`, `isMeta`) has already changed across releases; the reference and types are re-read at each pin bump.
- **Regex dialects**: Python `re` and JS `RegExp` differ (inline flags, lookbehind support, `\Z`). FR-003 limits the JSON to the common subset; SC-003 proves equality.
- **Over-masking** annoys; **under-masking** leaks. The pattern list is deliberately the one `project:recap` already trusts, not a new broad one.
- **Self-exposure**: a test that prints a mismatch would print the fixture; fixtures are fake by construction.

## Constitution Check

N/A — no constitution.
