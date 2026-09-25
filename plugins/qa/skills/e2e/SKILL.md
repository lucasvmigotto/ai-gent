---
name: e2e
description: Write and run cross-stack end-to-end journeys, Playwright by default, against the full devcontainer stack — dev identity personas, Mailpit, seeded data — covering negative paths, authorization, accessibility and mobile, with no sleeps, isolated data and flake quarantine; marks features Verified when they pass. Use for "write e2e tests", "test the whole flow".
---

# End-to-end tests

## Before starting

1. Read `../../references/qa-quality.md` and `../../references/pipeline.md`.
2. Required: `specs/NNN-*/qa.md` journeys (from `qa:strategy`) — if
   missing, derive journeys from the feature's acceptance scenarios and
   `ui.md` flows, and record them in `qa.md` as an `[UPSTREAM GAP]` fix.
3. Read `ui.md` (routes, states, copy keys), `backend.md` (errors, authz
   matrix), `contracts/openapi.yaml` (for API-based setup), and the
   devcontainer/infra setup (how to bring the stack up, how each persona
   logs in, Mailpit, seed data).
4. The feature must be **Implemented** on both sides; e2e against a mock
   API is a frontend test, not an e2e test.

## Setup

- Playwright project in `tests/e2e/` (or the path the plan sets), its
  own `package.json` if the frontend's tooling shouldn't carry it.
- `playwright.config`: `baseURL` from env, projects for Chromium,
  Firefox, WebKit and one mobile viewport (Chromium-only in PR CI if the
  full matrix is too slow — full matrix on main/nightly), `trace:
  'retain-on-failure'`, screenshots and video on failure, `retries: 0`
  locally and at most 1 in CI **with flake reporting**.
- Stack: bring up the devcontainers and `devcontainer:infra` compose (or
  the CI equivalent); wait on health endpoints, not sleeps.
- **Auth**: log in once per persona (from `qa.md`) through the real login
  flow — the dev IdP's login page (mock-oauth2-server/Keycloak) for OIDC,
  the link or code read from Mailpit for magic-link or one-time-code
  login, the app's own form otherwise — save `storageState` per persona,
  reuse it across tests. At least one test exercises the login flow
  itself.
- **Data**: create what each test needs through the API (fixtures using
  the contract), unique per test (suffix with the test id), clean up
  after; never depend on another test's data or on order.
- **Email flows**: read the message from Mailpit's API
  (`GET /api/v1/search?query=to:<addr>`), follow the link, assert on it.
- **Terminal interfaces** (when the project has one): drive the real
  binary against the running stack — the TUI through the stack's
  keystroke harness or a pseudo-terminal (`pexpect`, `teatest`), the CLI
  by asserting `--json` output and exit codes; keep a `vhs` tape per
  journey as the reviewable record.

## Writing journeys

- One spec file per journey from `qa.md`; test names say the user
  outcome ("member books the last slot and gets a confirmation email").
- Selectors: `getByRole`, `getByLabel`, `getByText` with copy from the
  locale files — these double as accessibility checks. `data-testid`
  only when no accessible handle exists.
- Web-first assertions (`await expect(locator).toBeVisible()`), never
  `waitForTimeout`.
- Cover per journey: the happy path; validation errors; a persona
  without the permission being denied (UI and direct URL); an empty state; a server
  error state (stop or stub one dependency where the stack allows) when
  the feature's risk is High.
- Accessibility: run axe (`@axe-core/playwright`) on each key screen of
  the journey — zero violations at the WCAG 2.2 AA tags.
- Keep journeys few and meaningful; anything testable at a lower layer
  goes back to the build stage as a gap.

## Run, stabilize, wire into CI

1. Run each new test **10 times** (`--repeat-each=10`) locally/CI; any
   flake gets fixed before merge.
2. CI job (with `devsecops:pipeline`): start the stack with the same
   images, run the suite, upload the HTML report and traces as artifacts.
3. Quarantine policy from `test-strategy.md` for tests that flake later.

## Verified

When a feature's journeys pass in CI (and `qa:load` passed where the
feature has load targets), set its status to **Verified** in
`specs/README.md` and note the run. If they fail, file the defect with
the trace, keep the status Implemented, and tell the owning build stage.

## Handoff

Journeys covered per feature, pass/flake results, defects filed with
evidence, statuses changed, and the CI job. Stop any stack you started
only for the run. Commits follow `git:workflow`.
