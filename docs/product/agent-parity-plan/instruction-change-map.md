# Proposed instruction and packaging change map

This is a proposal inventory for the build phase, not authorization to edit runtime instructions. Present exact text/diffs before each skill/plugin/hook/shared-reference change under AGENTS.md. Preserve the 50-skill audit inventory on the published audit branch; a broad audit does not imply every skill needs rewriting.

| Existing paths / boundary | Planned change | Owner / evidence |
|---|---|---|
| `plugins/agents/skills/delegate/SKILL.md`; its `references/supervisor.md` and `references/rules.md`; `shared/delegate-worker-rules.md` | Neutral coordinator/reviewer routing, registered execution, admitted plan and receipt-backed finish | 048/049/050 domain; 053 instruction integration |
| `plugins/agents/skills/orchestrate/SKILL.md`; `references/token-economy.md`; `plugins/agents/skills/parallel/SKILL.md` | Separate ownership from dependencies, route by capability, include all model phases, consume exact task graph and completion evidence | 048/050/051/053 |
| `shared/agent-roles.md` | Separate work role, host, provider/model, fresh context and actual permission evidence | 048/049/053 |
| `shared/token-tracking.md`; `plugins/agents/scripts/token_report.py`; Rust `tokens/` | Versioned raw/normalized counters, complete/partial/imported/missing, actual identities, pair metrics, historical compatibility | 046/047/051; preserve existing output readers |
| `shared/token-tags.json` and `shared/token-tags.md` | Change only if a genuinely new controlled tag is needed; model/provider identity belongs in evidence fields, not ad-hoc tags | 051/053; paired consistency gate |
| `plugins/project/skills/recap/SKILL.md`; `plugins/project/skills/checkpoint/SKILL.md`; their existing helper scripts | Explicit host/session adapters, correct identity, bounded/redacted history and remotely reachable handoff | 052/053 |
| `plugins/project/skills/spec/SKILL.md`; `shared/spec-kit.md`; `shared/pipeline.md`; `shared/pipeline-digest.md` | Active-host Spec Kit integration, applicability profiles, receipt-backed statuses and reviewed shared contract changes | 050/053; update all dependent stages deliberately |
| `plugins/qa/skills/e2e/SKILL.md`; frontend/backend build and spec skills | Align actual test tooling and description; consume applicability and receipt gates; retain relevant existing quality requirements | 053; no unrequested browser stack migration |
| Other project/backend/frontend/devsecops/security/db/repo/secrets/git/devcontainer/compact skills | Regression-evaluate host-neutral tools, owner permissions, external writes and artifact references; edit only evidenced incompatibilities | 053 portable trigger/behavior eval inventory; original audit is baseline |
| `setup.sh`, `setup.ps1`, `install.sh`, `install.ps1`, `scripts/package.py`, installer tests | Single AGENT_IDS source, deduplicated entry points, runtime allowlist additions and qualified optional native packaging | 053; platform/package tests |
| `plugins/*/hooks/guard.sh`; Rust `guard/`; `opencode/guard/` | Canonical host payloads and identical shared decisions, truthful unsupported ask/error behavior | 052; shared parity fixtures and live trust probes |
| `README.md`, `CHANGELOG.md`, `AGENTS.md`, `docs/site/` references/guides | Current tiers/capabilities and commands generated or checked against evidence; no claimed measurement before implementation | 053/054 and skill-docs |

Adapters and evidence are shared executable behavior, not nine copies of each skill. Tiers 2/3 continue through generated entry points and the common CLI even without native hooks; their capability gaps remain visible. Packaging must include new runtime schemas/data only if executable code actually needs them after compilation; planning documents and Spec Kit development bootstrap do not become runtime requirements automatically.
