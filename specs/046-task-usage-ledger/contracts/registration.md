# Registration slice v1 — implemented wire contract

046 US1 exposes `ai-gent usage register|segment|transition|replay`. Mutations accept `--input <regular-file|->`; `--root <project>` and `--protocol-version 1` may appear before or after the action. `--json` returns the v1 envelope in cli.md. New operations use exit 0/2/3/4/5 without changing any legacy command's exit or output contract. Invalid clap syntax retains the existing global argument-error envelope.

The successful fixture is `rust/crates/ai-gent/tests/fixtures/agent_046/register.json`. Unknown fields, nil/invalid UUIDs and unsupported versions are rejected. All metadata selectors are 1–256 ASCII letters/digits or `-_.:/@+`; no prompt, token value, environment or arbitrary configuration body is accepted. `config_digest` is `sha256:` plus 64 lowercase hex characters. Optional fields may be absent or null; absent effective identity stays unknown, never inferred from agent_app.

## Register

Envelope requires schema_version=1, request_id (UUID), repository_id (UUID), operation=register and payload. Payload requires task (repository_id, feature_id, task_id), phase, role, purpose and configuration. Optional: attempt_id (join an existing attempt), parent_attempt_id (create a new related attempt), parent_run_id, run_id, host_session_id. attempt_id and parent_attempt_id are mutually exclusive.

Task repository must equal envelope repository; feature slug matches `NNN-lowercase-kebab`, task ID `T` plus at least three digits. The task must appear exactly once as a checkbox in `specs/<feature>/tasks.md`, a bounded regular file within root. The task-definition fingerprint ignores checkbox state and includes the task ID and remaining text. Joining an attempt requires the same TaskKey and fingerprint. Existing parent run/attempt must have the same TaskKey; append-only parent references prevent cycles. Cross-task/shared parent scopes are deferred until shared-scope registration is specified with collectors.

Phases: planner, coordinator, builder, reviewer, repair, verification, recovery. Roles: planner, coordinator, builder, reviewer, reader. Planner/coordinator/builder/reviewer phases use the matching role; repair uses builder; verification uses reader; recovery uses coordinator. Purposes: implementation, retry, review, repair, planning, coordination, verification, recovery. Neither role nor purpose is a permission grant.

Configuration requires agent_app, agent_version, config_digest. Optional strings: requested_provider/model, effective_provider/model, effort, model_vendor, service_tier, provider_region. A host may execute a model from another provider. Effective identity supplied by this manual protocol is reported, never verified; absent provider or model gives unknown identity quality. Collectors qualifying identity are later work.

Result: scoped task/fingerprint, attempt UUID and local ordinal, parent attempt/run, run UUID, optional host_session_id, phase/role/purpose, generation=0, state=registered, one immutable segment (UUID, configuration, identity_quality), usage_quality=missing. UUIDs use OS-random v4 generation. A caller-provided run ID may not already exist.

## Segment

Same envelope with operation=segment. Payload: run_id, expected_generation, configuration. A changed configuration appends a new segment and increments generation; earlier segments retain IDs/content. An identical configuration, terminal run, missing run, stale generation or cross-repository request cannot switch segments. No existing segment can be edited. Host/session changes requiring a different execution context use a separate registered run.

## Transition

Same envelope with operation=transition. Payload: run_id, expected_generation, state. Allowed: registered→running/failed/cancelled and running→finished/failed/cancelled. Terminal states are immutable. There is no accepted/integrated state; finished still has missing usage. Lifecycle closure and metered acceptance belong to 047/050.

## Durability, idempotency and replay

request_id keys the normalized typed request, including operation. Repeating the same request returns its exact original result even after later segments/transitions/task edits; changed contents conflict. A retry flushes the existing record again before acknowledgement, covering a prior write followed by failed flush. One advisory OS lock serializes the journal (5-second wait maximum), including request lookup, validation, append and flush. Each frame contains version, sequence, canonical payload byte count, SHA-256 checksum and request/result pair. Replay re-evaluates domain transitions against the stored immutable result, not current task text.

The private journal is `.ai-gent/usage/journal/registrations.jsonl`. Registration creates a self-ignored `.ai-gent/usage/.gitignore` containing `*` before recording data, including in other projects. Existing conflicting ignore markers are preserved and cause refusal rather than an overwrite. Unix-created directories/files are 0700/0600, paths reject symlinks/non-regular files, and final Unix file opens use no-follow/nonblocking flags. Locks release on process death. Local same-user tampering and ancestor replacement races are not an adversarial security guarantee. Repository UUID is explicit and bound by the first durable record; strict owner-reviewed repository policy and worktree-wide supervisor admission are later integration work. Attempt ordinal is local to this journal, not a cross-machine clock.

`replay` validates bounded records and returns repository ID, sequence and latest runs without creating a journal. A torn tail or corrupt interior fails closed and preserves all bytes; automated quarantine/repair, receipts/export, usage ingestion/dedup and scalability qualification remain 046 US2/US3 work. Input/frame limit 1 MiB; current registration journal read/size ceiling 64 MiB. Unix flushes file and containing directories before mutation success. Windows directory durability/ACL behavior remains unqualified; no new platform guarantee is claimed.
