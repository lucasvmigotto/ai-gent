# Portable evidence transfer — bounded 046 FR-009 slice

`usage export` and `usage import` move a sanitized, versioned evidence archive between repositories or machines. An imported archive is isolated from the destination's authoritative registration/usage journals and receipt chain. Import does not register or execute work, add measured usage, certify a provider/model, accept a task, or reconcile overlapping archives. The bundle is marked `quality: imported`, `assurance: local_unverified` and `coverage: unclosed` even when its numerical counters are complete.

## Export

```sh
ai-gent --json usage --root <project> export
```

Export reads a consistent ledger under its existing repository lock, validates both journals and every published snapshot's bound evidence, and emits `{schema_version:1,operation:"export",ok:true,result:{bundle_digest,bundle},warnings:[...]}`. It creates no files and refuses a missing or corrupt local ledger. The digest is SHA-256 of canonical sorted-key JSON bundle bytes. Save `result.bundle` for transport; the CLI does not write a caller-selected output path. The non-JSON form emits the bundle as JSON.

The bundle follows `evidence-transfer.schema.json`. It contains source repository UUID, current registered run/attempt/phase/role/purpose/segment/configuration/scope metadata and numeric source observations. There is no caller-supplied aggregate summary; import recomputes normalization, contribution selection, totals and lower bounds.

Local `host_session_id` fields become null. Source namespace (including its original session), adapter/version, request and source event identifiers become deterministic SHA-256 pseudonyms. Source references become opaque `evidence:sha256/<hex>` values. Unrecognized raw-counter keys and semantics labels are hashed; recognized arithmetic vocabulary remains readable. This preserves source-stream separation and deduplication while removing these direct source identifiers/paths. Original raw numeric values, explicit unknowns, source digest, real validated UTC timestamps, TaskKeys/UUIDs, model/provider/effort/vendor/tier/region and agent application/version labels remain. Declared observation quality is preserved as an unverified source claim independently of the archive's imported classification.

This is pseudonymization, not guaranteed anonymization. Retained operational labels are caller supplied and can themselves contain identifying text; the exporter cannot infer that a model alias is an account name. Unsalted pseudonyms permit correlation and dictionary guesses. Review operational labels before sharing an archive. Prompts, raw transcripts, task prose, machine paths from source references, direct host session IDs, credentials and arbitrary extensions are not transfer fields. The format does not accept arbitrary extra nested objects.

## Import dry run and explicit apply

Wrap the bundle and its digest in a schema-v1 request:

```json
{
  "schema_version": 1,
  "request_id": "80000000-0000-4000-8000-000000000001",
  "repository_id": "20000000-0000-4000-8000-000000000001",
  "operation": "import",
  "payload": {
    "bundle_digest": "sha256:<64 lowercase hex>",
    "bundle": { "...": "the complete exported bundle" }
  }
}
```

Here `repository_id` identifies the **source** bundle, not a claim to execute in the destination. It must match the bundle. Request UUIDs share the destination's global mutation namespace with registration, ingestion, snapshots and recovery.

```sh
ai-gent --json usage --root <destination> import --input request.json --dry-run
ai-gent --json usage --root <destination> import --input request.json
```

Dry run validates the exact digest, full nested format, task/scope and run/attempt/segment references, acyclic parents, phase/role/state-generation relationships, immutable effective identity across streams, normalization and deduplicated contributions. It inspects existing destination evidence and import conflicts without creating directories, locks, journals or archives, including on an empty destination. Unknown fields, omitted required nullable fields, unsupported majors, malformed identities, invalid subsets/overflow, conflicting source events or a forged summary are refused. The incoming source keys must have the specified pseudonym form; raw identifiers cannot masquerade as sanitized keys. Import does not authenticate that a caller supplied truthful model names or counters.

Apply repeats validation under the destination lock and atomically publishes canonical bundle bytes as `.ai-gent/usage/imports/<bundle hash>.json`, then an immutable `import-<request UUID>.json` manifest binding request and bundle digests. It creates the private self-ignore marker before publication. Files use existing private-store permissions. Same-filesystem hard-link publication is no-clobber and durable before acknowledgement; unsupported filesystems fail closed. A failure between bundle and manifest can leave a validated orphan bundle, which an identical retry reuses before publishing its manifest. Unpublished temporary files and original evidence are preserved, not silently deleted. Invalid preflight requests do not create a store; I/O failures after apply begins may leave private staging files.

Identical retries return the same result only after validating the archive and manifest bytes and all bindings. A changed body using the same request UUID conflicts. Different transport UUIDs may point to one bundle without duplicating its immutable bytes. An existing corrupt archive or manifest is preserved and blocks import acknowledgements and global request-namespace validation. Current measured replay totals still exclude every imported archive. No implicit merge or repair happens.

## Limits and qualification

Both the request and CLI input are bounded at 1 MiB. A canonical bundle is at most `1 MiB − 4096 bytes`, reserving envelope headroom; export fails with `limit_exceeded` rather than emitting a bundle that cannot fit a compact import request. At most 2048 runs and 2048 observations fit one bundle. There are at most 4096 import-directory entries and 64 MiB of stored import data; pending publication names count toward directory capacity. Apply checks prospective bounds before acknowledgement. No pagination or streaming transfer is implemented, and oversize source ledgers must not be described as exportable.

Exit 2 means invalid schema/version/bounds; 3 means missing local export evidence or unsupported evidence precondition; 4 means identity/digest/idempotency or path conflict; 5 means corruption, filesystem I/O or bounded lock timeout. Errors carry sanitized diagnostics.

The implementation is an inert portable archive workflow, not restore/replay into the local authoritative ledger, qualified reconciliation, provider attestation, a disposable projection or feature 050 completion. Pagination/large-ledger transfer, actual power-loss/platform qualification and the target workload remain open. The independent full-feature FR-009/US3/convergence gates must not be closed from this bounded slice alone.
