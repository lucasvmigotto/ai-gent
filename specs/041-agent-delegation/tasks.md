# Tasks: agent delegation

- [x] T001 Owner review (approved 2026-11-02; open questions 3 decided, see spec) of this spec and its open questions (milestone 3.x, after 3.0.0).
- [x] T010 Adapter contract and the Codex adapter (`detect`, `ready`, `preflight`, `run`, `status`, `stop`, `result`) with a fake agent for tests. Verify: unit tests incl. commit preflight, stdin closed, PID-file stop. Evidence: PR #368: `cargo test -p ai-gent --test delegate` (9 journeys) and the `delegate::` unit tests, fake agent.
- [ ] T012a Supervisor step: re-fetch the cited sources for any claim that decides a verdict and compare (trial 3, finding 1).
- [ ] T010a The OpenCode adapter (`contracts/opencode.md`): detection outside `PATH`, model selection, per-run permissions, JSON event reading summed per step. Verify: fake-agent tests; the facts are already trial-verified (trial 3). Not used in ai-gent's own work (FR-002).
- [x] T011 Brief builder from `contracts/brief-template.md` with `allowed_paths`, worker pack and limits. Verify: golden briefs. Evidence: PR #368: `golden_brief` and `golden_minimal_brief` tests, fixtures in `rust/fixtures/delegate/brief/`.
- [x] T012 Supervisor diff checks (FR-011) with fixtures from trial 1 (marker line, out-of-scope path, environment prose, weakened test). Verify: each fixture is refused with its reason. Evidence: PR #368: `checks::tests` with fixtures from trials 1 and 2 in `rust/fixtures/delegate/checks/`.
- [ ] T010b The generic adapter template for further agents: what an adapter must prove (`detect`, `ready`, `preflight`, a trial against the checklist) before the agent is listed as supported.
- [ ] T013 Scheduler (with the simple-versus-judgment classification of FR-004a): tracks from a task list with disjoint paths, assignment rules (FR-004), worktree cap and disk floor counted together. Verify: fixture task lists.
- [x] T014 Delegation log and the `delegate` token tag (`shared/token-tags.json` and `.md` together). Verify: `scripts/check.sh`. Evidence: PR #368: `scripts/check.sh` green and the `status::tests`; `worker` facet added to the token vocabulary.
- [x] T015 `shared/delegate-worker-rules.md`: each guard's rule as a plain instruction; symlink into the plugin. Evidence: Codex draft reviewed by the supervisor, `scripts/check.sh` green.
- [ ] T020 The skill (`agents:delegate`): offer triggers, opt-in file, take-back. Verify: trigger and behavior evals.
- [ ] T021 Cross-agent review step (FR-010) and the landing checklist. Verify: a trial where the reviewer is not the builder.
- [ ] T030 Second trial and the comparison with `trial-1-codex.md` (SC-003).
- [ ] T040 Docs: README, docs-site skill page, `shared/releasing.md` untouched unless release flow changes.
- [ ] T050 `contracts/status.md` and the status-file writer used by the adapter and the supervisor (FR-016). Verify: schema tests; the adapter `status` reads only this file.
- [ ] T051 Workers model: status files and the delegation log in, rows and the pool header out, pure. Verify: unit tests with fixtures (queued, running, stalled, done, failed, taken back; no files).
- [ ] T052 Workers draw and the text form (golden fixtures, narrow and wide, ASCII fallback). Verify: draw tests.
- [ ] T052a The compact workers line for the pipeline pane (FR-018) from the same model, with fixtures (no workers adds no row). Verify: pipeline pane tests still pass unchanged when there are no workers.
- [ ] T053 `/workers` and `/workers-pane` commands, polling, option to turn off, no auto-open; the mod's types. Verify: `claude plugin test plugins/project`.
- [ ] T054 Check whether the mod API exposes Claude's running subagents and tasks (open question 3); if so read them directly, otherwise keep the supervisor-written files. Record the answer here.
- [ ] T055 README and docs-site entries for the pane; the `project` plugin's README row.

## Findings from the delegate-core build (PR #368), to settle in the spec

- `contracts/adapter.md` lists `--add-dir <common .git dir>` for Codex though trial 2 shows it does not make `.git` writable; the adapter leaves it off by default.
- `contracts/status.md` does not give the shape of `verdict`; implemented as `{result, reason}` with the rule id as reason.
- The brief names `shared/delegate-worker-rules.md`, which T015 still has to write.
- Preflight cannot prove "stdin is closed" beforehand; the proof is the run path itself (fake agent reading stdin to EOF).
- FR-011 checks are heuristics (prose files only for environment prose; protected paths hard-coded to this repo; Cargo version bumps not covered); `delegate check --evidence T010=...` passes task-mark evidence.
- Stop kills the process group (`kill -- -pid`, `taskkill /T /F`) from the PID file.
- Open hazard on Windows: a caller capturing `ai-gent delegate run` stdout through a pipe may block until the agent ends (the watcher inherits the pipe); callers redirect to a file.
