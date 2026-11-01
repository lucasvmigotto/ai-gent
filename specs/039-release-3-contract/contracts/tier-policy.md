# Contract: support tiers (D2)

| Tier | Agents | Promise |
|---|---|---|
| 1 | Claude Code | Everything: skills, enforced guards, mods, update notice, evals, required CI |
| 2 | Codex | Skills and the binary's tools installed and tested in required CI; guards enforced if Codex can run hook commands (unverified, T032), else advisory |
| 3 | opencode, Cursor, Antigravity, GitHub Copilot, Amp, Kiro, Windsurf | Best-effort and frozen: skills install through generated stubs; no new tier-3-only features; regressions fixed by PR when reproducible; no SLA; never a release blocker; tests run in a non-required job |

## Feature by tier

"Yes" means shipped and covered by the stated CI. "Best-effort" means shipped, run in the non-required job, not a blocker. "N/A" means the agent has no such surface.

| Feature | Claude Code (1) | Codex (2) | Others (3) |
|---|---|---|---|
| Plugin skills | Yes: `/plugin:skill`, symlinks, required CI (`check.sh`, `claude plugin validate`) | Yes: generated `plugin-skill` stubs in `~/.agents/skills`, required install test | Best-effort: same stubs (`~/.agents/skills` for Cursor, Copilot, Amp; Antigravity, Kiro, Windsurf own dirs) |
| Personal skills | Yes | Yes | Best-effort |
| `ai-gent` binary tools called from skills | Yes (shell tool) | Yes (shell tool) | Best-effort: needs the agent to run commands |
| Guards (git, db, secrets, repo, containers) | Yes: hooks call `ai-gent guard`, deny/ask enforced | Enforced only if hook commands are supported (T032); otherwise instructions | opencode: plugin calls the binary (best-effort); Cursor, Antigravity, Copilot, Amp, Kiro, Windsurf: instructions only (as README today) |
| Mods (bands, panes, status line, toasts) | Yes (Claude Code mods; engine 2.1.312 pin) | N/A | N/A |
| Secret masking at prompt and result | Yes (hooks module) | No | No |
| Update notice at session start | Yes (SessionStart hook runs `ai-gent update --check`) | No: run `ai-gent update --check` by hand | No: same |
| `ai-gent update`, `rollback`, `doctor` | Yes | Yes | Yes (agent-independent CLI) |
| Model-role routing (`shared/agent-roles.md`) | Yes (subagents) | Yes (custom agents) | Best-effort (opencode commands as subtasks; others custom agents where they exist) |
| Evals (`claude plugin eval`) | Yes (not in CI) | N/A | N/A |
| Install paths and `--target` | Yes (Windows: `install.ps1` + `setup.ps1`) | Yes (Windows: same) | Yes; `--target all` still includes them; on Windows through Git Bash or WSL |
| Required CI | Linux `check`, macOS installer job, Windows binary and guard job, six-target build | Codex install case in `scripts/test-install.sh` (required) | install cases in a separate `tier3` job, `continue-on-error` |
| Docs-site compatibility matrix | green | green | "best-effort" |

## Tier-3 support policy

1. Frozen: no new capabilities are built for tier 3 alone; shared work (a skill, a guard rule, a binary subcommand) reaches them for free.
2. Fixes: a reproducible regression reported with the agent version is fixed on `dev` in the next release; no promised time.
3. CI: tier-3 install cases run in `tier3` (non-required). A red `tier3` job opens an issue, not a release block.
4. Agent API drift: if an agent changes its skills directory or format, the entry in `AGENT_IDS` is updated by PR; if the agent cannot be supported for two consecutive minor releases, the README says "unsupported" and the next major may drop it. Nothing is dropped in 3.0.0.
5. Honesty: the README and docs site label tier 3 as best-effort; claims of enforced guards are made only for tiers with a tested enforcement path.
