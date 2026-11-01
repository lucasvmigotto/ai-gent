# Feature Specification: Guard evaluator

**Feature Branch**: `docs/specs-rust-core` (specification only; built on its own branches)

**Created**: 2026-11-01

**Status**: Draft

**Input**: D9, D10, D11, D12, D13 of `docs/product/rust-decisions.md`. Today five guards decide in five POSIX `sh` scripts (`plugins/{git,db,secrets,repo,devcontainer}/hooks/guard.sh`, 97+102+88+40+61 lines) and a JavaScript port (`opencode/guard/rules.mjs`, 358 lines) kept equal by a parity test. This feature replaces all six implementations with one evaluator in `ai-gent-core`, called by `ai-gent guard`, that also understands PowerShell and cmd.

**Depends on**: 032 (workspace, release, binary install). Read by 034 (doctor checks the guard), 039.

## Context and ground rules

Verified on 2026-11-01:

- A guard receives the Claude Code `PreToolUse` event JSON on stdin and prints `{"hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny|ask","permissionDecisionReason":"<guard marker> <text>"}}` or nothing (allow). Hook entries: git, repo, devcontainer and secrets match `Bash`; db matches `Bash|Read|Grep|Glob|Edit|Write|MultiEdit|NotebookEdit` and denies credential-file paths for the non-Bash tools.
- Reason text is contract, not decoration: `shared/guard-ux.ts` (copied into five `hooks/guard-ux.ts`) keys its explanations on a per-guard marker (`git:workflow guard:`, `db guard:`, `secrets guard:`, `repo guard:`, `containers guard:`) and `match` fragments, and `scripts/consistency.py` (`check_guard_ux`, line ~449) fails when a reason in a `guard.sh` has no entry. The Rust evaluator must print identical reasons.
- Existing vectors: `scripts/test-guard.sh` 44 `expect` calls, `test-db-guard.sh` 52, `test-secrets-guard.sh` 41, `test-repo-guard.sh` 16, `test-containers-guard.sh` 39 (plus its `leftovers.sh` cases, which are not guard decisions): 192 shell vectors. `opencode/guard/test/rules.test.mjs` (340 lines) holds `gitVectors`, `dbVectors`, `toolVectors`, `secretsVectors`, `repoVectors`, `containersVectors` and `adapterCases`, run against both `guard.sh` and `rules.mjs`. The two suites overlap heavily but are not identical; the union is the fixture set.
- The shell guards see the command JSON-escaped (`\n` and `\"` literal); `rules.mjs` sees the decoded string. The evaluator works on the decoded string (as `rules.mjs`), so the union fixtures must pass both views.
- Baseline latency of today's shell guards, measured on this host (Linux, one machine, 100 runs each, input `git status --short`, no branch lookup): git 3.3 ms, db 1.1 ms, secrets 0.8 ms, repo 0.6 ms, devcontainer 1.6 ms per run, process start included. These are the numbers to beat or match; `quickstart.md` makes the measurement repeatable.
- The guards are pattern matching on text, explicitly "not every obfuscation" (their headers). Parity means reproducing that behavior, not strengthening it; strengthening is a separate, flagged step.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - One implementation decides the same as today (Priority: P1)

**Why this priority**: D10. No behavior change is allowed to hide inside a port.

**Independent Test**: `cargo test -p ai-gent-core --test guard_parity` runs every fixture in `shared/guard-fixtures/*.json` for shell `sh` and asserts decision and reason equal the recorded values; `scripts/test-guard.sh` (and the four others) run the same fixtures against the `guard.sh` oracle until the cutover.

**Acceptance Scenarios**:

1. **Given** the 192 shell vectors and the opencode vectors extracted to fixtures, **When** the evaluator runs them, **Then** every decision and reason equals the value the shell guard printed when the fixture was extracted.
2. **Given** `git commit --no-verify -m "x"`, **When** `ai-gent guard --guard git` reads the event, **Then** it prints the same deny line as `plugins/git/hooks/guard.sh`.
3. **Given** a `Read` of the db credentials file under the ai-gent config directory, **When** the db guard evaluates, **Then** it denies with the credentials reason; **Given** a `Write` that merely names the file inside its content, **Then** no decision.
4. **Given** a commit on `main` in a worktree whose `.git` is a file, **When** the git guard evaluates, **Then** it asks and names the branch, like `git symbolic-ref` does.

### User Story 2 - The same intent is judged the same in PowerShell and cmd (Priority: P1)

**Why this priority**: D3 and D11; native Windows is a goal.

**Independent Test**: fixtures with `shell: "powershell"` and `shell: "cmd"`, including `variants` of each sh case, run in `cargo test` on all three OSes in CI.

**Acceptance Scenarios**:

