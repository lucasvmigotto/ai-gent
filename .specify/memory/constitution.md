<!--
Sync Impact Report — 2026-11-17
Version: absent → 1.0.0 (first constitution; no prior principles overwritten).
New principles I–VII derive from owner task-accounting/tier requests, AGENTS.md,
shared/secrets.md and existing Rust/release architecture constraints.
Template compatibility: upstream templates unchanged; per-feature plans explicitly
instantiate all seven gates. Tests are required by each feature specification.
Deferred: implementation/live-host evidence, not constitution placeholders.
Default HTTP, browser accessibility and endpoint RED principles were adapted out:
this scope adds CLI/evidence contracts and no network API/UI. Existing UI gates remain.
Existing 001–045 release decisions/waivers remain authoritative in their own scope.
-->

# ai-gent Constitution

Scope: new forward specifications created from 2026-11-17. Existing accepted release contracts and owner waivers remain authoritative for their original scope. This first constitution was created from an absent file, not substituted for an existing one.

## Core Principles

### I. Executable contracts before integration
CLI/event/receipt contracts are versioned before consumers change. A breaking contract requires explicit migration and compatibility tests. HTTP contracts remain N/A under the existing Applicability decision.
**Checked by:** per-feature contracts, negative schema/CLI fixtures and compatibility tests in CI.

### II. Tests required per story
Every behavioral story has an independent acceptance test, written to fail before implementation. Structural checks alone cannot establish live runtime behavior.
**Checked by:** story-to-test matrix in qa.md, task IDs and passing CI evidence before Verified.

### III. Evidence before completion
New registered work under strict policy cannot be accepted without durable scoped usage, required checks and eligible exact-artifact review. Unknown, imported, shared and waived evidence remain distinctly labelled.
**Checked by:** completion verifier, receipt fixtures and changed-task CI gate planned in 050; no claim that these gates already exist.

### IV. Provider-neutral roles and honest capabilities
Claude and Codex are equal Tier 1 targets for this program. Other agents use the same contracts through declared Tier 2/3 capabilities. Role, identity, effective permissions and supported host surface are separate evidence dimensions.
**Checked by:** capability matrix and host/adapter conformance fixtures in 047/048/052/053.

### V. Least data and explicit authorization
No prompts, credentials or full transcripts enter tracked receipts. Worker identity does not authorize acceptance, owner-only exceptions or remote writes. Silence never grants permission.
**Checked by:** per-feature authorization matrix, redaction/path-boundary tests and shared/secrets.md review.

### VI. Bounded, recoverable operations
Every external read/process and collector drain is bounded. Acknowledged records survive recovery. Errors cannot manufacture success or trigger model recovery loops.
**Checked by:** fault-injection, resource-limit and replay tests in 046/047/048/050/052.

### VII. Versioning and review
Use work branches from develop, Conventional Commits, explicit staging and full scripts/check.sh before every commit. Skill/plugin/hook/shared-reference changes require a concrete proposal and approval under AGENTS.md. Plans cannot alter existing release gates implicitly.
**Checked by:** git diff, check log, per-slice independent review and release-gate diff review.

## Constraints
No new hosted service, HTTP API, database dependency or UI is required for this program. Existing portable Rust CLI/package architecture remains. Supported OS guarantees are qualified by tested capability, never inferred from successful compilation. No regulatory/provider billing certification is claimed.

## Governance
This file gates new plans. Amendments require a versioned change record and impact review across affected plans; MAJOR removes/redefines a principle, MINOR adds materially, PATCH clarifies. Unjustified violations block implementation; justified exceptions must name evidence and owner authorization. This constitution does not retroactively reset statuses of specs 001–045.

**Version**: 1.0.0 | **Ratified**: 2026-11-17 | **Last Amended**: 2026-11-17
