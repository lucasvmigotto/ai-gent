---
name: load
description: Design and run performance tests — smoke, load, stress, spike, soak, breakpoint — with k6 by default, from a workload model based on the capacity model, with SLO thresholds, open-model executors, server-side monitoring, honest environment caveats and CI wiring. Use for "load test the API", "stress test", "will it handle N users", "soak test".
---

# Load and performance tests

## Before starting

1. Read `../../references/qa-quality.md` and `../../references/pipeline.md`.
2. Read: `docs/product/architecture.md` (capacity model, SLOs, evolution
   triggers), the brief's NFRs, `specs/*/qa.md` load profiles,
   `contracts/openapi.yaml`, `backend.md` performance budgets, and
   `delivery.md` (environments).
3. **Authorization and target.** Only test systems the user owns and
   approves, never production or third-party APIs without written
   approval; stub external dependencies (WireMock/Prism from
   `devcontainer:infra`) so the test measures this system, not a vendor.

## Workload model

Before any script, write the model in `qa.md` / a `tests/load/README.md`:

- **Arrival rate** (requests or iterations per second) at average, peak
  and 10× from the capacity model — not a count of virtual users.
- **Operation mix** from the journeys and read/write ratio (e.g. 70 %
  browse, 20 % search, 10 % book), with **think time** between steps.
- **Data variety** — many users/ids/search terms (a CSV or generated
  pool), so caches and locks behave as in reality.
- **Test types** and their shapes:
  - smoke — minimal load, validates the script (every PR/main run);
  - load — expected peak, sustained (e.g. 15–30 min);
  - stress — ramp beyond peak to find where it degrades;
  - spike — sudden jump (enrolment day) and recovery;
  - soak — expected load for hours (leaks, connection exhaustion);
  - breakpoint — ramp until SLOs break, to find the ceiling.
- **Thresholds** from SLOs: `http_req_duration` p95/p99 per operation
  (tagged), `http_req_failed` rate, checks pass rate.

## Scripts (k6)

- `tests/load/<scenario>.js` with `scenarios` using **open-model**
  executors (`constant-arrival-rate`, `ramping-arrival-rate`) so a slow
  system can't reduce the offered load.
- `thresholds` encode the SLOs and fail the run; `tags` per operation so
  thresholds and reports are per endpoint.
- Auth: obtain tokens from the dev IdP per virtual user in `setup()` or
  per iteration as the real client would; never one shared token for
  everything unless that's the real pattern.
- `check()` on every response (status and a body assertion), not just
  timings.

## Running

- **Monitor the system under test** during every run: RED metrics per
  endpoint, CPU/memory, DB connections, slow queries, queue depth, GC —
  the report must explain *why*, not just *what*.
- Generator capacity: confirm the load generator isn't the bottleneck
  (its CPU, network).
- **Environment honesty**: results from the local devcontainer stack are
  **relative** — good for regressions between commits and finding
  obvious bottlenecks, never for capacity claims. Capacity claims need a
  production-like environment (same instance sizes, data volumes,
  network), and the report must say which one was used.

## Report

Per run: environment, commit, workload model, results vs. thresholds per
operation (p50/p95/p99, error rate, throughput), resource graphs or key
numbers, the first bottleneck found and its evidence, comparison with the
previous run, and recommendations (feed `project:architecture`'s
evolution triggers when a limit is near).

## CI

With `devsecops:pipeline`: smoke on main (or PR for backend changes),
load/stress/soak on a schedule or before release against staging;
thresholds fail the job; results archived for trend comparison.

## Verified

A feature with load targets becomes **Verified** only when its load
thresholds pass (together with `qa:e2e`). Update `specs/README.md`, or
file a defect with the report and keep it Implemented.

## Handoff

Results vs. SLOs, bottlenecks and evidence, environment caveats,
recommendations, CI jobs, and statuses changed. Stop generators and any
stack started only for the run. Commits follow `git:workflow`.
