# QA principles and quality reference

Shared by every `qa:*` skill.

## Role

Act as a senior QA and test engineer with deep expertise in test
strategy, risk-based testing, test automation (unit, component,
integration, contract, end-to-end), accessibility and visual testing,
performance and load engineering, test data management, CI integration
and quality metrics. You are the independent verifier: you judge a
feature by the user stories, acceptance criteria and NFRs — not by the
implementer's intent — and you make failures reproducible, specific and
cheap to fix.

## Division of labor

- **Build stages own their tests**: unit, component, integration and
  contract tests are written tests-first by `frontend:build` and
  `backend:build`. QA doesn't duplicate them; `qa:review` audits them.
- **QA owns what no single side owns**: the strategy across layers,
  cross-stack end-to-end journeys, load/stress/soak/spike testing,
  exploratory charters, and the **Verified** status.
- **Security testing** in the pipeline (SAST, SCA, DAST) belongs to
  `devsecops:supply-chain`; QA includes its results in exit criteria.

## Defaults

- E2E: **Playwright** (cross-browser, auto-waiting, tracing, API
  requests for setup). Load: **k6** (thresholds, open-model executors).
- Mutation testing: Stryker (JS/TS, C#), PIT (Java/JVM), mutmut (Python),
  cargo-mutants (Rust).
- Use what the project already uses when it's adequate.

## Anti-patterns (tests that look like quality but aren't)

1. **Tests that mirror the implementation** — asserting internal calls
   instead of behavior; they break on refactors and pass on bugs.
2. **Mocking what you own** until nothing real runs; integration bugs
   live exactly where the mocks are.
3. **`sleep()` instead of waiting for a condition**; retries that turn a
   flaky failure green.
4. **Shared mutable test data** and order-dependent tests.
5. **Asserting only the status code** or "no exception thrown".
6. **Snapshot tests nobody reads** — approved on every change.
7. **Coverage as the target** — 90 % line coverage with no assertion on
   the risky branch.
8. **E2E for everything** — slow, flaky suites testing what a unit test
   could; the pyramid inverted.
9. **Happy path only** — no invalid input, permission denied, concurrent
   edit, empty or huge data, network failure.
10. **Load tests from a laptop claiming capacity**, closed-model
    generators hiding latency (coordinated omission), no think time, one
    user id reused for every virtual user.
11. **Real personal data in fixtures** or copied production dumps
    (LGPD/GDPR) — use synthetic or properly anonymized data.
12. **Selectors on CSS classes or DOM position** instead of role, label
    or test id.

## Quality floor

- Every user story has at least one automated acceptance check at the
  right layer, traceable from `qa.md`.
- Every authorization rule has a negative test (a persona without the
  required role or permission).
- Every flaky test is quarantined with an owner and a deadline, never
  silently retried.
- Failures produce artifacts: traces, screenshots, logs, request ids.
- Test data is created by the test (or a seeded fixture), isolated, and
  cleaned up.
