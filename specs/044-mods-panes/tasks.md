# Tasks: Mods panes say what the system knows

Slice 1 - Workers (`plugins/project/`)

- [x] T001 Exclude attention workers from the active list and give each group its own row budget, in `plugins/project/hooks/workers-model.ts` (`build`); test in `workers-model.test.ts`: 2 failed + 32 done + 1 taken back leaves counts, header and History as before and each failure once; acceptance US1.1, US1.8; FR-001, FR-002
- [x] T002 `Idle reason: not recorded`, `Cause: not recorded`, separate `State`/`Verdict` lines, `Tokens` omitted when null, in `workers-model.ts` (`build`, `details`); tests for failed+accepted, null reason, null tokens, nothing running; US1.2 to US1.4; FR-003, FR-004
- [x] T003 Marks on the glyph and state word of ATTENTION and WORKERS rows, drawn as `Text` segments inside one `Button`, in `workers-model.ts` and `workers-draw.tsx`; tests in `workers.test.tsx`: tones on the segments, segments join to the text, one press target per row, the press writes the toggle atom only; US1.6; FR-005
- [x] T004 Fixtures (`workers-fixtures.ts`): long shared-prefix tracks, failed+accepted, null cap; test that the opened `Track:` line is whole and the collapsed one is cut; update the old goldens one by one; US1.5, US1.7; FR-006
- [x] T005 Mutation pass, `claude plugin test plugins/project`, `claude plugin validate plugins/project`; README project-mod row and CHANGELOG `## Unreleased` line; `scripts/check.sh`; commit by path; Codex review; PR body notes `/reload-plugins`

Slice 2 - Worktrees (`plugins/agents/`)

- [x] T010 Spike: a test in `plugins/agents/hooks/worktrees.test.tsx` mounting the `worktrees` `Pane` with one `Button` row among `Text` lines; if the host refuses, stop and ask the owner (Q2); acceptance: the test passes or the owner has decided the fallback
- [x] T011 Arrow divergence and legend note in `plugins/agents/hooks/worktrees-model.ts` (`counts`, `row`, `notes`), narrow form `branch ↑a ↓b ●`, ASCII `^a vb`; tests at 100 and 50 columns, ASCII, null counts `?`; US2.1, US2.2, US2.7; FR-020
- [x] T012 One CI-failure signal: drop `checks failing` from `prCell`, keep `markOf`; tests: failing PR shows `failing` once per row and once in the summary, no-status branch shows none, `removable` hint still text; US2.3; FR-021
- [x] T013 Freshness: `Board.local`/`remote` `{ attemptedAt, okAt, error }`, set in `plugins/agents/hooks/worktrees.ts` (okAt only after a non-null parse), atom declared in `plugins/agents/types/index.d.ts`, notes `local: cached` and `GitHub: cached, read failed at <t>; last good <t>`; tests with a fake clock: failure keeps rows and `okAt`, one source failing keeps the other's rows, no-`gh` and non-GitHub notes unchanged; US2.5, US2.6; FR-023, SC-003
- [x] T014 Row expansion: `toggle` keys in `view`, `worktreesExpanded` atom, `Button` rows in `plugins/agents/hooks/register.tsx`, `text()` prints all open; tests: press opens full branch/path/dirty/divergence/PR/checks, survives a refresh, a vanished branch's key is dropped; refresh runs no spawn beyond the polls; US2.4; FR-022, FR-024; only after T010
- [x] T015 Update 022's tests that assert `+a/-b` by hand; mutation pass; `claude plugin test plugins/agents`, `claude plugin validate plugins/agents`; README and CHANGELOG lines; `scripts/check.sh`; commit by path; Codex review; PR notes `/reload-plugins`

Slice 3 - Pipeline (`plugins/project/`)

- [ ] T020 View-model order and counts in `plugins/project/hooks/pipeline-view.ts` (`build`): Next action before Implementation, `missing`/`stale`/`na` counts, `artifactsOpen = expanded.artifacts ?? hasProblem`; tests in `pipeline-view.test.tsx`: all-ready+1 N/A gives `7/7 applicable ready · 1 N/A`, a missing or stale artifact opens it and names the counts, an explicit false stays closed; US3.1 to US3.3; FR-030 to FR-032
- [ ] T021 Implementation vs readiness lines and the blocker/next-action link only when `Feature.deps` supports it, in `pipeline-view.ts`; tests: 32/43 implemented with 2 ready, 1 blocked gives 74% and a separate readiness line, a next action without deps stays neutral; US3.4, US3.5, US3.7; FR-033
- [ ] T022 Draw the new order and the collapsed Artifacts head with its `Button` in `plugins/project/hooks/pipeline-draw.tsx`; narrow `Current stage:` line; test in `pipeline.test.tsx`: block order by key, the Artifacts press writes the atom only, Next action press fills the prompt and runs nothing; US3.6; FR-030, FR-034
- [ ] T023 `text()` (compact and headless) follows the new order; update 029's goldens one by one; ASCII and width 59/80/120 cases; FR-041, SC-002
- [ ] T024 Mutation pass (always-open Artifacts, old order, percent from readiness), `claude plugin test plugins/project`, `claude plugin validate plugins/project`; README and CHANGELOG lines; `scripts/check.sh`; commit by path; Codex review; PR notes `/reload-plugins`

After all slices

- [ ] T030 Add 044 to `specs/README.md` (row, Notes with tasks done) and mark Status Implemented; owner sign-off on the real terminal look (not verified by tests)
- [ ] T031 Follow-up spec, number to be set (045 is taken; not this effort): the events and idle-reason reader that replaces `not recorded`
