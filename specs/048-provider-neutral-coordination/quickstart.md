# Quickstart Validation: Provider-neutral planning and admitted execution

Status: implementation runbook, not instructions to invoke unbuilt flags.

1. Fetch `origin` and branch from current develop using the program handoff; reserve this feature's file ownership before changes. Set `SPECIFY_FEATURE=048-provider-neutral-coordination` and `SPECIFY_FEATURE_DIRECTORY=specs/048-provider-neutral-coordination` explicitly.
2. Read spec.md → plan.md → contracts/cli.md → backend.md → qa.md → tasks.md. Check dependencies 46 are available and rebase only your work branch under git:workflow.
3. Write the story fixtures first at `rust/crates/ai-gent/tests/agent_048.rs`. After implementation, run from `rust/`: `cargo test --locked -p ai-gent --test agent_048`. This test target is planned and does not exist yet.
4. Exercise each positive/negative journey in qa.md using a temporary repository and fake host. Re-run mutations with the same request ID; assert exactly-once durable effects and inspect sanitized evidence.
5. For any live support claim, record host version, OS, surface, commands, expected/observed behavior and evidence digest. Without a real probe keep that capability unqualified.
6. Run `python3 docs/product/agent-parity-plan/validate_plan.py`, then full `scripts/check.sh` before committing. Run `/speckit-converge` or follow its installed skill for this feature; independent review precedes Accepted/Implemented claims.

Expected outcome after implementation: all three stories pass the named fixtures, negative paths retain explicit gaps, and no downstream consumer mistakes execution for accepted work. Preserve existing 3.0.0 gate scope.
