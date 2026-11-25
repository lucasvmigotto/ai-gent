# Segmented metadata parser prototype

This is a **test-only prototype**, compiled solely through `#[cfg(test)]`. No new code enters the production binary, enables storage-v2 reads/writes or migration, adds a CLI command, opens files or changes existing journal behavior. It implements a narrow part of slice B in the proposed segmented-journal plan, not its complete exit criteria.

The parser checks literal canonical HEAD/manifest bytes, strict fields/schema/kind, expected repository identity, HEAD digest/generation binding, predecessor digest shape, global object UUID uniqueness, contiguous frame ranges, empty/nonempty active descriptors, checked arithmetic and proposed metadata byte/frame/object bounds. The legacy size exception is restricted to the first closed object per stream when a nonnil migration ID is declared. Presence of that ID does **not** establish actual migration provenance.

Interpretations used only by this prototype: generation starts at one, with a null predecessor only at generation one; an empty active descriptor starts at the preceding sequence plus one, while its committed sequence remains the preceding sequence. These have not enabled a runtime format.

Encoded input limits (4 KiB HEAD, 128 KiB manifest) are checked **before** deserialization. Descriptor-count limits are checked **after** serde builds its vector within that byte-bounded input. Do not claim every resource limit is enforced before allocation. Stronger field-count streaming validation remains a prerequisite to advertising the proposal's full allocation policy.

Metadata consistency does not verify source frames or raw hashes, prior-manifest linkage, historical retry locators, migration intent, orphan inventory, filesystem safety, durability, receipt compatibility or provider identity. Those obligations remain open, together with manifest retention, disk-backed reduction and integrity/contention architecture. Synthetic fixtures intentionally use unverified source digests.

Independent fresh-context review found no blocker within this narrow scope and independently ran the initial 15 tests successfully. The parent added canonical UUID spelling and within-stream UUID collision coverage. Final focused suite: 16 passed, zero failed. Pinned Clippy and format checks passed. Full uninstrumented `scripts/check.sh --quiet` passed on 2026-11-25, including installer/update and Rust workspace gates; local log `/tmp/ai-gent-usage-segment-metadata-full-check.log`. Plan validation and diff checks passed. The work branch was fast-forwarded to merged develop at `67cd7e3` without changing source content before this commit; it is published as `feat/usage-segment-view`. No runtime enablement or new PR is claimed. Feature 046 task flags remain unchanged; no full-feature or platform qualification is inferred.


Integration baseline: PR [#464](https://github.com/lucasvmigotto/ai-gent/pull/464) merged at `67cd7e3f3ccd80e3ef32142e0bdb522febe559e7` on 2026-11-25 after all exact-head CI checks passed, including all six Rust targets and packaging. This prototype is a separate continuation; PR #464 does not contain it.

Post-merge Docs CI run `45642249549` also passed for the exact integration merge SHA. Deployment was skipped as expected on develop. All integration work remains below main; no release or promotion was performed.
