# Research and source register

Checked 2026-11-17. Primary sources only for external technical claims. Repository observations are pinned to develop 0a61fc4546491d6696fa46a1f2595958f4903ed0 unless explicitly stated. Proposed limits/defaults are design choices, not externally verified facts.

| ID | Source | Finding and consequence |
|---|---|---|
| S1 | [Codex hooks](https://learn.chatgpt.com/docs/hooks) | PostModelRequest usage may be null; Stop lifecycle is not a complete metering gate. Verify each supported payload with fixtures. |
| S2 | [Codex subagents](https://learn.chatgpt.com/docs/agent-configuration/subagents) | Children inherit permissions; a reviewer label does not establish a separate sandbox. Qualify context and permissions separately. |
| S3 | [Codex non-interactive mode](https://learn.chatgpt.com/docs/non-interactive-mode) | Structured execution events provide a detached integration surface. Terminal summaries need scope-aware deduplication. |
| S4 | [OpenAI plugin packaging](https://developers.openai.com/plugins/build/plugins) | Native packaging exists; compatibility/trust must be qualified rather than assuming generated skills are the only surface. |
| S5 | [Anthropic cache usage](https://platform.claude.com/docs/en/build-with-claude/prompt-caching) | Cache read/write input categories have distinct usage fields. Normalize total input using adapter-declared semantics, not a universal field-name sum. |
| S6 | [GitHub Spec Kit](https://github.com/github/spec-kit) and installed CLI | Local specify 1.0.14.dev0 bundled templates/skills used for this planning run; no extension git workflow installed. |

S1 further documents unsupported ask and error/timeout behavior for host-local hook decisions. Therefore 052 treats enforcement as a tested capability and cannot claim its script controls every hosted or direct OS operation. The official manual was also fetched locally earlier in this session; live version-specific integration is still a build qualification step.

## Repository evidence

- rust/Cargo.toml: edition 2024, MSRV 1.92, existing clap/serde/serde_json/thiserror and test dependencies. Plans retain locked versions, rather than opportunistically choosing latest frameworks.
- rust/crates/ai-gent-core/src/delegate/: adapter.rs, schedule.rs, review.rs, watch.rs, sources.rs and host adapters are current integration points. source lexical matching and role assumptions need distinct migrations.
- rust/crates/ai-gent-core/src/tokens/: transcripts/attribute/report/history and their Python predecessor currently encode Claude-oriented accounting; future ledger is additive and output compatibility explicit.
- specs/039-release-3-contract/contracts/tier-policy.md and docs/product/rust-decisions.md D2 record old Claude > Codex > others policy. Owner's latest request explicitly changes the new program to equal Tier 1; release requirements are not rewritten implicitly.
- specs/README.md already marks HTTP contracts N/A; local CLI contracts are correct here. Specs 044/045 exist but were absent from its index at this base.
- [Published audit and reproducible probes](https://github.com/lucasvmigotto/ai-gent/tree/7e92b5c712bb7dbe96cd6bcd481f64e642aae004/docs/product/codex-parity) distinguish observed/inferred findings and expose known dependency-fix limitations.

## Unqualified external contracts

Do not rely on private host transcript layouts, undocumented provider exports, subscription quota APIs or untested effective-model fields. They require versioned adapters and real fixture evidence before complete/verified identity claims. No host authentication files or secrets are read by this plan. No numeric pricing or provider quota claim is made.

Additional local evidence: delegate/events.rs uses std::fs::File::try_lock and sync_data; its existing lock wait is 15 seconds, so 046 must parameterize the stricter proposed 5-second boundary. Cargo.lock contains uuid 1.29.2 and sha2 0.10.9/0.11.1 transitively; direct use/feature qualification is an explicit 046 task, not a claim that ai-gent-core currently depends on them.
