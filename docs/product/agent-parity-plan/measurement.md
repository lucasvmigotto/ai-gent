# Task measurement and completion contract

Status: required design for 046–051, not implemented. The owner requires actual per-task usage and multi-model/provider builder–reviewer records.

## Identity and registration

Register `repository_id / feature_id / task_id / attempt_id / phase / run_id` before invoking a model. A run has one role and one execution scope. Its scope is either one TaskKey or an explicitly registered shared beneficiary set. Each provider/model/config change starts an immutable segment; do not overwrite the original identity. Requested model, effective reported model, provider endpoint identity, host application and effort are separate fields. Unknown effective identity remains unknown.

Record every planner, coordinator, builder, reviewer, repair, verification and recovery run that uses a model. A failed request can consume usage; absence of reported failed-request counters is a completeness gap. No model call performed outside a registered scope can retrospectively become exclusively measured because its prompt mentions a task ID. Legacy imports retain their actual attribution quality.

## Counter semantics

Canonical counters are nonnegative integers or null. Store raw usage and the source adapter semantic version. `input_total` includes all input categories; `output_total` includes its reasoning subset where the source defines one. `cached_input`, `cache_creation_input`, `reasoning_output` are breakdowns, not automatically added again. An adapter must declare subset/additive relationships and scope (request delta, turn total or session cumulative). If relationships are unknown, keep raw fields and normalized nulls.

For OpenAI-style inclusive input, a source providing input=1000, cached=400, output=200, reasoning=80 contributes 1200 total, not 1680. For Anthropic-style disjoint input categories, source input=600, cache_read=300, cache_creation=100, output=200 contributes input_total=1000 and total=1200. Preserve cache creation TTL buckets where supplied. Different semantics require distinct adapter fixtures. These arithmetic examples are synthetic, not model pricing or an invoice.

Store observation IDs and request IDs; a terminal cumulative summary is a reconciliation observation, not a second contribution alongside all its request deltas. Select one authoritative contribution per request/scope with recorded supersession and conflict detection. Counter resets, negative deltas, inconsistent subsets or overlapping parent/child usage are explicit errors/gaps. A cached or reasoning null means unavailable, not zero. A report can display a known lower bound while withholding an exact total.

## Builder–reviewer and repair links

Review record: review run/session → builder run → exact artifact digest → review policy → findings/dispositions. Segments supply actual provider/model for both sides; multiple segments create multiple observed pair contributions rather than relabelling the whole run. Require fresh context, not necessarily a different provider. Include failed reviews, repair attempts and re-review. Metrics include all tokens, wall-clock span versus summed active duration, review rounds, finding dispositions, sample count and missingness. Findings require evidence/disposition; raw counts do not measure model quality.

## Completion protocol

1. Supervisor registers task/attempt/run and protected collection scope; admission reserves capacity before launch.
2. Host and collector append bounded usage/lifecycle observations; final execution output is `execution_finished`, not accepted.
3. Finish creates an intent. Collectors drain final terminal usage without restarting/invoking a model. Review/check evidence can arrive independently.
4. Missing run closure => accounting_pending; missing eligible review/checks => review_pending with separate reasons. Work and commits remain preserved.
5. Verifier evaluates stored policy, registration coverage, all attempt/run closures, disjoint contributions, evidence quality, identity requirements, exact artifact checks and review.
6. Atomically publish immutable receipt with durable evidence references; then accepted becomes observable. A later integration attestation records the landed revision and covered-tree equivalence.

The strict profile requires complete collection for all registered runs, known validated normalization, verified effective model/provider identity where the host exposes it, closed shared-scope references and explicit attribution quality. If a Tier 1 surface cannot supply a required dimension, that surface remains unqualified for the guarantee until an alternative source qualifies; owner exemption remains separately labelled. An otherwise useful report can still show partial/imported work.

## What can be certified

- Schema/identity, deduplication, source integrity relative to recorded digests, arithmetic and run closure can be checked deterministically.
- Exact artifact/check/review links and local policy eligibility can be checked deterministically when their evidence is available.
- Source authenticity requires a trusted collection boundary; same-user writable logs cannot prove it against a malicious worker.
- Provider export reconciliation strengthens coverage for matched request IDs/window, but availability varies. Unmatched/dropped requests remain visible; local hashes cannot prove complete provider billing.

Default label: `locally verified usage receipt`. Reserve `provider reconciled` for an actual reconciliation artifact. Never label either a financial audit certification. Optional signing/remote protected collector is a future assurance extension, not required to ship the local toolbox.

## Evidence export and costs

Commit only sanitized normalized events, source digests/metadata and receipt/review/check manifests required for independent verification. Keep full transcripts, prompts, account identifiers and absolute private source paths outside Git. The CI verifier must not need private transcripts to check the declared local guarantee; provider reconciliation has a separate evidence policy.

Cost is an estimate from an explicit provider/model/service-tier/currency/effective-date price snapshot with category rates and rounding rules. Unknown category or model yields unknown cost. Subscription allowance is not inferred from token usage. Rename old billable fields only in versioned output and retain compatibility readers.
