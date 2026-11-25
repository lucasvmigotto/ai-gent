# PROPOSED: segmented journal implementation and proof plan

Status: **PROPOSED; no runtime implementation or qualification**. Prepared against `89c49c7` on `docs/usage-journal-segmentation`. This document and [the proposed storage contract](../contracts/segmented-journal-v2.md) are a review handoff, not feature completion. No new feature IDs, task flags, skill/plugin/hook changes or provider claims are introduced.

## Evidence and current gate

The [indexed replay slice](indexed-replay-slice.md) measured 30k observations / 61,485,576 raw bytes at 9.296 seconds and 232,504 KiB peak RSS, after contribution indexing. This was one debug warm-data sample on a shared eight-CPU host, without receipts/imports. It does not qualify the four-core/eight-GiB reference target. At the same approximate frame size, 100k events need about 195.5 MiB, exceeding the current 64 MiB observation limit. The unchanged source allocations would also threaten the 256 MiB RSS target. Increasing a constant is insufficient.

Two normal full repository checks failed in the unchanged backup executable fixture. Subsequent isolated passing tests do not establish the cause or clear the full gate. The parent subsequently published test-only fixture hardening at `7ea2de2`, with a full uninstrumented repository check passing on 2026-11-25; see [the fixture record](backup-native-fixture.md). The historical spawn cause remains unknown. This proposal is stacked on that locally green descendant. No test disabling, timeout increase, serialization, retry-to-green, check waiver or speculative database-runner fix is part of this plan. Resolve and pass the required full gate before landing runtime work.

Source evidence is in `rust/crates/ai-gent-core/src/usage/{journal,observations,contributions,receipts,durable,recovery,transfer}.rs`; public compatibility expectations are in the existing [contracts](../contracts/cli.md), [data model](../data-model.md) and [QA plan](../qa.md). The data model includes future lifecycle states absent from current code; storage work preserves actual current behavior and cannot claim those stages implemented.

## Decisions required before enabling v2 writes

All options below are **PROPOSED**. A prototype may characterize read paths before these decisions are settled; no production migration/write path should be enabled with unresolved durability/capacity semantics.

1. **Manifest retention:** An immutable manifest on every append plus a 4,096-generation hard cap cannot support 100k independently committed writes. The contract deliberately exposes this contradiction. Prefer reviewing bounded manifest retention: retain current and immediate predecessor plus manifests explicitly pinned by migration/recovery intents; remove only older unpinned metadata after durable HEAD publication, with a bounded orphan sweep under the lock. This is safe only after proving no receipt/source/retry response depends on discarded manifest bytes and crash recovery does not require their full chain. Alternatively use a rigorously specified two-slot commit pointer, or raise an explicitly measured disk/directory budget to at least the real workload. Do not silently raise the illustrative cap or pretend synthetic batched seeding proves write scalability.
2. **Integrity versus contention:** Cold full verification of approximately 200 MiB per independent CLI mutation is incompatible with assuming constant-cost writes. A disposable cache is useful for semantics but cannot detect same-length modified evidence without reading bytes. Keep strict verification initially and measure the limitation. If 16 writers fail, review a persistent single verifier/transaction service or a precisely scoped trust model; neither is implicit in this feature. Do not omit source verification while retaining a claim of current corruption detection.
3. **Disk-backed reducer/index:** Select an existing maintained dependency only after dependency/license/platform review, or a small external-sort implementation with bounded fan-in and exact-key collision comparison. Define transaction boundaries, scratch ownership and decoder limits first. A map of every full observation in memory is not an acceptable substitute.
4. **Migration fence/platform primitive:** Prove atomic replace and directory-flush semantics on each supported filesystem/OS. Current immutable hard-link publication is not a replace primitive. An old binary must reject the registration fence for every mutation surface. Never assume chmod, a new marker or a missing file prevents old writes.
5. **Historical retry outputs:** Current ingestion returns a response derived from the prefix ending at the original request. Define exact source-boundary locators and reconstruct that response through the same reducer; caching only the latest summary changes behavior. Characterize existing JSON byte/value/order expectations before replacing `Value` construction.