1. **Given** the fixture `git commit --no-verify -m "x"` and its PowerShell and cmd variants, **When** evaluated with the right shell, **Then** all three deny.
2. **Given** PowerShell `git push; Write-Output hi` and cmd `git push & echo hi`, **When** evaluated, **Then** both ask (separators `;`, `&`, `&&`, `||`, `|` split segments in both shells).
3. **Given** a commit message holding the text `--no-verify` in quotes in each of the three syntaxes, **When** evaluated, **Then** none denies (quoted text is blanked in each shell's own quoting rules).
4. **Given** PowerShell `git commit -m 'it''s fine' --no-verify`, **When** evaluated, **Then** deny (a doubled single quote is an escaped quote, not the end of the string).
5. **Given** cmd `git push ^& echo hi`, **When** evaluated, **Then** the caret escapes the `&`, the line is one segment, and the guard still asks because it is a push.

### User Story 3 - Shadow mode proves it before it decides (Priority: P1)

**Independent Test**: with the shadow hook enabled, a scripted run of the whole fixture set through the real hook produces a log with zero disagreement lines; a deliberately broken rule produces exactly the expected disagreement line.

**Acceptance Scenarios**:

1. **Given** 2.27 is installed, **When** a session runs a Bash command, **Then** the shell guard decides (output unchanged) and one shadow record is appended to the shadow log.
2. **Given** the evaluator disagrees with the shell guard, **When** the record is written, **Then** it carries both decisions, the guard, the shell kind, a command hash and a rule id, and `ai-gent guard shadow-report` lists it.
3. **Given** the binary is absent, **When** the hook runs, **Then** the shell guard decides and nothing else happens (no error, no delay beyond one failed lookup).
4. **Given** the promotion criteria are met, **When** 2.28 ships, **Then** for a package install the hook calls the binary to decide; a git clone keeps deciding with the shell logic until git mode is removed, and the shell logic is deleted at 3.0.0 (D7 amended, D12).

### User Story 4 - opencode calls the binary (Priority: P2)

**Independent Test**: `opencode/guard/test/` runs the plugin's `evaluate` hook against a fake `ai-gent` and against the real binary for the adapter cases.

**Acceptance Scenarios**:

1. **Given** an opencode `shell` permission event, **When** the plugin evaluates, **Then** it runs `ai-gent guard --guard all --format opencode` with the resources on stdin and sets `event.effect` and `event.message` from the answer.
2. **Given** the binary is missing or crashes, **When** the plugin evaluates, **Then** it applies the failure policy (FR-014), never silently allowing.
3. **Given** 3.0.0, **Then** `opencode/guard/rules.mjs` and its parity test are deleted and `scripts/check.sh` runs the binary-backed test instead; from 2.28 until then the plugin calls the binary when it runs from a package install and uses `rules.mjs` otherwise.

### User Story 5 - A guard failure never silently opens the door (Priority: P1)

**Acceptance Scenarios**:

1. **Given** the hook runs and the binary is not at the expected path, **When** a Bash command arrives, **Then** the hook answers `ask` with the install command, for every guard.
2. **Given** the binary panics or exits non-zero without a decision, **When** the hook runs, **Then** the launcher prints the same `ask` with the reason "guard failed" and exits 0.
3. **Given** the binary runs longer than 2 seconds on one event, **When** the budget expires, **Then** the answer is `ask` with the reason "guard timed out".

### User Story 6 - Speed is measured, not assumed (Priority: P2)

**Acceptance Scenarios**:

1. **Given** `scripts/bench-guard.py`, **When** run on the owner's machine, **Then** it prints p50/p95/p99 over 100 runs for the shell baseline and the binary for five representative commands, and exits non-zero when the binary's p95 exceeds the budget in FR-012.
2. **Given** the baseline numbers above, **Then** the first measurement task records them in `quickstart.md` per OS before any porting.

## Requirements *(mandatory)*

- **FR-001**: One evaluator in `ai-gent-core::guard` exposes `evaluate(event: &Event, ctx: &Ctx) -> Decision` for guard ids `git`, `db`, `secrets`, `repo`, `containers`, and `evaluate_all` (strictest wins: deny over ask over none, first of that level in the order git, db, db credentials, secrets, repo, containers, as `evaluate()` in `rules.mjs`).
- **FR-002**: `Event` is the Claude Code hook JSON (`hook_event_name`, `tool_name`, `cwd`, `tool_input`); `Decision` is `{decision: deny|ask|none, reason}`; the printed hook JSON is equal after JSON parsing (key order and spacing may differ; decision and reason strings are identical).
- **FR-003**: Shell syntaxes are `Sh`, `PowerShell`, `Cmd`. Selection: `--shell` flag, else `tool_name` (`Bash` is `Sh`, `PowerShell` is `PowerShell`), else a per-OS default (`Sh` on Unix and under Git Bash, `Cmd` on Windows without bash) [ASSUMPTION: the PowerShell tool's name and event shape on Windows are unverified; T040 verifies]. Nested invocations (`cmd /c "..."`, `powershell -Command "..."`, `pwsh -c`, `bash -c '...'`, `sh -c`) are **not** unwrapped in the parity phase (the shell guards do not); unwrapping is a flagged enhancement (FR-016).
- **FR-004**: Segmentation reproduces the shell guards' pipeline per syntax: blank quoted strings by that syntax's quoting rules (sh: `"..."` with `\` escapes and `'...'`; PowerShell: `'...'` with `''` as an escaped quote, `"..."` with a backtick escape and `""`; cmd: `"..."` only, with `^` escaping outside quotes) [RESEARCHED: S5 in `specs/032-rust-workspace-and-release/research.md`], turn the separators into segment breaks (sh and PowerShell: `; & | ( )` and newline; PowerShell also treats a trailing backtick as continuation; cmd: `& && || |` and newline, `^` escapes a separator), then match per segment. The containers guard additionally splits on a backtick in `sh` only.
- **FR-005**: Fixtures live in `shared/guard-fixtures/` (format in `contracts/fixture-format.md`), are the single source of vectors for the Rust tests, `scripts/test-*guard.sh` and the opencode test, and are not packaged.
- **FR-006**: Parity gate (D10): all extracted vectors pass for `Sh` with equal decision **and** reason; every sh case with `variants` passes for `PowerShell` and `Cmd` with the same decision; Windows-only cases are added for path forms (drive-letter paths, `%APPDATA%`, `$env:XDG_CONFIG_HOME`, `Get-Content`, `type`) for the db credentials check.
- **FR-007**: Branch resolution for the git guard: `git -C <dir> symbolic-ref --quiet --short HEAD` executed only when a commit or merge segment needs it (as the shell guard), `-C <dir>` taken from the segment, `cwd` otherwise, a timeout of 1 second, an empty result meaning "unknown, no branch decision".
- **FR-008**: The `ai-gent guard` CLI contract is `contracts/cli.md`: `guard --guard <id|all> [--shell sh|powershell|cmd] [--format claude|opencode] [--shadow ...]`, event on stdin, exit 0 always for the `claude` format.
- **FR-009**: Shadow mode (D12): while `ai-gent guard --shadow` is wired, the shell guard decides; after printing its decision the hook runs the evaluator detached and appends one record (format `contracts/shadow-log.md`) to `${XDG_STATE_HOME:-~/.local/state}/ai-gent/guard-shadow.jsonl` (Windows: `%LOCALAPPDATA%\ai-gent\state\`). The record holds no command text unless `AI_GENT_SHADOW_CAPTURE=1`, and then only after secret redaction with `shared/secret-patterns.json`, file mode 0600.
- **FR-010**: Promotion criteria (all must hold): (1) at least one minor release (2.27) shipped with shadow on; (2) zero records with `agree=false` and `expected_divergence=false` over at least 500 evaluations per guard across the owner's machines plus the corpus replay (T031); (3) the full fixture suite green on Linux, macOS and Windows in CI; (4) the latency budget met on the three OSes; (5) no open task in `tasks.md` Phases 1-3. Any disagreement resets the clock after the cause is fixed and a fixture added.
- **FR-011**: Hooks call the binary from 2.28 for package installs only (D7 amended): `plugins/<p>/hooks/guard.sh` gains a launcher head of about 10 lines: when `${CLAUDE_PLUGIN_ROOT}/../../VERSION` exists (a package install) it finds `${CLAUDE_PLUGIN_ROOT}/../../bin/ai-gent`, runs `guard --guard <id>` and translates failure into an `ask`; otherwise (a git clone) it falls through to the existing shell logic, unchanged. `hooks.json` and the Claude Code hook protocol do not change. At 3.0.0 the shell logic is deleted with git mode and `guard.sh` is only the launcher.
- **FR-012**: Latency budget (p95 of 100 consecutive runs, process start included, warm file cache): Linux and macOS at most baseline + 5 ms and at most 15 ms absolute; Windows at most 60 ms [ASSUMPTION: Windows process creation cost, to be measured in T003]. Shadow mode adds at most one extra detached process after the decision and must not delay the hook's exit.
- **FR-013**: The reason strings live in `rust/crates/ai-gent-core/src/guard/reasons.json` (id, guard, marker, text, decision), read by the Rust code and by `scripts/consistency.py` (replacing its `guard.sh` scrape) so `shared/guard-ux.ts` entries stay checked.
- **FR-014**: Failure policy per guard (package installs) when the binary is missing, not executable, crashes (non-zero without a decision, or a panic caught by `catch_unwind`), or times out: **ask** for every guard, with the reason `<marker> the ai-gent binary is unavailable (<why>); run: <install command>. Allow only if you accept this command unguarded.` Reasoning in `plan.md`. A deny is never produced by a failure (it would brick a session and the user cannot tell why); an allow is never produced by a failure (it would silently remove protection, and a crash is exactly what a malformed input could cause).
- **FR-015**: The evaluator never prints, logs or stores a secret value; the shadow log redacts; `ai-gent guard` has no network access.
- **FR-016**: Enhancements beyond parity (nested-shell unwrapping, here-strings, `Invoke-Expression`, encoded commands) are separate fixtures tagged `new`, with `expected_divergence: true` against the oracle, and ship only after the cutover, each named in the release notes.

### Key entities

See `data-model.md`: Event, Decision, Segment, Fixture case, Shadow record, Reason entry.

## Edge cases

- Multi-line commands with heredocs (the secrets, git and db suites contain them): newline is a separator; the heredoc body is part of the following segments, as in the shell guards.
- A literal backslash-n inside blanked quotes: the shell guard's `gsub(/\\n/, "\n")` runs on the JSON-escaped text; the evaluator on decoded text splits on real newlines only. Fixtures decide equivalence; the extraction task flags any vector where the two views differ.
- Non-UTF-8 bytes: the event is JSON, so strings are UTF-8; invalid input to `guard` is a failure (FR-014).
- Huge commands: evaluation is linear (the `regex` crate has no backtracking) [KNOWN]; a size cap of 256 KiB asks with "command too large to inspect".
- `cwd` missing or not a directory: no branch decision.
- Windows paths with spaces and drive letters in `-C` arguments.
- Concurrent shadow writers: appends use `O_APPEND` with one write per line.

## Out of scope

- Strengthening the guards (FR-016 lists candidates).
- The mod-side guard UX (`guard-ux.ts`); it keeps working unchanged because reasons are identical.
- `leftovers.sh` (a SessionStart report, not a guard); it stays shell until a later decision.
- A daemon; every evaluation is one short process.

## Success criteria *(mandatory)*

- **SC-001**: 100 percent of extracted vectors pass for `Sh`, decision and reason (`cargo test` and the five `scripts/test-*guard.sh`).
- **SC-002**: For every sh case with `variants`, PowerShell and cmd decisions equal the sh decision; Windows-only cases pass on the `windows-2025` and `windows-11-arm` legs.
- **SC-003**: The shadow log of the promotion window has zero unexplained disagreements and the evaluation counts of FR-010.
- **SC-004**: `scripts/bench-guard.py` meets FR-012 on Linux, macOS and Windows.
- **SC-005**: After 3.0.0 `grep -rn 'decide deny' plugins/*/hooks/guard.sh` finds nothing and `opencode/guard/rules.mjs` does not exist. In 2.28 a clone's `guard.sh` still decides and the fixture suite passes for both paths.
- **SC-006**: Renaming the binary makes every guard answer `ask` with the install command, tested for the five hooks and the opencode plugin.

## Open questions (for the owner)

1. **Failure policy**: ask for all five (recommended). Alternative: deny for `secrets` and the db credentials paths only, because a secret exposure is irreversible. Recommendation stays `ask` (a deny with no way forward bricks a Windows or CI session).
2. **Answered by D7 (amended)**: from 2.28 package installs are decided by the binary and clones keep the shell logic; the shell logic and `rules.mjs` go at 3.0.0. Remaining detail: a contributor in a clone who wants the binary runs `scripts/dev-bin.sh` (optional, not required).
3. **Promotion evidence is single-user.** ai-gent has no telemetry. Recommended: accept the owner's machines plus the corpus replay of the owner's Claude Code transcripts (counts and hashes only) as the evidence.
4. **PowerShell tool name and event shape on Windows**: unverified. Recommended: verify on a Windows machine with Claude Code before T040; until then PowerShell and cmd are only reachable through `--shell`.
5. **Nested shell unwrapping** (`bash -c "git commit --no-verify"` is invisible today). Recommended: ship in the first release after the cutover as `new` fixtures, announced in the changelog.
6. **Hook commands on native Windows (D24).** The launcher is POSIX sh; a native Windows install without Git Bash has no `sh` for hook commands, and a bare binary in `hooks.json` cannot turn a failure into an `ask` (a non-blocking hook error would fail open). Unverified how Claude Code runs hook commands on Windows. Recommended: T034 verifies; if hooks run through PowerShell or cmd, ship a `guard.cmd` launcher with the same contract (find the binary, run, translate failure to `ask`); if they need Git Bash, document it as a prerequisite for the guards on Windows.
