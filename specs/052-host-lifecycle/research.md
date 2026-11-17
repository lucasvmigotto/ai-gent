# Research: Tier 1 lifecycle, guard and recovery integration

Date: 2026-11-17. Read [program research](../../docs/product/agent-parity-plan/research.md) for dated primary sources, capability caveats and counter formulas. Audit coverage: F04, F07, F08, F09, F12.

## Decision

Add host normalization and lifecycle modules behind the existing Rust guard/delegate/doctor APIs. Keep sh, Rust and OpenCode guard parity where their shared decision contract changes. Hooks are capability-dependent adapters, not acceptance authorities. Extend project helpers with versioned history inputs and an explicit identity field.

## Rationale

The existing workspace and adapters are the integration point. Reusing their contracts avoids a parallel toolkit and preserves the owner's ongoing 3.0.0 work. Host/version details are adapter capabilities, not assumptions in the domain.

## Alternatives Considered

1. Markdown-only task declarations: rejected for authoritative completion because they are mutable and cannot prove collection or exact review.
2. A Claude-specific implementation with Codex stubs: rejected by the owner's equal Tier 1 requirement.
3. A mandatory hosted metering service or database: deferred; not necessary for local crash-safe evidence and would add deployment/credential dependencies.

## Observed vs Proposed

[KNOWN from repository] Existing code resides under `rust/crates/ai-gent-core/src/hosts` or adjacent current modules; baseline is 0a61fc4. Existing executable behavior is not evidence that these new requirements are built. [ASSUMPTION] Future host versions retain a usable collection surface; otherwise adapters report unsupported/partial and the core gate remains truthful. Verify all host payloads against fixtures before qualification.

## Clarification Coverage

Scope, identities, outcomes, permissions, recovery, project applicability and measurement quality are explicitly decided in spec.md, data-model.md and backend.md. No product question blocks writing tasks. Implementation probes may invalidate a host capability; record that as evidence and revise its qualification without inventing support.
