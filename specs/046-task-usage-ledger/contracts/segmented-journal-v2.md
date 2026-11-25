# PROPOSED: segmented journal storage v2

Status: **PROPOSED; not implemented or qualified**. Design against source `89c49c7`. Every normative statement below describes proposed behavior. Storage v2 is distinct from CLI protocol v1, observation schema v1 and receipt schema v1. This proposal changes storage capacity and implementation, not counter semantics, task acceptance or assurance. Parent review is required before implementation.

## Observed constraints and correction decisions

Source paths below are relative to the repository root.

| Observed source | Contradiction or risk | Proposed correction |
|---|---|---|
| `rust/crates/ai-gent-core/src/usage/journal.rs` and `observations.rs` | Separate 64 MiB logs; whole-file byte vectors and decoded objects. The measured 30k observation journal is 61,485,576 bytes. | Segmented streams and record-at-a-time decoding; do not merely increase the limit or concatenate segments into a vector. |
| `observations.rs::ingest`, `replay_value`; `contributions.rs` | Replay retains full requests, summaries retain ID arrays, retries reconstruct historical responses. Segmentation alone does not bound this state. | Disk-backed exact indexes and ordered output spools; keep compatibility JSON shapes and original retry results. |
| `receipts.rs::verify_prefixes` | Legacy digests bind literal logical prefixes including newlines. | Stream original bytes in global sequence order. Standalone segment digests cannot reconstruct SHA256 of their concatenation. |
| `journal.rs::lock` | Shared lock acquisition has a five-second deadline and retryable `lock_timeout`. Full replay under the lock can make contention fail. | Keep deadline; measure contention explicitly. Do not label retries or a faster warm cache as proof of the 16-writer target. |
| `transfer.rs::check_request_id` | Imports participate in the global UUID namespace and are validated by archive scans. | One exact namespaced operation index rebuilt from all authoritative stores, including imports and recovery. |
| `journal.rs::load` | An old binary ignores a new manifest and would append its legacy file. | Explicit migration fence at the old registration path; never rely on a new marker the old binary does not read. |
| `data-model.md` vs `identity.rs` | The model describes draining/closed and accepted lifecycle stages beyond implemented run states. | Preserve implemented states; this storage work does not implement future lifecycle/050 acceptance. |

## PROPOSED layout and authority

Under `.ai-gent/usage/`, preserve `journal/registrations.lock` as the single interprocess lock. Existing `receipts/`, recovery quarantine and `imports/` remain authoritative and retain existing schema/limits. Introduce:

```text
journal/registrations.jsonl             # migration fence after opt-in migration
journal/observations.jsonl              # preserved legacy source until migration commit
journal/v2/HEAD.json                    # atomic replace, canonical commit pointer
journal/v2/manifests/<sha256>.json       # immutable canonical manifest generations
journal/v2/registrations/<uuid>.jsonl    # closed objects or active tail
journal/v2/observations/<uuid>.jsonl
journal/v2/migration/<uuid>.json         # pinned intent and preserved legacy references
journal/v2/recovery/<uuid>.json          # explicit repair intent/result
journal/v2/cache/                       # disposable indexes and output spools
```

A manifest has exactly these top-level fields: `storage_schema_version:2`, `kind:"segmented_usage_journal"`, `repository_id`, `generation` (u64, checked increment), `predecessor_manifest_digest` (null only initially), `streams`, `migration_id` (nullable UUID). `streams` has `registrations` and `observations`; each has `sequence`, `closed` ordered descriptors, and `active`. A closed descriptor has `object_id` UUID, `first_sequence`, `last_sequence`, `byte_length`, `raw_sha256`, `frame_count`. Active has `object_id`, `first_sequence`, `committed_sequence`, `committed_bytes`, `committed_raw_sha256`. Empty active has zero bytes, sequence equal to the stream's preceding sequence and SHA256 of empty bytes. File names are derived from UUIDs, never supplied paths. Sequences are global within each stream, starting at one, contiguous across files; no frame crosses a file boundary. Existing frame bytes/schema stay unchanged.

`HEAD.json` has `storage_schema_version:2`, `generation`, `manifest_digest`. It names exactly one immutable manifest. Unknown versions/fields, inconsistent generation/predecessor, missing objects, duplicate object IDs, gaps, overlapping ranges, escaping paths, symlinks and special files fail closed. Closed descriptors bind exact full file bytes. Active descriptors bind a committed prefix. Extra physical tail bytes are not silently ignored for successful commands; they require the explicit recovery path below. Head replacement is a new platform-qualified same-filesystem atomic-replace primitive, **not** current `durable::publish`, whose hard-link behavior is no-clobber.

