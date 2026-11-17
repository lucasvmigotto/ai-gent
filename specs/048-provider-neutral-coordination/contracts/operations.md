# Operation payload and CLI mapping v1

All commands below are proposed, not implemented. Use `--input <file|-> --json --protocol-version 1` for the typed envelope in cli.md; reads may expose equivalent convenience flags. Protocol v1 inputs reject unknown required fields. Existing commands preserve their legacy argument/output/exit behavior unless explicitly opting into v1. In particular, legacy delegate exit 4 retains its old meaning; v1 exit codes apply only to new operations or v1 mode. New `delegate review-start|review-submit|review-verify` operations coexist with the legacy `delegate review` wrapper, which becomes a compatibility facade after migration.

| Operation | ai-gent command | Required payload fields | Successful result | Additional error codes |
|---|---|---|---|---|
| plan | `delegate plan` | scoped tasks, dependencies, ownership globs/resources, routes, policy digest | plan digest, ready/waiting tasks, reservation proposal | policy_denied, scope_mismatch, capability_missing |
| approve | `delegate approve` | plan digest, actor, authorization reference, expiry | admission digest, authorized scope | policy_denied, stale_generation |
| run | `delegate run` | approved plan/admission digest, scoped task, expected lease generation | registered run ID, lease, launch evidence | policy_denied, capability_missing, lock_timeout |
| recover | `delegate recover` | supervisor generation, last acknowledged sequence, mode=status|takeover | new generation, replayed events, pending actions | stale_generation, lock_timeout |

Identity references use UUIDs, content references use `sha256:<64 lower hex>`, UTC times use RFC 3339, bounds use nonnegative safe integers and TaskKey uses the schema in 046. The phrases in required fields identify typed domain objects defined in data-model.md and the shared schemas; arbitrary provider JSON is allowed only inside bounded raw_usage/declared extensions. Input authorization references are verified against supervisor/owner evidence, never trusted merely because a caller supplied a role string. Expected generation is required for mutable aggregate operations. All operations also permit the common I/O/lock/schema errors in cli.md as applicable; reads have no side effects.

Operation-specific preconditions and invariants are the matching FRs in spec.md. Success is idempotent under the same request ID; changing the body conflicts. Failed preconditions have no authoritative mutation. A valid status/report can contain pending/unknown data without claiming acceptance. CLI conformance tests must assert both result payload and persisted effects for each row.
