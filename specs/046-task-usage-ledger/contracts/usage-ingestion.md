# Implemented usage ingestion contract — 046 US2

`ai-gent usage ingest --root <repository> --input <file|-> --json` accepts one UTF-8 JSON request, maximum 1 MiB. The envelope has `schema_version: 1`, UUID `request_id`, UUID `repository_id`, `operation: "ingest"` and `payload` matching `usage-observation.schema.json`. See `rust/crates/ai-gent/tests/agent_046_usage.rs` for executable examples. This implementation is opt-in; no host automatically collects tokens yet.

## Binding and trust

The attempt, run, parent, segment, phase and configuration must match prior registration. A known registered host session must match; an initially unknown session may be discovered, but authoritative observations cannot change session within a run. Requested models do not substitute for unknown effective models. Model changes require a new registered segment. Identity claims remain reported/unknown; `verified` and `provider_reconciled` are rejected until qualified collectors supply authorized evidence.

Registration optionally accepts `execution_scope` with `kind` and `beneficiaries` (TaskKeys). Omitted scope preserves v1 exclusive behavior. Exclusive lists only the anchor task. Shared requires at least two distinct, declared beneficiaries in the anchor repository, including the anchor. Unattributed requires no beneficiaries. Ingest must match that preregistered scope: exclusive observations name the task; shared/unattributed observations have `task: null`. Shared parent runs may register children for their declared beneficiaries. Shared/unattributed usage is retained separately and never assigned exclusively to the anchor.

This is local validation and corruption detection, not protected process authorization or provider certification. A same-user caller can supply invented raw counters. Digests identify caller-supplied evidence; this slice does not fetch or independently authenticate it. Replay always reports local unverified assurance and unclosed coverage. No acceptance receipt is emitted.

## Arithmetic and quality

All raw/canonical counts are nonnegative integers or null, bounded by 2^53−1; at most 64 raw keys. Missing is unknown, including missing cache fields. Canonical fields must exactly match recomputed normalization.

- `inclusive-input-v1`: input already includes cached/creation subsets; output already includes reasoning. Total is input + output only.
- `anthropic-disjoint-input-v1`: total input is uncached input + cache read + cache creation only when all three are known. Output includes reasoning.
- Unknown semantics preserve bounded raw fields and require null canonical counts. No guessed conversion.

Subsets, combined cache subsets, explicit totals and arithmetic bounds are validated. Unknown parent totals cannot hide overflowing known subsets. Exact aggregates remain null when contributions are incomplete; known lower bounds are separate. Declared source quality, normalized quality and counter quality are separately reported. Imported counters never become independently attested.

## Exactly-once local contributions

Transport request IDs are unique across registration and ingestion journals. Identical canonical retries return the original result; conflicting reuse fails. Source event identity includes adapter/version/namespace/session/request/event/scope; repeated copies under new transport IDs cannot increase usage. Source request ownership cannot move between runs, segments or adapters. Adapters must provide stable source sequences and namespaces unique to the actual source stream.

Distinct request deltas sum. A terminal turn total replaces that request's earlier deltas and cannot regress known counts. A null terminal field cannot replace an established positive delta bound; ambiguous replacements fail rather than discard known usage. A session cumulative total replaces earlier per-request coverage; later deltas can contribute after its cutoff. Ambiguous overlaps, counter resets (including across null gaps), conflicting terminal totals and reused sequences are refused. Diagnostic `contribution: observation` entries are retained without charging. Authoritative selection is derived; direct `superseded`, nonempty `supersedes` and nonempty `extensions` are reserved and rejected in this slice.

## Persistence and errors

A separate checksummed `.ai-gent/usage/journal/observations.jsonl` shares the registration lock. Both logs are validated before mutations; append flushes and directory sync precede success. Each log is bounded at 64 MiB and each framed record at 1 MiB (framing can make a maximum-size input too large). Private ignore/permissions follow the registration contract. Historical registration bytes remain readable without rewriting.

`usage replay --root <repository> --json` returns registrations plus `usage_sequence` and a recomputed `usage` summary, including contributing/noncontributing event IDs and separate exclusive/shared/unattributed counters. No persistent projection or receipt publication is implemented. Corrupt/torn logs refuse reads/mutations and preserve bytes. Crash fault injection and target-scale performance qualification remain US3 work.

Exit 2: malformed/version/counter/limit errors; 3: scope, coverage or unsupported assurance precondition; 4: contradictory identity or idempotency conflict; 5: I/O, corruption or lock timeout. JSON errors follow the implemented registration envelope. No prompt payloads or arbitrary extension text are accepted.