A manifest chain is storage history, not a new source of provider authentication. Retain every manifest initially; enforce its explicit history budget rather than adding automatic garbage collection. Closed files are never modified by application code. Unix permissions remain 0700 directories/0600 files; this does not prevent owner modification or establish adversarial immutability.

## PROPOSED hard bounds

All limits are checked before allocation, path traversal or mutation; arithmetic uses checked operations. These are new storage-v2 capacity limits requiring review, not claims about current defaults.

| Resource | Proposed bound and consequence |
|---|---|
| New encoded frame | 1 MiB including newline, preserving existing writer limit; reject larger new writes before decode. Existing v1 decoders count the line excluding newline; preserve that one-byte legacy acceptance boundary when reading/migrating existing evidence, pending explicit compatibility review. |
| Ordinary segment | 8 MiB including all frames; rotate before a frame would exceed it. A 1 MiB frame fits. |
| Migrated legacy object | Up to existing 64 MiB per stream; initially allowed as one exceptional closed object to preserve exact bytes without large allocation. |
| Stream | 256 MiB, 131,072 frames, at most 128 closed objects plus one active; whichever is first. |
| Both source streams | 512 MiB combined. 100k observations at about 2,050 bytes consume about 195.5 MiB and approximately 25 ordinary segments; 100k maximum-size frames deliberately do not fit. |
| Manifest/HEAD | 128 KiB manifest, 4 KiB HEAD; at most 4,096 retained manifests / 64 MiB total manifest bytes. Capacity failure is explicit; no automatic pruning. |
| Ancillary evidence | Existing receipt, archive and recovery bounds remain independently enforced; do not accidentally replace their 64 MiB checks with the new stream bound. |
| Directory/path | Only fixed layout components, canonical UUID/hex names; relative generated path <=240 UTF-8 bytes, full path <=4,096 bytes plus platform restrictions. Enumeration <=512 stream entries including pending/orphan files. |
| Disposable work | <=1 GiB combined cache/spool scratch and <=64 open file descriptors; fail explicitly on exhaustion. No unbounded mmap or collections. |
| Working memory | Target internal budget <=128 MiB, leaving headroom within process RSS <=256 MiB; bounded I/O buffer 64 KiB and at most one decoded 1 MiB frame per decoder. This is a budget to prove by measurement, not a proof. |

Manifest-history exhaustion may occur before source-byte exhaustion for long-running ledgers. That is an explicit limitation; review a separate crash-safe history-compaction design before advertising indefinite operation. The 100k fixture must batch initial fixture construction or otherwise include enough measured manifest generations to expose this restriction: **100k independently committed writes cannot fit 4,096 generations**. Options requiring review are raising a measured manifest-history budget, a bounded two-slot HEAD plus retained checkpoint policy, or immutable commit records compacted with a proved retention rule. Do not ship the current illustrative history limit as a claim that 100k ingestion succeeds. This unresolved decision blocks write-capacity qualification, not the streaming reader prototype.

## PROPOSED append, rotation and ACK transaction

1. Acquire the existing lock within five seconds. Resolve migration state/HEAD, validate authoritative sources and namespace at the selected generation; no cache-only integrity shortcut. Build the canonical request digest and exact original-response locator. Identical UUID/body retries revalidate bound evidence and return the original result after required durability barriers; changed body or another operation conflicts.
2. Validate business rules and all byte/count budgets before appending. Assign the next global stream sequence. If the active file would exceed 8 MiB, seal its currently committed bytes as a closed descriptor and create a fresh private UUID tail. Do not rename or rewrite the old bytes; the new manifest changes their role. Empty tails need not become closed segments.
3. Append one complete frame to the selected tail; sync the file. Flush any newly created file's parent directory. A failure gives no success response and leaves evidence for recovery. The previous HEAD remains authoritative until commit.
4. Build the next manifest binding both streams, including the unchanged stream. Publish it using no-clobber immutable publication, file flush and directory flush. Write a private HEAD temporary file, flush it, atomically replace HEAD, then sync its directory. Reopen/validate the committed pointer when replacement outcome is uncertain.
5. Publish disposable index updates only after evidence commit, or discard them. ACK only after source file, manifest and HEAD directory durability. Receipt creation additionally verifies and flushes bound source evidence before existing immutable receipt publication. Receipt retries still verify current evidence before ACK. Cache failure after a committed write must not invent rollback; retry the same UUID to discover its durable result.