## Proposed ordered implementation slices

Each slice stays within existing 046 US3/convergence/QA scope. The labels below are planning labels, not new Spec Kit task IDs. Keep existing task flags unchanged until their complete recorded exit criteria hold. Parent reviews the contract before implementation. Use a normal feature branch with committed evidence that the home agent can fetch; local ignored worktrees alone are not a handoff.

| Order | Proposed change | Required evidence before proceeding |
|---|---|---|
| A | Characterize existing bytes/JSON/errors/UUID namespace and add a bounded streaming frame reader for existing v1 files. Do not switch writers. | Literal legacy frames and receipts replay identically; reject malformed lengths, giant lines, torn tails, gaps, checksum changes and unsafe paths. Tests prove bounded reads, not merely iterator syntax. |
| B | Add internal logical-stream abstraction and v2 manifest/HEAD parser behind internal tests. Resolve retention decision. | Sequence/object/digest/schema invariants, explicit caps, missing references, orphan bounds, ancestor/symlink/FIFO attacks, overflow; exact original-byte concatenation streamed without materialization. |
| C | Introduce disk-backed exact indexes, reducer state and ordered output spool; preserve v1 CLI response. | Differential replay versus current reducer across generated synthetic mixed cases; exact null floors, aliases, cumulative cutoffs, terminal replacements, model/role scopes, original retry responses and output order; cache deletion/corruption cannot change truth. |
| D | Implement append/rotation/manifest/HEAD durable transaction under existing five-second lock; no migration yet. | Every commit fault boundary and actual process-kill boundary, byte/count cap checks before append, duplicate UUID after lost ACK, two contenders, full disk/short write, unsupported filesystem failure; no durability ACK on injected failure. |
| E | Implement pinned explicit migration and resume/rollback fence. | Read-only dry-run writes no file or directory; old-binary mutation denial; source changed since plan conflicts; crash before/after each fence and HEAD step; byte-identical legacy receipt validation and denied rollback above old capacity. |
| F | Adapt snapshots, recovery, transfer and global operation indexes to logical streams. | Cross-store UUID conflicts, immutable receipt retry after corruption, tail adoption/truncation constraints, no closed/interior repair, import isolation, bounded export refusal, archive tamper detection, recovery retry after later appends. |
| G | Run real workload and independent review; document remaining limits honestly. | Deterministic full checks, platform evidence, reference release-build replay/RSS/concurrency data, fault logs and exact binary/source identity. No Verified label without all feature exit criteria. |

Implementation tests should attach to existing core/CLI suites where practical; do not add test-only production environment controls. Failure injection belongs behind `cfg(test)` as current durable/process-crash support does. Tests should attack the invariants, not merely mirror helper implementation.

## Proposed correctness obligations

