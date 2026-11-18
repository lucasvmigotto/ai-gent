# Code-agent parity and required task accounting

Status: complete implementation plan; 046 registration is In progress; features 047–054 remain Planned. Prepared 2026-11-17 from develop 0a61fc4. The owner requests a lifecycle toolbox that works across code agents, equal Claude + Codex Tier 1, and mandatory task-level usage including multi-provider/model builder–reviewer work.

Start with [roadmap](roadmap.md), [architecture](architecture.md), [measurement contract](measurement.md), [capabilities](capabilities.md) and [handoff](handoff.md). The machine-readable [program manifest](program.json) is planning input, not a second runtime agent registry.

## Product Brief Supplement

Problem: current skills are broadly installable, but delegation, history, host assumptions and accounting still favor Claude. A worker exit or task checkbox can imply completion before independently reviewed delivery. Current heuristic token attribution cannot certify actual usage per task or expose mixed builder/reviewer providers accurately.

Users: owner/coordinator choosing execution policy; builder doing scoped work; reviewer assessing an exact artifact; collector recording usage; CI/tracker consumers deciding whether a completion claim is valid; another agent resuming remotely.

Goal: every new strictly accepted task has a durable receipt linking scoped execution, actual provider/model segments, final usage, checks, review and accepted artifact. Claude and Codex use the same domain rules; other agents access those rules through qualified adapters or portable CLI/import workflows. Unsupported telemetry remains explicit rather than zero.

Non-goals: rewriting 3.0.0, changing billing systems, proving a compromised same-user collector honest, generating a new dashboard/TUI, introducing an HTTP API, ranking models by uncontrolled quality scores, or inventing historic token usage.

Success: all 81 feature requirements have named acceptance tests and implementation tasks; all registered new tasks after strict cutover verify receipts before dependencies/trackers unblock; all advertised Tier 1 cells have independent live qualification; every AGENT_IDS entry has a declared lower-tier capability path or limitation. Targets are prospective, not current results.

## Scope and Traceability

| Feature | Deliverable | Audit findings |
|---|---|---|
| 046 | Identities, raw/normalized usage, durable journal, immutable envelope | F02, F10 |
| 047 | Claude/Codex collectors; extension protocol; OpenCode first lower-tier adapter | F10, F12 |
| 048 | Role/capability policy, opt-in admission, DAG/ownership scheduling, launch safety | F01, F03, F05, F07 |
| 049 | Independent exact-tree review, provider/model pairs, findings/repair chains | F02, F06 |
| 050 | Required receipt gate, all consumers, CI, migration and owner exemptions | F02, F05, F10 |
| 051 | All-attempt reporting, builder–reviewer pairs, dated cost estimates | F10 |
| 052 | Host guards/trust/doctor, durable continuation, recap/checkpoint | F04, F07, F08, F09, F12 |
| 053 | Host-neutral skills/install/evals, tiers and workflow applicability | F01, F12, F13 |
| 054 | Honest source verification, inventories/statuses and reproducible claims | F11, F13, F14 |

Original detailed audit and first dependency fix are published on [fix/delegate-dependencies at 7e92b5c](https://github.com/lucasvmigotto/ai-gent/tree/7e92b5c712bb7dbe96cd6bcd481f64e642aae004/docs/product/codex-parity). That fix validates ordering/cycles and accepted worker results; it does not implement this receipt system or separate ownership from DAG scheduling.

## Glossary

TaskKey = repository UUID + feature slug + task ID. Attempt = one attempt at satisfying that task. Run = one registered execution scope/role. Segment = immutable effective provider/model/config interval inside a run. Observation = raw source usage evidence. Contribution = deduplicated authoritative usage selected from observations. Receipt = immutable completion decision bound to evidence and artifact. Integration attestation = later linkage to the landed revision. Complete evidence, exclusive attribution and provider reconciliation are distinct dimensions. A receipt may certify local verification rules; it never automatically certifies an invoice.

## Relationship to Existing Product Artifacts

The old docs/product/brief.md and architecture.md primarily describe the docs site and pre-Rust runners. This supplement extends them using current Rust decisions/specs 032–045; it does not replace the broader brief/domain model. Global HTTP contracts remain N/A. The new constitution applies prospectively; old release/platform waivers are retained. [Migration](migration.md) lists explicit amendments and rollout boundaries.

The first 046 registration slice is described in [development evidence](../../../specs/046-task-usage-ledger/evidence/registration-slice.md). US2 ingestion is documented in [ingestion evidence](../../../specs/046-task-usage-ledger/evidence/ingestion-slice.md). Each feature contains spec, clarification decisions, plan, research, data model, CLI contract, quickstart, Backend plan, QA strategy, phased tasks, quality checklist and analysis. Follow the installed Spec Kit skill files; no slash-command execution or live model evaluation is implied by these documents.

Implementation details: [storage/policy contract](storage-and-policy.md); each feature also has `task-graph.json` for exact local execution dependencies.
