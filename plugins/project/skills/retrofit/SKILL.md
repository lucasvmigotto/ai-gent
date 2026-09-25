---
name: retrofit
description: Upgrade an existing project with its business behavior frozen — level patch (CVE fixes, in-range updates), minor (plus code adapting to deprecations, replacing abandoned libraries) or major (plus breaking runtime and framework upgrades, one major at a time) — behind characterization tests, with CVE reachability triage and an EOL table. Use for "fix the CVEs", "upgrade to Java 21".
---

# Retrofit — upgrade without changing behavior

Keeps what the application does exactly as it is, and renews what it runs
on: language runtime, framework, libraries, base images, build tools. The
contract is simple — the same inputs produce the same outputs after the
retrofit as before it, proven by tests written before anything changes.
When the business behavior itself should change, that's
`project:refactor`.

## Levels

The user picks one (default `patch`; ask when the request doesn't say).
Each includes everything in the levels above it.

| Level | Allowed | Not allowed |
|---|---|---|
| `patch` | lockfile refresh; patch and minor bumps within the declared ranges; minor bumps outside them when they fix a CVE and change no API the code uses; base image digest refresh within the same tag line | code changes beyond trivial ones (an import path, a renamed config key) |
| `minor` | code adapting to deprecations and API changes; minor upgrades of the runtime and framework (Node 20 → 22 LTS, Spring Boot 3.2 → 3.4); replacing abandoned or vulnerable libraries with maintained ones; automated migration tools | breaking changes to the project's own public API, data format or behavior |
| `major` | runtime and framework major versions (Java 11 → 21, Spring Boot 2 → 3, Angular 12 → 18, Python 3.8 → 3.12), build tool majors, replacing an end-of-life framework — however much code it takes | changing business behavior; skipping the one-major-at-a-time rule |

If a fix needs more than the chosen level allows, stop and ask: fix it at
the higher level, or record it as accepted risk in `retrofit.md`.

## Before starting

1. Read `../../references/pipeline.md` and `references/tooling.md`.
2. Read `project:introspec`'s output. If it doesn't exist, run at least
   its inventory, SBOM and test-run steps first — a retrofit needs to know
   what's there and whether it builds.
3. Confirm the level, the scope (all modules or some), the target
   versions if the user has them in mind, and the deadline. Work on a
   `git:workflow` branch — one per step, off the retrofit's working
   branch.

## Process

1. **Baseline.** Build and run every test suite in the project's
   container (`devcontainer:workflow`); record the results, the SBOM, the
   image size and, when there is one, a load smoke (`qa:load`). If the
   baseline is red, stop and report — a retrofit can't prove "unchanged"
   against a broken start.
2. **Safety net — before any upgrade.** Behavior is only frozen if tests
   say so. Use `qa:review`'s gap analysis on the critical paths, then add
   **characterization tests** where coverage is thin: they assert what the
   system does today, bugs included, at the outermost practical layer
   (HTTP responses recorded against the running app, job outputs, rows
   written, emails sent to Mailpit, files produced). Snapshot the database
   schema and the API description. All of it must pass on the baseline.
3. **Findings.**
   - **Vulnerabilities:** scan dependencies and images (`tooling.md`).
     Triage each CVE by **reachability** — is the vulnerable function or
     feature actually used here? — then by exploitability (CISA KEV
     listing, EPSS score, network exposure) and severity. Reachable →
     fix. Not reachable → record why as a VEX-style "not affected"
     statement, re-checked each retrofit.
   - **End of life:** runtime, framework, database, base image and OS,
     with end-of-support dates (endoflife.date) and the supported target.
   - **Outdated:** each dependency's current version, the newest the level
     allows, and what it takes to get there.
4. **Plan — `docs/product/retrofit.md`.** Level and scope; baseline;
   CVE table (id, package, severity, reachable?, KEV/EPSS, fix version,
   step); EOL table; the ordered steps. One concern per step (the
   runtime, the framework, one library family), each with its expected
   code impact, risk, how it's verified and how it's rolled back. Order:
   build tooling first, then runtime, framework, and libraries that
   depend on the framework last. The user approves the plan before step
   one.
5. **Execute, step by step.** For each step: upgrade, adapt the code
   (migration tools first, hand edits second), build, run everything. A
   failing characterization test means behavior changed: fix the
   adaptation or revert the step — **never edit the characterization
   test to match**, unless the user agrees the old behavior was a bug
   (then it's recorded as a finding, and arguably belongs in
   `project:refactor`). Merge each step back per `git:workflow` before
   the next.
   - **Major level:** one major version at a time, following each
     framework's official upgrade path (Spring Boot 2.7 → 3.0 → 3.x;
     Angular one major per step), with the tests green between majors.
     Deprecation warnings are cleared at each step, not accumulated.
   - A security fix that must change behavior (stricter parsing, a
     removed insecure option) is shown to the user before it lands.
6. **Verify.** Rescan: CVEs before and after, the SBOM diff, EOL table
   updated, every suite and the characterization tests green, image
   rebuilt (smaller, if the base changed, following
   `../../references/containers.md`), load smoke within the baseline's
   range. Update `architecture.md`'s stack and versions and the
   devcontainer and CI images (`devcontainer:setup`, `devsecops:pipeline`)
   so local, CI and production run the same versions.

## Coverage checklist

- [ ] baseline green and recorded before any change
- [ ] characterization tests cover every critical path and pass on the baseline
- [ ] every CVE triaged: fixed, or not-affected with a reason, or accepted risk the user approved
- [ ] EOL table complete; nothing past end of support left without a decision
- [ ] every change within the chosen level, or escalated and approved
- [ ] one concern per step; each step merged green before the next (majors one at a time)
- [ ] no characterization test changed to hide a behavior change
- [ ] devcontainer, CI and deploy images match the new versions
- [ ] `retrofit.md` has before/after numbers (CVEs, versions, image size, test results)

## Handoff

The level, what changed (versions before → after), CVEs fixed and
remaining with their reasons, what the user must do (secrets, rotated
credentials, infrastructure upgrades such as the database engine),
findings that belong in `project:refactor`, and when to run the next
retrofit (nearest EOL date). Commits follow `git:workflow`.
