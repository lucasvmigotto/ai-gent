# Storage, policy and trust boundary contract

Proposed paths are part of the implementation contract; none are runtime files created by planning.

| Path | Purpose | Git/trust policy |
|---|---|---|
| `.ai-gent/usage/journal/` | Framed private observations and replay cursors | Excluded, supervisor owned, restrictive permissions, bounded records |
| `.ai-gent/usage/projections/` | Rebuildable indexes/status views | Excluded, disposable, never authoritative |
| `.ai-gent/usage/runs/<UUID>/` | Private scratch and scoped collector IPC | Excluded, per-run, no broad TMPDIR exposure |
| `task-evidence/policy.json` | Repository UUID, baseline, rollout and evidence/review rules | Tracked, owner-reviewed and CI protected |
| `task-evidence/registrations/` | Sanitized immutable scope/run coverage manifests | Tracked on export; no task omitted from accepted accounting |
| `task-evidence/objects/sha256/<digest>.json` | Sanitized normalized events, checks, review and closures | Tracked content-addressed objects; prompts/transcripts excluded |
| `task-evidence/receipts/` | Immutable completion receipts | Tracked, versioned, links only to scoped sanitized objects |
| `task-evidence/integrations/` | Later landing revision attestations | Tracked, separate from accepted artifact identity |

The existing `.ai-gent/delegate.yaml` remains the delegation opt-in/routing input, not an unchecked way to weaken tracked completion policy. Its effective digest is registered with the approved plan; private machine overrides may tighten restrictions but cannot relax required tracked policy or CI protection. During 046 implementation, add actual runtime ignore rules deliberately and test packaging; this planning branch creates no private usage store.

Journal format v1: one UTF-8 JSON frame per line containing schema_version, monotonically increasing sequence, immutable request/event ID, payload byte length, payload and SHA-256 checksum over a documented canonical UTF-8 payload encoding. Use exact-length canonical serialization for digest inputs (sorted object keys, UTF-8, no insignificant whitespace, integers only in counter data); do not hash arbitrary pretty-printed JSON and later assume byte equality. A line is at most 1 MiB. An incomplete final frame is quarantined and not acknowledged; a valid interior frame followed by corruption blocks replay until explicit repair. Repair writes a new recovery event and preserves original bytes, never silently discards history.

Receipts and exports use same-filesystem temporary files, flush before atomic publication and directory durability where supported. Lock wait ≤5 s; never hold locks during provider calls. Refuse unsafe network filesystem assumptions until explicitly qualified. Windows replacement/create and crash semantics require real tests before an advertised guarantee. Locking provides concurrency correctness, not malicious same-user isolation.

Policy change verification compares the protected base policy, repository UUID, task definitions, coverage manifests and baseline against the proposed head. A worker cannot self-authorize a later baseline or remove a registered run. Owner authorization needs a verified external review/approval reference for protected CI changes; local `actor=owner` strings alone are not proof. CI validates the local integrity guarantee from sanitized bundles; authenticity against a compromised account requires a stronger signed/protected collector outside this program's default threat boundary.

Counter range is 0..2^53−1 for JSON interoperability; checked aggregation fails on overflow. Unknown fields are explicit null, unknown schema majors fail, raw adapter-specific counters preserve declared semantics. Domain verification additionally checks shared beneficiary rules, attempt coverage, fresh review context and artifact equivalence; JSON shape alone cannot establish any of them.
