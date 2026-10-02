# Writing the constitution

`.specify/memory/constitution.md` holds the principles every plan, every
code review and every `/speckit-converge` pass is measured against.
`project:spec` writes it once, before any feature is specified.

Read this when the project has no constitution yet. It carries the gate
test, the default text to start from, and the bar a finished constitution
has to clear.

## The gate test

Spec Kit's plan template puts a gate before Phase 0 research:

```markdown
## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

[Gates determined based on constitution file]
```

**The constitution determines the gates.** So this is the test of whether
a principle belongs in it:

> Every principle must be expressible as a pass/fail check with named
> evidence. A principle you cannot gate is a value, not a principle —
> move it to Constraints or drop it.

Each principle below therefore carries a `Checked by:` line naming the
check and where its evidence lives. That line is what `/speckit-plan`
turns into a gate and what `qa:strategy` turns into a CI check. A
principle written without one is a slogan.

## The default constitution

Start from this. Cut what the project genuinely doesn't need, and say in
the handoff which principles you cut and why — a constitution that
carries a principle the project ignores teaches every later stage that
the constitution is advisory.

```markdown
# <Project> Constitution

## Core Principles

### I. Contract-First API

`contracts/openapi.yaml` is the single source of truth for the
frontend/backend seam. Every feature's `contracts/` is a subset of it and
must match it. Changing the contract is a spec change and goes through
`backend:spec` — never an edit made so code passes.

**Checked by**: contract tests in CI; a per-feature subset check.

### II. Tests Required per Story

Every user story ships with tests, written to fail before the
implementation that satisfies them. A story is not done until its tests
pass. This overrides the Spec Kit tasks template, which calls test tasks
optional.

**Checked by**: test tasks present per story phase in `tasks.md`; CI runs
them.

### III. Security Baseline

OWASP ASVS L2 where the product handles personal data, L1 otherwise.
Validate every field on entry; allow-list binding, never mass assignment;
no secrets in the repository.

**Checked by**: `devsecops:supply-chain` gates (dependency audit, secret
scanning); the authorization matrix in each `backend.md`.

### IV. Accessibility

WCAG 2.2 AA on every user-facing surface: keyboard operable, visible
focus, status never conveyed by color alone, touch targets per the
design system.

**Checked by**: axe checks in the frontend build's per-phase checkpoint.

### V. Privacy and Data Minimisation

Classify personal data before storing it. Log no personal data. State
retention in the domain model, not in a service's defaults.

**Checked by**: the personal-data and retention section of `backend.md`;
review of log fields in `backend:build`.

### VI. Budgets Stated Up Front

Every operation the brief gives an NFR for records a p95 latency and a
throughput budget. Hot paths carry a query-plan concern.

**Checked by**: the performance section of `backend.md`; the load profiles
in `qa:load`.

### VII. Observability

Structured logs with a correlation id and no secrets or personal data;
RED metrics per endpoint; readiness checks real dependencies and
liveness checks none.

**Checked by**: the observability section of `backend.md`; the alerts
listed in `docs/product/delivery.md`.

### VIII. Versioning and Review

Conventional Commits, a branch per context, work enters `dev` and moves
up one level at a time. Contract and architecture changes are reviewed
before merge.

**Checked by**: `git:workflow`; the merge and promotion gates in
`devsecops:pipeline`.

## Constraints

Filled per project from the brief and the ADRs. Do not default these —
an invented constraint is worse than an absent one, because every later
stage treats it as decided.

- Supported platforms and browsers
- Privacy regime (LGPD, GDPR, …) and data residency
- Regulatory or certification obligations
- Environments the product must run in

## Governance

This constitution outranks habit, README prose and a plan's convenience.
A plan that violates a principle MUST fill the template's Complexity
Tracking table with the violation, why it is needed, and the simpler
alternative rejected. An unjustified violation fails the plan's
Constitution Check.

Amendments bump the version — MAJOR when a principle is removed or
redefined, MINOR when one is added or materially expanded, PATCH for
clarifications — and record what changed in the command's Sync Impact
Report. Every plan's Constitution Check and every `/speckit-converge`
finding cites a principle by number ("Constitution III").

**Version**: 1.0.0 | **Ratified**: YYYY-MM-DD | **Last Amended**: YYYY-MM-DD
```

## Deriving it from the brief

Principles come from three places, and nowhere else. If a candidate
principle traces to none of them, it is invented — cut it or ask the user
where it came from.

| Source | Becomes |
|---|---|
| The brief's NFRs — performance, security, accessibility, privacy regime, supported platforms | principles III–VII, each stated as a checkable rule |
| The user's standing practices — `git:workflow`, contract-first API, tests per story, WCAG 2.2 AA, no secrets in the repo | principles I, II, VIII |
| `docs/product/architecture.md` and its ADRs | principles the ADRs actually decided; Constraints |

Two things not to do:

- **Don't restate the stack.** Language, framework and project structure
  belong to `plan.md`. The constitution outranks; plans follow it.
- **Don't copy a principle that duplicates an ADR.** Cite the ADR. Two
  sources of truth for one decision drift apart.

## When a constitution already exists

Load it and keep it. Never overwrite: amend it under Governance, and say
in the handoff what changed and which principles the new features were
checked against. A constitution that silently resets loses the reasoning
behind every principle in it.

If it exists but is thin — prose with no gates — that is an amendment, not
a rewrite: add the missing `Checked by:` lines, note the version bump, and
keep the principle text as the author wrote it.

## Before finishing

- [ ] Every principle is a gate: a `Checked by:` line naming the check and its evidence
- [ ] Five to nine principles — past nine the Constitution Check stops being a real gate
- [ ] Declarative and testable; no bare "should" where a MUST is meant
- [ ] Every principle traceable to an NFR, a standing practice or an ADR
- [ ] No stack, framework or structure restated from `plan.md`
- [ ] Principles numbered, so converge and plan can cite them
- [ ] Constraints filled from the brief, with nothing invented
- [ ] Governance present: amendment procedure, version bumps, what a violation requires
- [ ] `Version` / `Ratified` / `Last Amended` filled, dates ISO
- [ ] Cut principles listed in the handoff with the reason