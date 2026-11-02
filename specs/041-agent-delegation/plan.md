# Plan: agent delegation

Approach: a new skill `agents:delegate` in the `agents` plugin, built on `agents:parallel` (worktrees) and `agents:orchestrate` (roles). The adapters and the supervisor checks are scripts under `plugins/agents/scripts/` (Python until the 3.x tools move to the binary, then `ai-gent delegate` subcommands; decide at build time).

Files (proposed, per CLAUDE.md "propose first"): `plugins/agents/skills/delegate/SKILL.md`, `.../references/{adapters,brief,checklist}.md`, `plugins/agents/scripts/delegate.py` and its tests, `shared/delegate-worker-rules.md` (symlinked), a `delegate` tag in `shared/token-tags.json` and `.md`, README and docs-site reference entries, an eval suite under `plugins/agents/evals/`.

Phases: 1 adapter and preflight for Codex with tests that fake the agent; 2 brief builder and the supervisor checks (FR-011) with fixtures from trial 1; 3 scheduler and log; 4 the skill, evals and docs; 5 a second trial and the review requirement (FR-010).
