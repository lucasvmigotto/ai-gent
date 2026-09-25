# Incremental migration patterns

Used by `project:refactor` (`../SKILL.md`).

## Choosing the seam

| Seam | Pattern | Fits when |
|---|---|---|
| HTTP routes in front of the old system | **Strangler fig** — a proxy or gateway routes path by path to the new implementation | the old system has a web/API surface; slices map to routes or screens |
| An internal module interface | **Branch by abstraction** — introduce an interface, make the old code one implementation, build the new one behind a flag, switch, delete the old | the change is inside one codebase |
| Messages or events | **Event interception** — the new consumer subscribes alongside the old, takes over topic by topic | the system is already asynchronous |
| The database | **Change data capture** — stream the old tables' changes into the new model | the old system can't be modified to dual-write |

## Verifying a slice

- **Characterization tests** — must stay green for everything the slice
  didn't intend to change.
- **Parallel run** — both implementations handle the same input; compare
  outputs, log differences, serve the old result until differences are
  explained. Scientist-style libraries (`github/scientist` and its ports)
  or a comparison in the proxy.
- **Shadow traffic** — mirror production requests to the new path and
  discard its responses; compare offline. Mind side effects: only for
  reads or with writes disabled.
- **Canary** — a small share of real traffic, with automatic rollback on
  the SLO.

## Moving data

1. **Expand** — add the new tables or columns alongside the old; nothing
   reads them yet.
2. **Migrate** — backfill in batches (resumable, idempotent), then keep
   them in sync (dual write in the application, or CDC). Verify with row
   counts, checksums per batch, and sampled record comparisons.
3. **Switch reads** — behind a flag, per slice.
4. **Contract** — once nothing reads or writes the old structure (check
   the logs and grants), remove it in a later release.

Dual writes without reconciliation drift silently: always pair them with
a comparison job and an alert.

## Cutover and rollback

- Every slice has a flag or route switch that returns traffic to the old
  path in minutes.
- Data written by the new path during a canary must be readable by the old
  one (or the rollback plan says how it's carried back).
- The old path is decommissioned — code, tables, jobs, credentials — in a
  planned slice with a date, not left running "just in case".
