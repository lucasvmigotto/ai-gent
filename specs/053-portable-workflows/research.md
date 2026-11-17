# Research: Portable skills, installation and workflow profiles

Date: 2026-11-17. Read [program research](../../docs/product/agent-parity-plan/research.md) for dated primary sources, capability caveats and counter formulas. Audit coverage: F01, F12, F13.

## Decision

Change skill/reference text only in implementation after presenting an explicit change set under AGENTS.md. Centralize capability data and generate/check README and docs-site projections. Reuse setup AGENT_IDS and package allowlist; do not create a second host registry. Keep legacy release 039 unchanged until its owner adopts the program tier amendment.

## Rationale

The existing workspace and adapters are the integration point. Reusing their contracts avoids a parallel toolkit and preserves the owner's ongoing 3.0.0 work. Host/version details are adapter capabilities, not assumptions in the domain.

## Alternatives Considered

1. Markdown-only task declarations: rejected for authoritative completion because they are mutable and cannot prove collection or exact review.
2. A Claude-specific implementation with Codex stubs: rejected by the owner's equal Tier 1 requirement.
3. A mandatory hosted metering service or database: deferred; not necessary for local crash-safe evidence and would add deployment/credential dependencies.

## Observed vs Proposed

[KNOWN from repository] Existing code resides under `rust/crates/ai-gent-core/src/workflow` or adjacent current modules; baseline is 0a61fc4. Existing executable behavior is not evidence that these new requirements are built. [ASSUMPTION] Future host versions retain a usable collection surface; otherwise adapters report unsupported/partial and the core gate remains truthful. Verify all host payloads against fixtures before qualification.

## Clarification Coverage

Scope, identities, outcomes, permissions, recovery, project applicability and measurement quality are explicitly decided in spec.md, data-model.md and backend.md. No product question blocks writing tasks. Implementation probes may invalidate a host capability; record that as evidence and revise its qualification without inventing support.
