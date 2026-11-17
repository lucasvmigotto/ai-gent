# Program test strategy

This is an additive strategy for 046–054, preserving docs/product/test-strategy.md and existing release QA decisions. Every feature is High risk (5 impact × 4 likelihood) because false usage/completion/provenance can unblock incorrect work.

| Layer | Owner | Evidence |
|---|---|---|
| Domain invariants/counter math/graph rules | Build | Rust unit tests; property-style generated edge fixtures for overflow, subsets, dedup and cycles |
| CLI and wire contracts | Build | Versioned valid/invalid JSON and process exit/output fixtures; legacy readers |
| Filesystem/process integration | Build + QA | Real temp repos, locks, pipes and fake hosts; kill/flush/rename/disk-failure injection |
| Independent end-to-end journeys | QA | Real CLI registration → fake build → collect → review/repair → finish → dependency/tracker readiness |
| Security boundary | QA | Actor matrix denials, forged scope/model/session, path escape and hand-edited completion bypass |
| Live host capability | Maintainer/QA | Claude and Codex interactive/detached/native-child probes, mixed provider pairs, version/OS evidence |
| Lower-tier conformance | Adapter maintainer | Generic protocol and OpenCode qualification; other AGENT_IDS explicit unknowns |
| Resource limits | QA | 100k events, 16 writers, bounded records, ≤10 s replay/≤256 MiB RSS target on declared hardware |
| Docs/installation | Build | Existing check.sh installer/consistency/package gates; capability projections match canonical data |

No browser or external database stack is introduced. If implementation modifies existing docs-site views, run that surface's existing accessibility/i18n/browser tests. Do not add a new UI solely to test it.

## CI gates

PR: deterministic unit/contract/integration/fault and compatibility suites, changed-task receipt verifier after 050 strict cutover, secret/schema/link checks, full scripts/check.sh. No paid model calls or provider credentials. Main/develop: existing release workflow remains unchanged until an explicit reviewed follow-on CI patch. Scheduled/manual: real-host version probes, platform durability and resource workload; stale/failed probe cells are unqualified until resolved.

Verified: every FR/SC and both scenarios of every story mapped to passing evidence; no unresolved High defects; independent exact-artifact review; zero unsupported cells labelled qualified. A feature can be Implemented while some host cells remain unqualified; publish that limitation rather than inflating global status.

Fixtures are synthetic, one temporary repository per case, with deterministic cleanup. Keep raw customer data/account IDs/prompts out. Flake policy: rerun only to diagnose, record owner/issue/deadline for quarantine, never silently retry to green. Record suite duration, escaping defects, pending-accounting age and missing-evidence rate; raw coverage or finding count is not the quality gate.

## Essential cross-feature journeys

1. Claude builder → Codex reviewer → accepted receipt → downstream unblocks.
2. Codex builder → fresh Codex reviewer, ordinary policy → eligible; same session → rejected.
3. Provider/model fallback during repair → distinct segments and exact-tree re-review; no lost failed-attempt usage.
4. OpenCode or generic lower-tier wrapped execution → same verifier; null counters → pending/exempt, never measured zero.
5. Crash after worker exit/before durable final usage → recover once, no new model call, acceptance remains pending until closure.
6. Shared planner + exclusive task runs → global total counts shared once, task allocations are estimates.
7. Hand-edit task checkbox, receipt digest, policy baseline or artifact exclusions → CI blocks unauthorized completion.
8. Home machine fetches published handoff branch → all planning artifacts available without the ignored office worktree.