- **Logical byte preservation:** For each stream, the ordered file sequence plus committed active prefix equals the pre-migration literal byte sequence followed only by valid newly appended frames. Induction covers append and rotation; migration base case compares streamed raw hashes and lengths. Golden receipts at segment boundaries and mid-segment boundaries prove legacy SHA256 behavior. Verify empty/repeated/out-of-order cutoffs and conflicting same-sequence claims.
- **Durable commit:** A successful response implies durable source bytes, selected manifest and HEAD publication; a failed/absent response may still be committed. Enumerate all kill and I/O fault states and demonstrate that retry discovers exactly one authoritative request. File sync alone does not prove directory/name durability.
- **Semantic equivalence:** Differential/property tests compare current and streaming reducer results for bounded generated histories, including invalid histories rejected by both. This is evidence, not a mathematical proof for every input. Independently review cumulative request floors, identity ownership and unknown/null preservation; these are the highest-risk state transitions.
- **Namespace uniqueness:** Rebuild from all authoritative operation stores after deleting indexes. Equal UUID/equal operation/body reproduces original result; any cross-operation reuse or changed body conflicts. Include migration and repair IDs; event/source identities remain distinct from transport request IDs.
- **Cache nonauthority:** Replace cache contents, omit cache, preserve file size/mtime while changing source bytes, alter closed files and archive metadata. Qualified commands must detect evidence corruption and never ACK an old receipt based only on cache. Cache version mismatch rebuilds, not evidence deletion.
- **Resource bound:** Decoder buffers, page caches, external sort fan-in, output spools and directory enumeration have separately enforced byte/count caps. Exercise large valid configurations and many runs, not just many small observations. Measure RSS of the actual CLI including validation and JSON writing.
- **Recovery monotonicity:** No committed sequence decreases, acknowledged/receipt-bound evidence is never discarded, and later valid appends survive old repair retries. Recovery preserving originals is not proof that missing evidence can be reconstructed.

## Proposed load and contention qualification

The unchanged target is replay <=10 seconds and process peak RSS <=256 MiB for 100k synthetic observations, with 16 concurrent writers, on a declared four-core/eight-GiB reference runner. Use a pinned release binary, record commit/binary SHA256, OS/kernel/filesystem, CPU/memory allocation, toolchain, frame-size distribution, source bytes, receipt/archive counts and workload seed. Publish commands and portable results, not only `/tmp` logs. Do not extrapolate 30k debug timing.

Separate these measurements:

- Cold verified replay with disposable projections absent, then verified warm replay; include source validation, receipt verification and output generation in both. Document cache eviction method and its limitations.
- Independently committed ingestion through real CLI, not direct fixture generation, with 16 separate processes and unique immutable request UUIDs. Assert final sequences, exact totals, no duplicated authoritative contributions and all accepted requests replayable.
- Record per-attempt lock acquisition latency, five-second timeouts, bounded client retries, end-to-end completion, errors and retries per UUID. A timeout remains a visible failed attempt. Retry only retryable failures with the same UUID/body and bounded attempts; jitter/backoff is a harness policy, not a new CLI flag. Report first-attempt and eventual outcomes separately. No fairness/starvation guarantee is assumed.
- Replay while writers contend, and writers while verified replay holds the lock. Report whether the five-second deadline blocks the target; do not remove competing readers to advertise a general concurrency claim.
- Empty/sparse and many receipt boundaries, imported archives near existing limits, cross-model/provider/role segments, duplicate sources, cumulative snapshots and null fields. The earlier receipt-free fixture alone misses important lock work.
- Maximum allowed frame/metadata and near-cap cases separately from the representative approximately 200 MiB fixture. A 100k-by-1-MiB workload exceeds proposed storage caps and must return an honest limit error, not be called a successful scale test.

The QA text does not define a writer-throughput or tolerated-timeout SLO. Record the observed distributions and ask the reviewer to resolve that acceptance ambiguity; do not invent a favorable threshold. At minimum, an eventual-success claim requires all planned requests either durably committed once or explicitly failed and reported, with no lost ACK evidence. Replay target timing is still unchanged.

## Proposed compatibility and portability matrix

| Case | Required outcome |
|---|---|
| New binary, untouched v1 store | Existing operations and limits remain, no implicit migration. |
| New binary, valid v2 | Global sequences and existing protocol-v1 semantic responses preserved; large replay streamed. |
| Old binary, fenced v2 | Explicit failure without evidence mutation/ACK for register, segment, transition, ingest, snapshot, recover and import. Test the actual previous binary. |
| New binary, unknown version or interrupted migration | Fail closed or explicit pinned resume; never fabricate empty state. |
| v2 export larger than existing bundle limits | Typed limit error, no partial output/bundle and no totals changes. |
| Rollback after growth beyond v1 bounds | Denied with evidence retained; compatible binary required. |
| Linux/macOS/Windows | Independent actual filesystem/launch/lock/replace evidence before advertising durability. Unsupported barriers fail explicitly. |

