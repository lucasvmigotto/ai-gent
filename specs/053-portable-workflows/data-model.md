# Data Model: Portable skills, installation and workflow profiles

## Entities and Validation

| Entity | Fields, relationships and constraints |
|---|---|
| WorkflowProfile | artifact breadth, required checks, review diversity and policy version; schema version required, immutable ID and explicit nulls for unavailable evidence |
| CapabilityRow | AGENT_IDS key, capability, tier, version/OS/surface, evidence and status; schema version required, immutable ID and explicit nulls for unavailable evidence |
| InstallationSource | shared skill/native plugin origin, digest, precedence and conflicts; schema version required, immutable ID and explicit nulls for unavailable evidence |
| ArtifactApplicability | project shape, artifact path, applicable/N/A and recorded reason; schema version required, immutable ID and explicit nulls for unavailable evidence |

## Cross-feature Invariants

Task identity is repository UUID + feature slug + task ID; attempts, runs and segments have independent UUIDs. A segment belongs to exactly one run; one run has one role and one registered scope. A request contributing to multiple tasks is shared, not exclusively counted multiple times. Parent/child graphs are acyclic and all references resolve within the repository or an explicit imported namespace.

Usage quality (complete/partial/imported/missing), attribution (exclusive/shared/unattributed), identity verification and attestation (local/reconciled) are separate dimensions. Missing is null, not zero. Raw observations are immutable; corrected normalization is a new version referencing its predecessor, with only one active authoritative contribution.

Artifact identity is a SHA-256 digest of a canonical tree manifest, not the future commit containing the receipt. A review binds this artifact and the builder run. Integration evidence later binds the receipt to a landing revision. No self-referential commit hash is required.

## Lifecycle

Registration precedes launch. Run: registered → running → terminal → draining → closed (complete/partial/missing), with failed/cancelled terminal outcomes preserved. Task: registered → running → execution_finished → accounting_pending/review_pending → accepted → integrated. A failed check may leave review_pending; exemption is a distinct owner-authorized outcome. Only 050 publishes accepted; this feature must not manufacture it.

## Persistence and Concurrency

Journal commands use immutable request IDs and expected generation. Duplicate identical requests are no-ops; conflicting duplicates return conflict. Append and flush precede success. A torn trailing record is quarantined; interior corruption blocks replay and acceptance. Projections can be deleted/rebuilt without deleting evidence. Receipt publication uses a temp file on the same filesystem, durable flush and atomic replacement/create with generation validation; platform durability must be explicitly qualified.

## Retention and Classification

Normalized counters and synthetic model/provider IDs are operational metadata. Repository/task/session identifiers can reveal work patterns: do not export machine paths, account IDs or source text. Raw host sources remain host-private with a proposed 30-day retention default; no automatic deletion in this program. Sanitized receipts and their evidence export are retained with project history. Source cleanup reports reduced ability to reconcile; hashes alone do not restore missing proof.
