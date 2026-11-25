# Local recovery and evidence snapshots — 046 US3 slice

These opt-in operations extend protocol v1 without accepting tasks. They do not emit the feature 050 completion receipt. All results retain local unverified assurance and unclosed coverage. No runtime host automatically invokes them yet.

## Snapshot

`ai-gent usage snapshot --root <project> --input <file|-> --json`

```json
{
  "schema_version": 1,
  "request_id": "90000000-0000-4000-8000-000000000001",
  "repository_id": "20000000-0000-4000-8000-000000000001",
  "operation": "snapshot",
  "payload": {
    "expected_registration_sequence": 1,
    "expected_usage_sequence": 0,
    "predecessor_digest": null
  }
}
```

Both sequences must match the current locked ledger. The predecessor must be the current snapshot head, null only for the first snapshot. Request IDs share the registration/ingestion/recovery namespace. Identical retries return the original snapshot even after later publications; changed bodies conflict. Source journals are validated and flushed before publication. A previous snapshot's bound journal prefixes must still be available and byte-identical; rollback or corruption cannot be hidden by a later snapshot.

The result contains `receipt_digest` and `receipt`. The latter follows `ledger-snapshot.schema.json`: kind `usage_snapshot`, repository/request identities, request digest, independent registration/usage sequences and raw-log digests, predecessor digest, sanitized numerical summary, `assurance: local_unverified`, and `coverage: unclosed`. Canonical sorted-key JSON bytes are hashed with SHA-256; the digest is outside those bytes. No prompt, raw transcript, machine path or host session appears in the snapshot. Raw evidence remains private and is required to independently reconstruct its arithmetic; hashes alone do not restore it.

Publication writes a private temporary file in `.ai-gent/usage/receipts`, flushes it, then atomically creates `<64 lowercase hex>.json` through a no-clobber hard link on the same filesystem. The directory is synced before acknowledgement, then the temporary name is removed and the directory synced again. Existing names must contain exactly the expected bytes; they are never overwritten. A failed or interrupted publication leaves either no final name or a complete final object that an identical retry re-flushes. Unpublished `.pending-*` files are retained for inspection and never treated as receipts. Filesystems without hard-link support fail closed. Unix file and directory durability is exercised locally; macOS/Windows/filesystem cells require their own qualification.

The head is derived from a validated single predecessor chain under the repository lock; no independently updated head pointer can become stale. At most 4096 receipts and 8192 directory entries are read, with 1 MiB per receipt. This bounded local implementation is not target-scale qualification. Replay exposes `receipt_head`, and detects corrupt receipts or missing/mismatched bound evidence. Existing receipt bytes remain immutable.

## Explicit torn-tail recovery

`ai-gent usage recover --root <project> --dry-run --json` creates no files. Under the existing five-second repository lock it reads bounded journals and validates both complete newline-terminated prefixes against each other. It refuses corrupt complete frames, invalid cross-log references and any recovery that would remove evidence already bound by a published snapshot. A surviving complete registration is required; an interrupted first registration remains a manual inspection case.

The dry run returns the registration and usage plans (original/prefix SHA-256 digests, byte counts, tail byte count and original existence), surviving sequences and an explicit `request` envelope. Review that output and save the request, then apply:

`ai-gent usage recover --root <project> --input <request.json> --json`

The request is schema v1, operation `recover`, nonnil request/repository UUIDs and payload `{ "registration_digest": "sha256:…", "usage_digest": "sha256:…" }`. Digests pin the exact bytes observed in the dry run. Changed evidence requires a new plan. There is no implicit repair during replay or ordinary ingestion.

Before truncating either log, apply durably publishes every damaged original under `.ai-gent/usage/quarantine/<original hash>.jsonl` and a canonical `recovery-<request UUID>.json` manifest. The manifest binds both plans and the request. Only incomplete trailing bytes are removed with `set_len`, followed by file/directory sync. If failure occurs between logs or after truncation, retry verifies the durable copies and manifest, accepts either the original or already-recovered prefix, and resumes. A completed recovery can be retried after later valid appends without truncating new records. Failed operations never return success. Quarantined bytes are not deleted automatically and remain private operational evidence.

Exit 2 covers malformed/unsupported requests and bounds; 3 covers missing registered evidence; 4 covers conflicting IDs, stale snapshot state or changed recovery evidence; 5 covers corruption, filesystem failure and lock timeout. Lock timeouts are explicitly retryable. Unknown fields and major versions are refused.

## Qualification limits

Synthetic test-only fault checkpoints exercise append write/partial-write/flush/directory-sync and snapshot write/flush/link/directory-sync boundaries, plus interrupted recovery before/after truncation. They cannot be selected by production callers. These tests prove error handling and resumable states; they do not establish actual power-loss behavior, remote filesystem guarantees, independent source authenticity or completeness. Process-kill/power-loss qualification and the 100k-event/16-writer performance target remain open. Bulk versioned import with dry-run validation and disposable projections are not implemented by this slice; FR-009/US3 and final QA remain incomplete.
