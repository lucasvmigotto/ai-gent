# Indexed replay and receipt-prefix verification — 2026-11-24

Status: bounded performance correction on `perf/usage-ledger-indexes`, based on published transfer commit `d924de0`. Journal/receipt bytes, protocol v1, source semantics, counters and assurance labels are unchanged. This does not qualify the reference 100k-event/16-writer workload.

## Implemented changes

- Contribution membership uses a set while preserving the original ordered ID vectors. Normalized quality dimensions are computed once per observation per summary and cached without retaining another full counter object. Source keys are computed once per unique-ID candidate.
- The redundant quadratic terminal/cumulative cutoff scan is removed: the existing cumulative-floor validator already tracks every covered request across all snapshot cutoffs. The validator and its rejection behavior remain in force.
- Receipt verification sorts/coalesces requested raw prefixes, checks sequence zero against the empty SHA-256 digest, and incrementally hashes each journal's covered bytes once. Cloned hash states verify exact newline boundaries. Conflicting claims, missing boundaries and altered bytes still fail. Existing receipt bytes are never rewritten.
- The Linux diagnostic now has an explicit bounded timeout and terminates/reaps its private process group on timeout or interruption, rather than leaving its detached wrapper/CLI running.

This is incremental hashing over the existing in-memory journal slices, not a streaming journal decoder. Whole-log allocations/clones, repeated summary passes across operations and archive scans under the ledger lock remain future work.

## Compatibility checks

Two added real-CLI cases cover stable contributing/noncontributing order through duplicate source aliases and terminal requests crossing a cumulative cutoff. Imported normalized quality remains distinct from missing counter quality. These characterization cases passed before optimization.

Three receipt golden tests passed against both old and new verifiers: literal-byte hashes/newline boundaries, zero/repeated/out-of-order claims, conflicting claims, corruption, missing prefixes and unbound trailing bytes. A new real-CLI fixture publishes five immutable snapshots with interleaved registration/usage growth and repeated cutoffs, retries the oldest object and refuses lost bound evidence. Existing usage, recovery and transfer suites remain required.

The probe timeout path was forced with a 0.001-second limit and returned explicit exit 124. The first actual SIGINT test exposed a startup race between child creation and ownership by the cleanup guard. The corrected probe defers SIGINT through construction and restores/replays it inside the guard. Three repeated startup-interruption checks then confirmed cleanup and parent return within two seconds. Both tests use only owned synthetic processes/data.

## Matched replay diagnostic

One before/after sample per size was run after agent builds finished on the same shared 8-CPU Linux host, using pinned debug binaries and identical synthetic fixture generation. Journals were freshly generated without evicting caches. This is replay-only, warm-data measurement: one run, distinct terminal requests, **no receipts or imported archives**. Counts and 1,200 tokens/event totals were asserted. Memory is Linux child peak RSS. The timing wrapper and JSON output are included.

- Baseline binary SHA-256: `b443d5eecc9d5bd793d499908708a874945240d8dd9ba43c44b2824093131229`.
- Candidate binary SHA-256: `59b5621054302395483938cdbb64106ac1fb756d81ac15de666e5db01384a40d`.
- Both hashes remained unchanged throughout their runs. Candidate source is this performance diff; the final commit is reported by the producing session without a self-referential hash here.

| Events | Identical journal bytes | Before seconds | After seconds | Before RSS KiB | After RSS KiB |
|---:|---:|---:|---:|---:|---:|
| 1,000 | 2,042,572 | 0.354 | 0.306 | 22,732 | 21,856 |
| 10,000 | 20,465,576 | 3.882 | 3.245 | 87,932 | 87,040 |
| 30,000 | 61,485,576 | 17.180 | 9.296 | 232,936 | 232,504 |

The 30k sample used about 46% less wall time with similar memory. This fixture demonstrates contribution-path improvement; it does not empirically measure receipt-prefix gains because it contains no receipts. Receipt hashing has separate compatibility coverage and the reduced traversal is evident in its single-pass implementation.

An initial baseline attempt during concurrent builds completed 1k/10k samples but exceeded the old 45-second limit at 30k. It is not treated as a completed measurement or a speedup denominator. The table uses the subsequently repeated, matched diagnostic with a 90-second timeout for both binaries. No timing is a test assertion or a reference-runner qualification.

Reproduce with `probes/measure-replay.py --binary <pinned binary> --counts 1000 10000 30000 --timeout-seconds 90`. Local producing-session result files were `/tmp/usage-performance-before-idle.jsonl` and `/tmp/usage-performance-after-idle.jsonl`; the committed probe/table are the portable handoff.

## Remaining qualification

The 64 MiB journal cap still cannot fit 100k observations at this frame size. Segmentation with legacy-prefix compatibility, bounded-memory decoding, disposable verified projections, archive/request indexes and reference release-build load tests remain open. Process death tests do not establish power-loss durability. No task acceptance, provider authentication or live host qualification is added. Feature 046 stays In progress; full US3/convergence/QA task flags remain open.

Fresh independent review passed three prefix goldens, 24 ingestion cases and the five-receipt CLI fixture, and found no runtime compatibility blocker. It also re-reviewed timeout/SIGINT cleanup, including the startup guard, and recommends approval subject to the full repository gate. Final full-check results are recorded below after execution.


## Repository-wide gate investigation

The first full repository run failed in unchanged `ai-gent-dbrun/tests/backup.rs`, test `real_exec::arguments_carry_no_password_and_stdin_reaches_the_tool`: the fake engine process could not start. The same symptom was recorded during earlier 046 development. No database-runner source was changed by this performance slice. Ledger suites and other executed checks passed; that does not turn the failed full run into a pass. An isolated diagnostic temporarily recorded only spawn error kind/errno, then ran 21 complete backup suites: all 273 test executions passed without reproducing an unexpected spawn error. The temporary instrumentation was fully removed and that worktree is clean. No errno evidence establishes a cause, so no speculative runtime/test fix was applied. The failed full log is preserved locally as `/tmp/ai-gent-usage-performance-first-failed.log`; the final gate is repeated with the failure recorded as inherited and unresolved. No assertion, deadline or required check is waived.


## Publication gate status — 2026-11-25

Two full `scripts/check.sh --quiet` runs completed with the same unchanged backup-fixture spawn failure; neither is reported as a pass. Their local logs are `/tmp/ai-gent-usage-performance-first-failed.log` and `/tmp/ai-gent-usage-performance-second-failed.log`. A further five exact-environment escalated backup suites passed (65 test executions), and the workspace-selected original backup harness passed alone too. These isolated passes do not override the full-run failures or establish their cause.

The independently reviewed performance work is published as a **work branch for review**, not merged into develop. Focused ledger tests, prefix goldens, lint and structural plan validation passed. The full repository gate remains blocked; no assertion, timeout, default parallelism or required check was disabled or waived. Full-workspace sanitized errno diagnosis is continuing in a separate clean-base worktree. Resolve the gate before landing; full feature and reference-load qualification remain open.
