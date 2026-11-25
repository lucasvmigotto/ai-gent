# CLI Contract: Task identity and durable usage ledger

Status: v1 design contract with partial implementation. Registration/segment/transition/replay are implemented as described in `registration.md`; ingest is implemented as described in `usage-ingestion.md`. Local recovery/snapshots are implemented as described in `recovery-and-snapshots.md`; bounded export/import are described in `evidence-transfer.md`. Protected collector authorization, completion receipts and pagination remain design targets. Operation IDs below are stable boundary names; implementation maps them into existing `ai-gent delegate`, `tokens`, `doctor`, `spec` or new `usage` groups without two overlapping public commands.

| Operation ID | Actor | Purpose |
|---|---|---|
| register | coordinator | Create immutable scope and return registration/run IDs |
| ingest | collector | Append one bounded source observation idempotently |
| replay | collector | Recover journal and rebuild disposable projection |
| export | owner | Export bounded pseudonymized evidence with digests |
| import | owner | Validate/dry-run or archive foreign evidence separately from local execution totals |

## Shared Request/Response Envelope

Every mutation accepts schema_version=1, request_id (UUID), repository_id (UUID), operation, payload and expected_generation (integer ≥0 when updating an aggregate). Registered work additionally requires feature_id, task_id, attempt_id and run_id as applicable. JSON input is UTF-8, at most 1 MiB, with no prompt/credential fields. Schema definitions and the successful/rejected usage/receipt examples in 046 and 050 fix the shared evidence wire format.

Success JSON: `{ "schema_version": 1, "operation": "verify", "ok": true, "generation": 2, "result": { "outcome": "accepted", "receipt_digest": "sha256:…" }, "warnings": [] }`. This is an illustrative envelope; only 050 verification may return accepted, and a real digest is 64 lowercase hexadecimal characters.

Error JSON: `{ "schema_version": 1, "ok": false, "error": { "code": "evidence_missing", "retryable": false, "message": "run has no terminal usage", "references": ["run UUID"] } }`. No stack trace, prompt, command secret or host account data is exposed.

Reads default to human output; `--json` emits one machine result and diagnostics to stderr. A streaming collector emits newline-delimited envelopes with bounded records. No operation changes state merely by rendering text or a task checkbox. Report pagination defaults to 100, maximum 1000, ordered by durable sequence then event ID with opaque cursor.

## Errors and Exit Status

0 = operation succeeded (a read may report pending in result); 2 = invalid arguments/schema or unsupported major; 3 = failed policy/precondition, missing evidence or negative verification; 4 = stale generation/identity conflict; 5 = I/O, corruption or timeout. Mutating finish returns 3 while pending and includes reasons. Read-only status returns 0 for a valid pending status. Retryability is explicit: lock timeout/transient I/O may be retried using the same request ID; invalid evidence or unsupported host needs intervention.

Codes include invalid_schema, unsupported_version, identity_conflict, stale_generation, scope_mismatch, capability_missing, policy_denied, evidence_missing, evidence_corrupt, review_stale, check_failed, limit_exceeded, lock_timeout and io_failure. Each operation must list its exercised subset in CLI fixtures; unexpected failures return io_failure with sanitized diagnostics, never success.

## Authorization, Idempotency and Versioning

The supervisor registers scoped collector credentials or protected process handles; a worker-provided role string is untrusted. Collector permissions permit only observations for its registered run. Read consumers cannot mutate. Owner-only approvals/waivers require explicit actor/authorization evidence and cannot be synthesized by a worker. Local same-user compromise remains outside the local integrity guarantee.

Identical request_id + canonical body returns the prior result; a different body conflicts. Generation checks serialize state transitions. Unknown optional extension fields are preserved only in the declared extensions object; unknown required versions are rejected. Historical documents have no implicit v1 certification. Migration is dry-run first, with a backup and reversible projections.
