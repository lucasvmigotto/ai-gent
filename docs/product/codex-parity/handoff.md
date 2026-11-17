# Codex parity handoff

Updated: 2026-11-17. Built by Codex in the `fix/delegate-dependencies`
worktree. Status: initial dependency slice complete and locally validated;
ready for the next agent's independent review. No other edit track is active.

## User intent and constraints

> This project is mostly Claude-oriented - maybe because it was develop coop with them. In any case, this project idea and core concept is to be a major and helpful code agents tool-box for development and applications kick-start, planning and all other phases that include software lifecycle.

> Bear in mind, im running 3.0.0 development in parallel at home, so, if you want to develop something, branch-off and work.

> Document this excellent analysis and start development - consider that there is a second code agent catching up later on your work

`AGENTS.md` requires a proposal and approval before changing a skill, plugin,
hook, or shared reference; full checks before each commit; explicit-path
staging; and no manual version bumps. Do not infer permission to push, merge,
publish, or change historical owner decisions from this handoff.

## Location and ownership

- Repository: `/home/lucas/src/ai-gent` (main checkout stays on main).
- Worktree: `.worktrees/delegate-dependencies`.
- Branch: `fix/delegate-dependencies`.
- Base: develop `0a61fc4546491d6696fa46a1f2595958f4903ed0`.
- Owned paths for this slice: `docs/product/codex-parity/`,
  `rust/crates/ai-gent-core/src/delegate/schedule.rs`,
  `rust/crates/ai-gent/src/cmd/delegate.rs`, and the matching
  `rust/crates/ai-gent/tests/delegate_events.rs` / `delegate_plan.rs` tests.
- No other agent was spawned by this session. An external second agent is
  expected to catch up later; avoid overlapping edits and re-check remote
  commits before resuming.
- No remote writes, PR, merge, release, or tag requested.
- The implementation and this handoff are committed together. Locate that
  commit with `git log -1 --format='%h %s' -- docs/product/codex-parity/handoff.md`
  on this branch. The branch is local and has not been pushed or merged.

## Read first

1. `README.md` beside this file: active scope and open limitations.
2. `audit-2026-11-17.md`: full historical analysis and F01–F14.
3. `baseline-evidence.json`: observed original failures.
4. This branch's diff and git status: files may have changed since the last
   handoff update. Preserve any changes you did not make.

## Implementation decisions

- Keep current track grouping and public status schema in the first slice.
- Validate the dependency DAG and topologically order each track.
- Preserve track IDs from the first declared task so corrected execution
  order does not rename an existing dispatch. Preserve existing grouping;
  splitting dependency edges from ownership conflicts is future work.
- An execution exit is not acceptance. The first gate requires a completed,
  supervisor-accepted non-reviewer run. Independent review and integrated
  revision evidence remain follow-up work; do not mark all of F02 resolved.
- Prefer the newest task attempt, including failed/taken-back attempts,
  instead of unioning completion from historical runs.
- Do not alter Claude/Codex assignment policy until its explicit replacement
  is proposed and approved. The original policy was owner-directed.

## Validation

- Red: original scheduler failed dependency order and cycle regressions;
  original CLI failed the pending-verdict dependency regression.
- Green: 12 scheduler unit tests; 11 delegate-events and 9 delegate-plan CLI
  tests. The final full run also checks the track-ID preservation assertion.
- Full `scripts/check.sh --quiet`: **passed**, exit 0, final line
  `all checks passed`. This includes workspace tests, fmt and clippy with
  warnings denied, guard/parity checks, installer, packaging, migration,
  rollback and update tests. `git diff --check` also passes.
- The first full run was interrupted after detecting inherited `NO_COLOR`
  breaking the installer's color assertion and temporary-home Rust toolchain
  downloads. No tests or gates were weakened.
- One normalized full run caught a missing-panics-doc clippy diagnostic in
  the new track anchor code. Replaced the unchecked assumption with an
  explicit error; workspace clippy passed, then the full script was rerun
  successfully. No lint suppression was added.
- Passing command, from this worktree:

  ```sh
  env -u NO_COLOR RUSTUP_HOME=/home/lucas/.rustup \
    CARGO_HOME=/home/lucas/.cargo \
    CARGO_TARGET_DIR=/home/lucas/src/ai-gent/rust/target \
    scripts/check.sh --quiet
  ```

  The explicit toolchain paths prevent temporary-home tests from reinstalling
  Rust. Cargo's build cache was reused; source edits remain in the worktree.
  Final log: `/tmp/ai-gent-codex-full-check-normalized.log` (temporary).
- The optional cargo-deny gate was skipped because cargo-deny is not installed.
  Validation was on Linux; no Windows/macOS or remote CI result is claimed.
- `dependency-slice-evidence.json` contains normalized observations from the
  rebuilt binary. The original ordering/cycle/rejected-dependency probes now
  show corrected behavior. Unrelated findings remain open.
- No paid eval or live agent was run. Review so far is the current session's
  diff review; no independent-agent review is claimed.

## Next

1. Read this branch's diff against `0a61fc4` and independently review the
   dependency slice. Re-run the focused tests as needed; do not repeat the
   entire audit or the already landed develop branch rename.
2. Inspect current status, worktrees, and remote changes before claiming work.
   Publish/land this branch only with the required permission; it is not yet
   visible to an agent on another machine through origin.
3. Continue the staged plan in `README.md`. The next small candidate is F11's
   honest lexical-match outcome, with its public contract and dependent
   instructions reconciled. Coordinate any shared CLI edits with concurrent
   work. Do not claim that the broader F02/F03 lifecycle/grouping redesign,
   Codex coordinator policy, hook integration, or recovery work is finished.
