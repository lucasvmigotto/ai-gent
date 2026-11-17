# Backend/CLI Layer: Source, inventory and status evidence integrity

OpenAPI and AsyncAPI remain N/A: no network API or external event bus is introduced. The authoritative boundary is [contracts/cli.md](contracts/cli.md), not a fictional HTTP service.

## Operations and Authorization

| Actor | Read status/export | Register/dispatch | Append registered observations | Accept result | Waive/approve |
|---|---|---|---|---|---|
| Owner | Allow sanitized output | Conditional on explicit opt-in/policy | Through collector validation | Through verifier only | Allow explicit recorded authorization |
| Coordinator/supervisor | Allow | Approved plan only | Through scoped collector | Through verifier only | Deny |
| Worker | Own scoped read | Deny direct admission | Deny authoritative ingestion; submit evidence to collector | Deny | Deny |
| Collector | Own source/cursor | Deny | Own run only, schema checked | Deny | Deny |
| CI/read consumer | Allow sanitized evidence | Deny | Deny | Verify only | Deny |

Every deny cell needs a negative integration test; same-user OS access does not itself enforce this logical matrix. Strict unattended collection requires a supervisor-owned/protected store or separate trust boundary. If the host cannot provide that boundary, doctor states local integrity only and makes no adversarial attestation claim.

## Persistence and Transactions

Use 046 identities and journal uniqueness rules. This feature owns `rust/crates/ai-gent-core/src/evidence`; shared CLI entry points are serialized in the program roadmap. Keep data-model.md invariants in constructors and replay validation, not only command-line checks. Acquire locks in repository-ledger → aggregate generation order; never hold a ledger lock during a model process, remote request or collector drain. Publish immutable evidence before projection updates; recover by replay if projection update fails.

## Async Work and External Integrations

Collectors and worker supervisors are bounded processes, not mandatory daemons. Persist cursor/lease before acknowledging work; at-least-once delivery with idempotent ingestion. Default helper timeout 30 s; drain 10 s with maximum 60 s; at most 3 transport retries with bounded backoff and the original request ID. Never retry a paid model call automatically as telemetry recovery. Fail closed for required acceptance evidence; status remains readable with pending reasons.

## Observability and Security

Structured events include operation, repository pseudonymous ID, task/attempt/run references, outcome, duration, quality and safe error code. No prompt text, source path, account token or raw command is logged. Track ingest/replay failures, unclosed runs, age of accounting_pending and qualification gaps. Alert via CLI diagnostics; a production monitoring service is not introduced. Validate bounds before allocation/parsing; reject traversal, symlink escape, non-regular sources and cross-scope writes. Secrets remain under shared/secrets.md; collectors do not read credentials.

## Performance and Migration

Use the plan's 100k-event/16-writer qualification workload; measure replay wall time, peak RSS and lock timeouts on recorded hardware. Large reports paginate, collectors stream, projections avoid rescanning raw transcripts on every status read. Expand with v1 readers and observe-only writes first; preserve existing JSON, backfill only explicit imports, then enable 050 strict policy after compatibility tests. Rollback disables new dispatch/strict rollout explicitly and preserves journal/receipts; never downgrades evidence labels silently.

## Test Plan

Unit tests exercise domain validation and transition rules. CLI integration tests in `rust/crates/ai-gent/tests/agent_054.rs` exercise all operations, authorization cells, retries and error exits. Contract tests load success/rejected JSON fixtures and old output readers. Fault tests inject append/flush/rename/lock failures; host processes are fakes in required CI, separately qualified with live probes. No external DB/service/container stack is needed for these synthetic tests.
