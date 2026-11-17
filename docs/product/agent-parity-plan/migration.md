# Migration, compatibility and rollback

## Existing contracts affected

| Existing feature | Planned addendum | Backward compatibility |
|---|---|---|
| 033 guards | 052 host payload normalization and truthful decision capability | Preserve shared Rust/sh/OpenCode fixtures; unsupported host asks never silently become allow |
| 034 doctor | 052 trust/collector/host qualification diagnostics | Existing offline checks unchanged; new probes explicit and bounded |
| 035 tokens | 046/047 canonical observations, 051 projections | Legacy Claude heuristic readers remain versioned; no rewritten historical measured usage |
| 036 wt finish | 050 verified acceptance before completion/integration | Legacy unregistered scopes imported/unknown, no fake receipt |
| 037 spec/repo | 050 receipt-backed status and tracker readiness | Existing board permission gates/aliases preserved; no remote write by collection |
| 039 release | 053 prospective Claude+Codex Tier 1 successor policy | Do not change 3.0.0 release gates or platform waivers in this branch |
| 040 docs site | 053/054 evidence-backed compatibility/index projections | Existing browser/UI test requirements apply only if those surfaces change |
| 041 delegation | 048/049/050 neutral policy, review and acceptance | Published 7e92b5c fix is only ordering/cycle/accepted-result baseline, not full receipt compliance |
| 044/045 | No behavioral change | Index their existing draft specs honestly; avoid home agent's mod/board files |

## Expand → qualify → enforce

1. Expand: v1 identity/observation/receipt readers and journal registration behind explicit program policy. No existing status is silently reinterpreted. Use a tracked repository UUID and baseline commit; private runtime store remains git-excluded.
2. Observe: collect new work, expose missing counters/models and closure gaps. Import old history only as imported/unknown. Record migration dry-run and affected tasks.
3. Qualify: validate Claude and Codex fixture/live capability cells, same-host and mixed-provider review, fault durability, platform behavior and all completion consumers. Repair adapters before advertising strict support.
4. Warn: completion previews explain would-block reasons; dependencies/tracker still use the documented pre-cutover compatibility mode. Never label warn-mode completion as certified strict acceptance.
5. Strict: explicit cutover policy makes every newly registered task's completion consumers require a receipt. Changed-task CI checks baseline and policy updates, not just a worker's chosen task list. Exceptions require owner evidence and remain exempt in reports.

Policy downgrade, baseline advance and artifact exclusion changes require explicit owner review and a CI-visible diff. A worker cannot evade enforcement by editing task Markdown, removing registration from the manifest, changing the task ID or swapping repository UUID; compare against protected base and retained registration inventory.

## Schema migration

Schema major is required in every export and receipt. Reject unknown required majors; optional extensions live only under extensions. Preserve original bytes/digests; migrated normalization is a new version referencing old observations, never an overwrite. Validate all links before committing a replacement projection. Rebuild the disposable index after migration; leave old readers usable until the documented release boundary. Dated pricing is independent of normalization versions.

## Acceptance and integration

The receipt binds a task definition fingerprint and canonical artifact manifest excluding its own evidence/projection files. Review/check manifests reference the same digest. Integration attestation links to the actual landing commit; merges changing covered content require renewed evidence. A checksum proves bytes, not provenance of the producer. Required CI verifies the declared guarantee from sanitized evidence without depending on ignored local transcripts.

## Rollback

Stop new dispatch and preserve pending work; export/backup journal before migration. Roll back executable/adapter version through existing update mechanisms without deleting evidence. Restore a previous projection only after verifying it reads the retained schema. Disable strict enforcement only via explicit owner policy change recorded as such; do not relabel unverified/exempt results as measured completion. Keep old receipts readable or emit unsupported-version diagnostics. No destructive store pruning, migration auto-merge or history rewrite.

## Skill/reference amendment proposals for the build phase

Proposed changes are catalogued here, not applied: agents delegate/orchestrate/parallel use role/capability routing and executable registration/finish; shared agent roles/token tracking use receipt vocabulary and actual identity; project recap/checkpoint read actual host; project spec selects host integration; QA descriptions match implementation; install/docs generate one capability matrix; source-check language distinguishes lexical evidence. Present exact affected paths/text at each implementation slice and obtain AGENTS.md approval. Current runtime skill files remain untouched.