No multi-operation batching changes public ACK semantics. Internal batched fixture creation is not a substitute for real ingestion measurement. Reader commands initially hold the same lock for coherent validation/output; optimistic unlocked reads and persistent services are separate proposals, not assumed optimizations.

| Failure boundary | Required restart outcome; no false ACK |
|---|---|
| Before tail write | Previous HEAD usable, no new request committed. |
| During tail write | Extra/torn physical suffix reported; explicit recovery preserves original bytes before repair. |
| After file sync, before manifest/HEAD | Valid extra frames may exist; no implicit discard/adoption; pinned recovery decides their valid continuation. |
| After new manifest, before HEAD | Unreferenced manifest/tail remains preserved and bounded; old HEAD authoritative; explicit recovery handles its extra physical suffix. |
| During HEAD replace/directory sync | Validate whichever complete HEAD is visible; unsupported atomicity/durability fails closed. No guaranteed ACK if sync failed. |
| After durable HEAD, before ACK | Request is committed; identical retry returns the original response once, without recounting. |
| During cache publication | Delete/rebuild cache; verify source, never lose acknowledged evidence. |
| Missing/altered closed object | Corruption; no automatic truncation or reconstruction from projection. |
| Receipt publication interrupted | Existing immutable publication/retry guarantees apply; no receipt before evidence commit. |

## PROPOSED bounded-memory replay and indexes

Internal interfaces are pull-based: `open_view(root) -> LockedView`, `frames(stream) -> Iterator<Result<FrameWithRawBoundary>>`, `verify_prefixes(sorted_claims)`, `fold_usage(frame, spill_state)`, `lookup_request(uuid) -> operation/body_digest/response_locator`, and `write_replay_json(writer)`. These are proposed internal APIs, not new CLI flags. Decoders retain raw bytes only until validation/fold completes; no `Vec<IngestRequest>`, whole-log `Vec<u8>`, or `events(records)` clone on v2 paths.

Use a disk-backed exact-key store with bounded page cache (implementation selection must be reviewed): request UUIDs, canonical source dedup keys, run/segment identities, ownership, cumulative cutoffs, delta/terminal membership, ordered contributing/noncontributing IDs and quality facts. Hashes accelerate lookup; compare original canonical keys/body bytes on collisions. Keep source locators rather than full request bodies. Spill registration state too: many large configurations/runs can exceed memory even if observation parsing streams. Preserve all current null floors, terminal/cumulative coverage conflicts, source alias order and model/provider/role distinctions. Incremental aggregation must prove equivalence to the characterized reducer, including historical retry responses, before replacing it.

Protocol-v1 replay JSON retains existing arrays and order by streaming them from spools; output size can exceed RAM. The CLI must write incrementally rather than first construct `serde_json::Value`. Caller buffering is outside CLI RSS but must be documented. Failures during validation occur before successful JSON begins: validate and prepare a bounded disk spool first, then emit. A later stdout I/O failure can still leave partial output; callers must treat a nonzero exit or incomplete JSON as unsuccessful. Prevalidation cannot make stdout delivery atomic. No silent pagination, omitted IDs or new summary-only response. If this cannot meet resource limits, explicitly review a new protocol rather than changing v1.

A cache binds storage schema, reducer version, repository, manifest digest, all ancillary evidence inventory digests, and exact source generations. It is disposable and never authoritative. File length/mtime, unchanged manifest and cached segment hashes **do not prove actual source bytes unchanged**. A qualified command scans actual closed bytes, verifies frame checksums/semantic references and committed tail, and recomputes receipt prefixes. A fast cache-only mode is not proposed. This implies at least O(source bytes) validation per cold CLI invocation; the five-second lock/16-writer target may demand further architecture. A persistent trusted verifier would require a separate explicit threat/consistency decision; do not claim the cache solves this tradeoff.

## PROPOSED receipt, recovery and UUID compatibility

For each stream let `L` be the literal concatenation of its ordered closed bytes and committed active bytes. A receipt v1 sequence N still binds SHA256 of `L` through the Nth newline, including that newline; sequence zero binds empty bytes. Parse/check each frame's global sequence while advancing one SHA state. At sorted receipt cutoffs clone/finalize that state. Neither reserialization, segment digest concatenation nor Merkle roots are equivalent. Preserve legacy receipt bytes, names and predecessor chain exactly. A future receipt major version is outside this slice.