## Handoff and nonclaims

Only the two proposed Markdown artifacts are produced by this design workstream. No Rust edits, builds, commits, pushes, task completion flags or protected instruction changes are included. Independent review approved publication as planning artifacts; the parent added explicit rotation-orphan recovery and partial stdout failure semantics. The fixture descendant has passed its full gate; the final proposal publication records its own full check separately. The office/home handoff should link the final published branch/commit and remaining decisions.

This plan does not certify provider usage, billing, task acceptance, live Claude/Codex adapter behavior, all Code Agents, power-loss recovery or unmeasured performance. It retains capability-based Tier 1/2/3 accounting semantics while proposing a storage layer common to them. Correctness/durability/resource evidence must be recorded independently; no single passing benchmark substitutes for the others.


## Reviewed first implementation slice: bounded v1 frame reading

Read-only source review identifies a private shared frame-reader module as the lowest-risk insertion point. Switch `journal.rs::Store::load` and `observations.rs::read` to checked file handles with fixed I/O buffers and a reusable, explicitly capped line buffer. Return literal frame bytes, newline-inclusive byte boundaries and checked ordinals. Keep typed frame parsing, semantic validation and the slice-based `decode` wrappers in their current modules for recovery. Preserve missing-file handling, regular-file/path checks and existing lock callers.

This slice removes raw whole-file buffering only. Keep `Ledger`, decoded request vectors, duplicate sets, `events()` clones and the reducer unchanged. Receipts and recovery still independently hold raw vectors. Do not claim bounded total replay RSS from this first change.

Observed compatibility constraints:

- Existing decoders cap line bytes **excluding** newline, while writers cap encoded frames **including** newline. Preserve legacy decoder acceptance; do not silently tighten it to the writer boundary.
- File readers reject total size above 64 MiB before decoding; decoders reject missing final newline before parsing any frame. Preserve error precedence using a bounded preflight scan, rewind of the same checked file handle, then bounded decoding. Slice wrappers retain upfront final-newline checks without inventing a new whole-log limit.
- Verify checksums over canonical payloads, preserving acceptance of noncanonical frame JSON whitespace/key order. Keep registration validate/apply, observation normalization/binding, all UUID conflict checks and final whole-history summary validation.
- Recovery passes complete prefixes deliberately; the general reader must never trim torn tails automatically. Bound reads and checked arithmetic before buffer growth; merely calling `read_until` is insufficient.

Required tests compare current and candidate decoding on valid mixed histories and noncanonical frames; split tokens, UTF-8 and newline across chunks; cover empty/blank/repeated/torn frames, unknown versions/fields, sequence errors and checksum/length errors. Assert both exact line-limit boundaries, malformed-first-frame plus oversized/torn-final-file error precedence, and bounded allocation/read consumption from a generated unbounded newline-free reader. Inject sanitized read failures in both passes. Run existing path/symlink/FIFO, snapshot, import UUID, historical retry and recovery-after-later-append regressions. This is characterization and implementation guidance, not completed code or test evidence.


## Publication validation — 2026-11-25

Independent review approved these proposed artifacts with no runtime enablement. The parent incorporated rotation-orphan inventory, partial stdout failure semantics and the reviewed v1 decoder compatibility brief. Plan structural validation passed (9 features, 27 stories, 81 requirements, 198 tasks, 12 complete), as did `git diff --check`. Full uninstrumented `scripts/check.sh --quiet` passed on this descendant, including installer/update checks and pinned Rust gates; local log `/tmp/ai-gent-usage-segmentation-plan-full-check.log`. Together with the fixture commit's earlier successful full run, this is two successive full local passes of the revised fixture. Historical failure cause remains unknown; no remote CI, merge, provider certification or scale qualification is claimed.
