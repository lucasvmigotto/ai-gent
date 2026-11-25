# Ledger scale baseline and correction plan — 2026-11-18

Status: diagnostic measurement, **not qualified**. The target remains 100k observations, 16 independent writers, replay ≤10 s and peak RSS ≤256 MiB on a recorded 4-core/8-GiB reference runner. None of those combined qualification claims follows from this probe.

## Observed baseline

A read-only scout used an existing debug binary, version 2.28.0, Linux x86_64, SHA-256 `99b541380a1f8918acc365e4634df0cf6ec12ccc8a7de76566a1bebf8d67650f`, unchanged before/after the runs. Source baseline associated with the workspace was `e0d2d9164a63e74c74021299d1dabb3a9a51c842`. This was an 8-CPU shared host, cpuset 0–7, CPU/memory cgroup limits unset, MemTotal 10,186,392 KiB. It was not the reference runner or a pinned release build.

Synthetic checksummed observations were written directly into a temporary journal to measure **replay**, not ingestion throughput: one run/segment, distinct terminal requests, no receipts. Every invocation asserted the sequence count and total of 1,200 tokens/event. Peak RSS measures the CLI child on Linux; wall time includes the measurement wrapper and JSON output. Each size was measured once, so timings are diagnostic, not distributions. Temporary journals were generated immediately before replay without evicting filesystem caches; this is a warm-data diagnostic, not a cold-disk or ingestion/fsync-throughput measurement.

| Events | Journal bytes | Wall seconds | Peak RSS KiB |
|---:|---:|---:|---:|
| 1,000 | 2,042,572 | 0.331 | 21,756 |
| 5,000 | 10,230,572 | 1.710 | 51,364 |
| 10,000 | 20,465,576 | 3.688 | 87,312 |
| 30,000 | 61,485,576 | 16.609 | 232,396 |

At the 30k frame size (~2,050 bytes), 100k projects to ~195 MiB, exceeding the current **64 MiB total observation-log cap**. Roughly 32.7k such frames fit. The projection is arithmetic, not a measured 100k run. Merely raising the cap would expose existing memory and lock-duration problems.

The portable reproduction is `probes/measure-replay.py`:

```sh
python3 specs/046-task-usage-ledger/evidence/probes/measure-replay.py \
  --binary /absolute/path/to/ai-gent --counts 1000 5000 10000 30000
```

It uses only synthetic sources and deletes its own temporary repository on exit. Counts are bounded to 30k. The probe pins a private copy and reports its binary hash so concurrent builds or later source changes are not silently compared as the same baseline. No paid model/provider call occurs.

## Code-evidenced blockers

- `usage/observations.rs` and `usage/journal.rs`: 64 MiB complete-log bounds and whole-log allocation/decoding.
- `usage/contributions.rs`: repeated vector membership for noncontributing IDs and repeated cross-cutoff group scans; quadratic worst-case work.
- `usage/observations.rs`: repeated summarize/normalization passes and clones during decode/response/replay; full history is processed under the five-second repository lock.
- `usage/receipts.rs`: separate prefix hashing for each receipt amplifies work with receipt count.
- The transfer continuation adds `usage/transfer.rs` archive validation to global request-ID guards: mutations may decode/validate up to 64 MiB of archived bundles under the same lock. The measured baseline predates these guards and does not measure this amplification. Future verified indexes must cover archive manifests while preserving corruption and request-conflict detection.
- A replay taking 16.6 seconds can outlive a competing writer's five-second lock wait. Identical-request concurrency tests do not qualify sustained independent writers.

## Incremental implementation plan

1. Replace membership/rescan loops with indexed sets while preserving deterministic output order. Avoid cloning full observations and compute normalized projections once. Keep existing semantic, unknown/null, deduplication and out-of-order regressions.
2. Verify all receipt-bound prefixes in a single streaming SHA-256 traversal, cloning hash state at required newline boundaries. Preserve published receipt bytes and the meaning of their digests.
3. Design versioned segmented journals with a durable atomic manifest. Keep legacy frames unchanged as the initial segment; old receipt digests must still verify against identical logical prefixes. Recovery may isolate only an incomplete active tail; missing/interior segments must fail closed.
4. Add disposable digest-bound projections and bounded append batches. Rebuild after a mismatch; a cache cannot substitute for source-integrity checks. Measure memory before selecting limits.
5. Qualify a pinned release build on the declared runner: mixed cumulative/delta/unknown/shared/model-switch evidence, multiple receipt counts, 16 distinct writers, retries and faults. Record cold/warm replay, RSS, lock timeouts and throughput separately. A throughput optimization must not waive acknowledged-write or corruption guarantees.

Segmentation/projections and real crash/load qualification remain open. This document does not close T008/T018/T020–T022 or qualify a host/provider cell.


## Indexed replay continuation — 2026-11-24

The first incremental performance slice is recorded in `indexed-replay-slice.md`: contribution membership/cutoff indexing and normalized quality reuse per summary, plus single-pass raw-prefix hashing for receipts. The matched 30k debug sample improved from 17.180 to 9.296 seconds with similar memory. Whole-history cloning/summary work across operations, archive scans, journal segmentation and verified projections remain open. Existing measurement limits and the unqualified 100k/16-writer target still apply. The Linux probe now supports a bounded `--timeout-seconds` and cleans its private process group on the tested timeout/SIGINT paths; it does not promise cleanup after an uncatchable parent kill.
