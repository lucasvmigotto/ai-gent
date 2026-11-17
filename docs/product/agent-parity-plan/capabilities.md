# Agent tiers and capability qualification

Owner decision, 2026-11-17: **Tier 1 = Claude Code + Codex**. This is the target for this feature program, not a claim that existing Codex paths already meet it. Existing spec 039 D2 still describes Codex as Tier 2 for its release scope; 053 must publish an explicit successor amendment before advertising the new guarantee. No agent is removed.

| Tier | Promise for this program | Qualification/gates |
|---|---|---|
| 1 | Equal contract, collection, admission, review and completion semantics for Claude + Codex, including mixed-host runs | Required deterministic CI for both adapters; real version/OS/surface probes for every advertised live capability |
| 2 | Integrated adapter for supported capabilities; same core receipts/verifier; specific gaps labelled | Required conformance for promoted adapter; live evidence per promoted capability; OpenCode first candidate, not pre-qualified |
| 3 | Portable generated skills and CLI/wrapper/import path where shell access exists | Registry/install smoke and protocol validation; best-effort host integration; no fabricated telemetry or enforcement |

The tier does not erase a missing capability. Unsupported host surfaces can still plan/build and preserve work; strict measured acceptance needs adequate evidence or an explicitly recorded exemption. Do not automatically downgrade all other agents to zero support or promise all host-native UI features everywhere.

## Coverage matrix to implement

Source of agent identities remains `setup.sh` AGENT_IDS; this table is a planning inventory only. Required dimensions are skills, CLI launch, telemetry, effective model/provider identity, terminal closure, independent review context, effective restrictions, guard decisions, recovery/history and packaging. Every cell additionally keys by OS, version and surface (interactive/detached/native child).

| Agent | Target | Planned usable path | Live qualification at planning |
|---|---|---|---|
| Claude Code | Tier 1 | native/detached collector + CLI gate + hooks where supported | new collector/receipt system not implemented |
| Codex | Tier 1 | native/detached collector + CLI gate + trusted hook normalizer | new collector/receipt system not implemented |
| OpenCode | Tier 2 candidate | event/transcript adapter + common CLI gate | adapter qualification pending |
| Cursor | Tier 3 initially | shared skills + wrapper/import + CLI gate | telemetry/host restrictions unknown |
| Antigravity | Tier 3 initially | generated skills + wrapper/import + CLI gate | telemetry/host restrictions unknown |
| GitHub Copilot | Tier 3 initially | shared skills + wrapper/import + CLI gate | telemetry/host restrictions unknown |
| Amp | Tier 3 initially | shared skills + wrapper/import + CLI gate | telemetry/host restrictions unknown |
| Kiro | Tier 3 initially | generated skills + wrapper/import + CLI gate | telemetry/host restrictions unknown |
| Windsurf | Tier 3 initially | generated skills + wrapper/import + CLI gate | telemetry/host restrictions unknown |

A host without shell/process access needs an external supervisor integration; portable Markdown alone does not enforce completion. Never equate successful stub installation with a working hook, collector, independent reviewer or fail-closed tool gate.

## Qualification protocol

Record host/tool version, OS/architecture, source revision, surface, adapter version, configuration/trust digest, commands, expected/observed output, exit status, normalized payload fixture and evidence digest. Run positive, unsupported and timeout/failure probes, including mixed builder/reviewer providers and same-host fresh-context review. Confirm final usage occurs after terminal execution and is not counted twice. Use synthetic tasks with no production data; paid calls are an opt-in qualification activity.

Required CI tests fakes/contract fixtures without credentials. A scheduled or manual live probe records support evidence; stale upstream versions become `unqualified`, not implicitly supported. Release support minimum is the lowest version actually qualified, not whichever version was installed during planning. Installed Codex 0.169.0 and documented hooks are research evidence, not proof of this new integration.

## Decision fallback

If a host lacks an ask decision, use a documented native approval route if qualified; otherwise block the requested protected action or label the operation advisory outside strict enforcement. Errors/timeouts may be fail-open upstream: a successful local script test does not change that. Native subagents inheriting parent permissions do not satisfy an enforced read-only reviewer requirement without an additional qualified boundary.
