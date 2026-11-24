# Process-termination evidence — 2026-11-24

Status: Unix process-death tests, not power-loss or cross-platform durability qualification. The fixture control exists only in the unit-test binary under `cfg(test)`; production CLI callers cannot select checkpoints or pause the process through environment variables.

`rust/crates/ai-gent-core/src/usage/process_crash_tests.rs` contains six parent tests and one ignored child fixture. Parents invoke that fixture explicitly, wait for a bounded ready-file handshake, assert acknowledgement presence/absence, terminate it with `Child.kill`, and assert Unix signal 9. A drop guard kills/reaps children even on test failure; the child has a bounded fallback if its parent disappears.

Nineteen actual child terminations cover:

- registration append before write, after write, after flush and after acknowledgement;
- a partial registration frame requiring explicit, preserving recovery;
- snapshot publication before write/flush/link, after atomic link and after acknowledgement;
- recovery before/after truncation, before directory sync and after acknowledgement, including retry after later valid work;
- usage append before write, after write, after flush and after acknowledgement;
- partial usage append preserving earlier acknowledged tokens through explicit recovery.

Assertions verify durable previously acknowledged records, no premature acknowledgement, an absent or complete final snapshot object, exactly-once retries, preserved damaged originals, and recovery that does not truncate later valid work. The fixture is intentionally ignored for ordinary direct test discovery; all six parent tests invoke it with their own synthetic roots.

The isolated subprocess suite passed locally: six parent tests passed, zero failed, one fixture ignored by ordinary discovery. Combined-source validation and full `scripts/check.sh --quiet` passed on 2026-11-24; see `transfer-slice.md` for the final gate evidence. No paid host, provider or remote service participates.

These tests use synthetic temporary roots on the local host (whose `/tmp` is tmpfs), and exercise process death while the kernel/filesystem remains running. They are not storage-hardware durability tests. They do not simulate power removal, storage-controller failure, remote filesystems, or qualify macOS/Windows. Reference scale/load and full feature convergence remain open.
