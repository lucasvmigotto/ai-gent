# Implementation Plan: Checkpoint holds in the session (project mod)

**Branch**: `feat/checkpoint-band` | **Date**: 2026-10-28 | **Spec**: `spec.md`

## Summary

A Claude Code hooks module in the `project` plugin. On `session.start`, on `turn.complete` and before each gated Bash call, it reads `.ai-gent/checkpoint.md` (only when its mtime changed). It keeps the parsed holds in `$.state`, draws them in an `AbovePrompt` band, and puts a `$.ui.ask` in front of `git push|merge|tag`, `gh pr merge` and `gh release create` while holds are listed.

## Technical Context

**Language**: TypeScript hooks module (no Node, no DOM; everything goes through `$`). **API**: the Claude Code plugin hooks API, early access, Claude Code 2.1.312. **Storage**: `$.state` (`project.checkpoint`, shape `checkpoint-1`, and `project.isBandHidden`). **Testing**: `claude plugin test plugins/project` (`hooks/checkpoint.test.tsx`), which stubs `session.cwd`, `process.run`, `fs.stat`, `fs.read` and the AskUserQuestion tool call.

## Files

| File | Role |
|---|---|
| `plugins/project/hooks/hooks.json` | `{ "modules": ["./register.tsx"] }` |
| `plugins/project/hooks/register.tsx` | the hooks: refresh, the gate, the band |
| `plugins/project/hooks/checkpoint.ts` | pure: `section`, `parse`, `gated` |
| `plugins/project/hooks/checkpoint.test.tsx` | parser, gate and band tests |
| `plugins/project/types/index.d.ts` | the `$.state` contract (`plugin.json` `"types"`) |
| `plugins/project/.claude-plugin/plugin.json` | `types`, `userConfig` (`holdsBand`, `holdsGate`) |
| `scripts/check.sh` | `claude plugin test` for each plugin with a hooks module |
| `.github/workflows/check.yml` | `CLAUDE_CODE_VERSION` 2.1.282 → 2.1.312 |
| `.gitignore` | `plugins/*/.claude-plugin/types/` (laid by the engine) |
| `plugins/project/skills/checkpoint/SKILL.md`, `README.md`, `CHANGELOG.md` | the bullet format, the mods table, release notes |

## Risks

- The API moves between releases. CI pins the version, and `check.sh` validates and tests against it, so a drift fails there first.
- A machine with an older Claude Code may not load the module. The plugin's skills don't depend on it.
- Each gated call runs `git log -1`, which costs a few milliseconds.

## Release

`feat(project)` gives a minor release, and `release.py` bumps the `project` plugin's version.