The global request namespace covers registration, segmentation, transition, observation ingestion, snapshots, recovery and imports. Migration/repair operation IDs must not collide either. Exact operation/body identity and historical response reconstruction survive rotation, cache deletion and migration. Import bundles remain isolated and do not contribute to current totals. Rebuild the index from validated authoritative manifests/frames/receipts/recovery/import archives; refuse inconsistent duplicate ownership instead of choosing a winner.

Recovery dry-run writes nothing and pins HEAD digest, generation, both logical streams, affected physical-tail full digest/length, and candidate complete-frame boundaries. Apply under the same lock revalidates the pinned plan, publishes full original damaged tails and repair manifest durably, then repairs only incomplete trailing bytes beyond all bound evidence and commits a new manifest/HEAD. Valid complete uncommitted suffixes may be adopted only after full cross-stream/business/UUID validation and explicit pinned apply; do not discard a complete frame merely because HEAD is older. Never repair interior/closed corruption, remove receipt-bound evidence, renumber frames or lower a committed sequence. A retry resumes the exact plan and cannot truncate subsequent valid appends. Recovery must also enumerate rotation-created orphan tails absent from the authoritative HEAD, pin their full bytes and inventory, and associate them with abandoned manifests or migration intents where possible. Unknown or unassociated objects are preserved and reported; neither adoption nor deletion is allowed without explicit validated, digest-pinned recovery. Recovery cannot inspect only the referenced active tail. Evidence absent because a referenced file was lost is a failure, not an empty stream.

## PROPOSED migration, export and rollback

Migration is explicit opt-in; no implicit migration on read/ingest. Existing v1 stores remain readable/writable with existing limits in the new binary. Dry-run is read-only, takes the existing lock if present, validates all sources/receipts/archives and capacity, and reports exact raw digests, sequence counts, planned storage version/fence and rollback availability. Applying rechecks these digests under the same lock; changing evidence causes conflict.

Preserve legacy raw files as byte-identical private closed objects, sync copies and directories, and publish a durable migration intent containing original path/digests, candidate manifest and phase before changing legacy paths. Stage a small newline-terminated incompatible storage-fence JSON at the old `registrations.jsonl` path and atomically replace that path, then sync its directory. An old binary's strict registration-frame decoder rejects the fence; it cannot ACK appends. Do not use only chmod (owner can write), a marker old code ignores, or a missing legacy path (old code can recreate it). Prove this with an actual old binary, including register/ingest/import/recovery. The v2 binary treats an intent without committed HEAD as migration-in-progress and resumes/rolls back explicitly; it never interprets the fence as missing evidence. Before fence commit, old writes can proceed only after the lock releases; resume therefore must compare pinned legacy digests again, never overwrite newly accepted old writes. Unknown version rejection is mandatory.

After the durable fence, commit the initial manifest and HEAD, sync all ancestors, then ACK. Keep original bytes and migration intent; do not automatically delete old observations. Crash at any step leaves either valid legacy storage, a resumable fenced migration, or committed v2. Exact phase detection, including atomic-replace failure, needs platform tests before this is safe to enable.

Current export remains schema-v1, <=2,048 runs/observations and <=1 MiB minus 4,096-byte wrapper headroom, pseudonymized/imported/unverified/unclosed. A large segmented ledger returns an explicit limit error with no partial bundle; count/size checks must be streaming. No paging, restore promise or automatic archive injection is added. Imports keep current archive limits and UUID conflicts.

Rollback before v2 appends can restore byte-identical legacy paths atomically under the lock using preserved originals, after validation and durable rollback intent. After v2 writes, an explicit flattening rollback is possible only if each logical stream fits the old 64 MiB limits and all current evidence remains representable; stream exact concatenated bytes into staged files, verify every receipt and old-binary compatibility, and publish under the same fence/intent discipline. Multi-file restoration is not atomic: recovery must resolve intent before any new-binary operation, and keep the registration fence until observations and ancillary state are safe. Above old limits, rollback is denied without data deletion; retain v2 and use a compatible binary. Dry-run export is not rollback or full backup.

## PROPOSED qualification boundary

No segmented capability is implemented by this document. Existing snapshots are local evidence snapshots, not accepted-task receipts or provider billing certification. Existing Unix process-kill tests do not prove power-loss safety, disk-full behavior, filesystem portability or macOS/Windows durability. See the implementation plan for staged proof obligations, unresolved design decisions and unchanged reference-runner targets.
