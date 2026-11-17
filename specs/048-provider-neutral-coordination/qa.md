# QA: Provider-neutral planning and admitted execution

Risk: impact 5 × likelihood 4 = 20/25, High. False completion or false evidence can unblock incorrect work. Build owns domain/contract tests; QA owns independent CLI journeys, fault scenarios and gate evidence.

## Traceability

| Story/scenario | Requirements | Success criterion | Layer | Planned test |
|---|---|---|---|---|
| US1 positive | FR-001, FR-002, FR-003 | SC-001 | CLI integration | `role_routing_and_admission_positive` |
| US1 negative | FR-001, FR-002, FR-003 | SC-002 | contract + CLI negative | `role_routing_and_admission_negative` |
| US1 replay/fault | FR-001, FR-002, FR-003 | SC-003 | concurrency/fault injection | `role_routing_and_admission_replay_fault` |
| US2 positive | FR-004, FR-005, FR-006 | SC-001 | CLI integration | `dag_ownership_and_slot_races_positive` |
| US2 negative | FR-004, FR-005, FR-006 | SC-002 | contract + CLI negative | `dag_ownership_and_slot_races_negative` |
| US2 replay/fault | FR-004, FR-005, FR-006 | SC-003 | concurrency/fault injection | `dag_ownership_and_slot_races_replay_fault` |
| US3 positive | FR-007, FR-008, FR-009 | SC-001 | CLI integration | `launch_isolation_and_limits_positive` |
| US3 negative | FR-007, FR-008, FR-009 | SC-002 | contract + CLI negative | `launch_isolation_and_limits_negative` |
| US3 replay/fault | FR-007, FR-008, FR-009 | SC-003 | concurrency/fault injection | `launch_isolation_and_limits_replay_fault` |

## Independent Journeys

Run each story's two scenarios from spec.md through real `ai-gent` CLI entry points and a temporary repository. Use owner/supervisor/collector/worker/CI personas from backend.md. Assert persisted evidence and downstream behavior, not only exit text. Add each denied matrix cell, no-answer authorization, tampered IDs/digests, unsupported major version and duplicate request conflict. For host-facing stories run Claude and Codex fake adapters independently; run a synthetic lower-tier adapter with deliberately missing capabilities.

## Failure, Concurrency and Load

Inject full disk, append failure, torn final record, interior corruption, process kill, delayed terminal usage, lock timeout and two competing actors. Prove exactly one authoritative transition/contribution and explicit pending outcomes. Seed 100k synthetic observations, 16 concurrent writers, max 1 MiB record; record replay ≤10 s/RSS ≤256 MiB target on a declared 4-core/8 GiB reference runner. This is a test target, not current measured performance. Resource measurements run scheduled/manual until hardware is stable; deterministic correctness remains required on PRs.

## Data, Environments and Evidence

All fixtures are synthetic; create a temporary repo per test and reset by fixture teardown, never a production profile. Linux PR CI is the deterministic baseline; macOS/Windows check filesystem/launch/path/durability differences before claims. Real Tier 1 interactive, detached and native-child probes require explicit version/OS/surface evidence; compilation and fake executables do not qualify live behavior. Tier 2/3 limitations are recorded per capability. Paid evals are opt-in; no live provider credentials in CI fixtures.

## Exploratory Charter and Exit

Try to claim acceptance with missing final usage, a different artifact, a forged role, an unsupported host and a manually ticked task. Investigate any path that converts a failure to success or unknown to zero. Verified requires all FR/scenario fixtures passing in CI, independent exact-artifact review, no open High defects, compatibility coverage and live evidence for every advertised host cell. No browser/accessibility suite is added because this feature adds no UI; existing docs-site checks still run. Flakes are quarantined only with owner/issue/deadline and remain visible; never retry silently to green.
