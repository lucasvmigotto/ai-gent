# Feature Specification: CI watch band

**Feature Branch**: `docs/mod-specs-2` (specification only; built on its own branch)

**Created**: 2026-10-31

**Status**: Accepted

**Input**: A Claude Code mod that, after a push, keeps the state of that commit's CI runs in a band above the prompt (`release ✓ · docs-ci ⏳`), with priority above the informational bands and raises a toast when one fails, so the person sees a red run without asking. The band obeys the band-priority rule (at most two bands visible above the prompt, holds and failures first; specified separately, Planned): a failing run is a `failure`-priority band, running or green runs are `normal`. Grounded in `plugins/devsecops/skills/watch/SKILL.md` ("find the right run", "by commit, not the latest") and `plugins/repo/scripts/reporun.py` (`detect`, `cli`).

**Depends on**: 012 (checkpoint mod) — module layout, `check.sh` wiring, versioning and install; 017 (repo hygiene) — the `AbovePrompt` composition and the "ref-moving command refreshes the band" pattern (those hooks live in the `git` and `project` modules; `plugins/repo/hooks/` has no `AbovePrompt` hook yet, so 023 adds the first). The module is `repo`'s, shared with 015's guard-ux part, which today owns `register.tsx` (it returns early when `guardUx` is false, `register.tsx:12`; 023 restructures that, FR-007).

**First cut**: GitHub only, through `reporun`. GitLab, a `/ci-status` text command and other platforms follow once the GitHub path is proven.

## Context and ground rules

- A mod is an optional, Claude Code-only layer. `devsecops:watch` stays the skill that follows runs, reads failed logs and classifies causes; the band shows run state per commit and a pointer to that skill. It never reads logs, never re-runs, cancels or dispatches anything, and never pushes.
- Read-only, and only when authenticated: the mod runs reads through `reporun cli github -- …` (no `--write`, so the repo guard's classification is read), and only when `reporun detect --json` says `gh` is installed and signed in. **`reporun cli` is not decrypt-free**: `prepare_cli` (`reporun.py:345-347`) reads the pass entry whenever `pass_has(entry)` is true, so a pinentry prompt can appear mid-session if gpg-agent has not cached the key. `detect` itself only checks that the entry exists (`token_in_pass`). The mod therefore gates on `token_in_pass`: when true, it runs only if the person turned on `ciBandUsePass` (default off); when false, `reporun cli` uses the CLI's own login and never decrypts.
- Secrets are never read or shown: no secret listing, no variable values, no log lines. `shared/secrets.md` stands.
- Owner: the `repo` plugin. Code in `plugins/repo/hooks/`, beside `register.tsx` and `guard-ux.ts`.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - See what CI did with my push (Priority: P1)

After a successful `git push`, a band reads `CI for 3a9f1c2 on dev: docs-ci ⏳ · check ✓` and updates as the runs finish, until they all complete.

**Why this priority**: today a red run is found when someone remembers to look, or when `project:status` runs.

**Independent Test**: a `claude plugin test` case mocks `$.process.run` to answer `git rev-parse HEAD`, `reporun.py detect --json` (GitHub, `signed_in` true, `token_in_pass` false) and `reporun.py cli github -- run list --commit <sha> --json …`, raises a successful Bash `git push`, and asserts the band text on `terminal` and `desktop`.

**Acceptance Scenarios**:

1. **Given** a Bash call `git push` that exits 0 on a GitHub remote with `gh` signed in, **When** the call completes, **Then** the band shows the pushed commit's short SHA, the branch and one entry per run from `run list --commit <sha>`, each with a state mark.
2. **Given** runs still queued or in progress, **When** the poll interval elapses, **Then** the band updates; **Given** all runs completed, **Then** the band shows the result for a short time and then clears.
3. **Given** the push ran nothing (a bot commit, a path filter), **When** the first read returns no runs, **Then** the band says `no runs started for <sha> (paths, branches or a bot commit?)` and clears after the first completed check, as `devsecops:watch` teaches that "no run" is not "broken".
4. **Given** the push failed or was a dry run (`--dry-run`), **When** the call completes, **Then** nothing happens.
5. **Given** the user hides the band, **When** the prompt redraws, **Then** it stays hidden for that push.

---

### User Story 2 - Told when a run fails (Priority: P1)

When a run of the pushed commit concludes `failure`, a toast says which workflow and points to the skill: `check failed on 3a9f1c2 — ask /devsecops:watch why`.

**Independent Test**: the mocked `run list` answers one run `completed/failure`; assert one toast naming the workflow and the skill, and none on the next poll.

**Acceptance Scenarios**:

1. **Given** a run concluding `failure`, `timed_out` or `startup_failure`, **When** read, **Then** one toast (and the band entry marked ✗) appears, once per run id.
2. **Given** `cancelled` or `skipped`, **When** read, **Then** the band shows it neutrally and does not toast.
3. **Given** a failing run, **Then** the toast carries no log text, no job step names from logs, and no secret or variable names.

---

### User Story 3 - Quiet when it cannot know (Priority: P2)

Without a signed-in `gh`, an unsupported platform or no remote, the band never appears and the module spawns nothing after the first check.

**Acceptance Scenarios**:

1. **Given** `detect` reports `gh` not installed or not signed in, **When** a push completes, **Then** one toast says what `gh auth login` would add, once per session, and nothing else runs.
2. **Given** a GitLab, Azure DevOps, Bitbucket, Jenkins or unknown platform, **When** a push completes, **Then** nothing shows (first cut: GitHub only).
3. **Given** `ciBand` off, **When** a push completes, **Then** nothing runs.
4. **Given** `detect` reports `token_in_pass` true and `ciBandUsePass` off, **When** a push completes, **Then** nothing is run through `reporun cli` (it would decrypt the entry) and one dim line says why, once per session.
5. **Given** `reporun cli` prints a `gh` warning before the JSON (`cmd_cli` merges stderr into stdout, `reporun.py:383-384`), **When** the output is read, **Then** the JSON array is taken from the first line starting with `[` to the end; output with no parsable array counts as a failed read.

### Edge Cases

- **No polling loop in the conversation**: the poll is the module's own `$.clock.after` with growth (30 s, then 60 s, then 120 s) and a hard stop (default 15 minutes), never a model turn and never a blocking call.
- **Chained runs** (a release workflow that triggers a deploy): the band follows the pushed commit only; a chain beyond it is `devsecops:watch`'s, and the band's note says "more runs may follow".
- **Several pushes**: the newest replaces the band; earlier toasts stay once per run id.
- **Push to a branch with no remote CI** (no `.github/workflows/`): checked first through `$.fs.exists`; nothing is called.
- **Rate limits or network errors**: a failed read keeps the last band and backs off; after three failures it stops with one dim line.
- **Hot reload**: the pushed SHA, the runs seen, the toasted run ids and the timer's phase live in `$.state` atoms.
- **Headless (`claude -p`)**: nothing draws and nothing polls. There is no `/ci-status` in the first cut.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The mod MUST only read: `reporun.py cli github -- run list …` without `--write`, `reporun.py detect --json`, `git rev-parse`, `$.fs.exists`; it MUST NOT call `run view --log`, `run rerun`, `run cancel`, `workflow run`, or any listing of secrets or variables.
- **FR-002**: It MUST run the platform read only after `reporun.py detect --json` reports `gh` as installed and signed in, and MUST NOT call `reporun cli` while `token_in_pass` is true unless `ciBandUsePass` is on (the call would decrypt the entry and may raise a pinentry prompt). The mod never reads or passes a token itself.
- **FR-008**: The mod MUST read the run list defensively: take the JSON array from the first `[` line of the merged output, treat anything unparsable as a failed read, and use only these fields, all offered by `gh run list --json` (checked against `gh` on 2026-10-31): `databaseId,name,status,conclusion,workflowName,headSha`; `--commit` is a real `gh run list` flag.
- **FR-003**: Runs MUST be found by commit SHA (`--commit <sha>`), never "the latest" (`devsecops:watch`).
- **FR-004**: Polling MUST be bounded by a back-off and a maximum duration (`ciBandMinutes`, default 15) and MUST stop when every run is completed, the push is superseded, or the band is hidden.
- **FR-005**: A toast MUST fire once per failing run id, MUST NOT include log text or secret/variable names, and MUST name `/devsecops:watch`.
- **FR-006**: The band MUST compose with the bands beneath (`AbovePrompt`: the checkpoint and hygiene bands), as 012 and 017 do, and MUST declare its priority (`failure` while a run failed, `normal` otherwise) to the band-priority rule; it MUST NOT draw when two higher-priority bands are visible.
- **FR-007**: `ciBand` (boolean, default true), `ciBandPollSeconds` (number, default 30), `ciBandMinutes` (number, default 15) and `ciBandUsePass` (boolean, default false) MUST be `userConfig` values in `/config`. `register.tsx` MUST gate each part on its own option: `guardUx` false must not disable the CI band, and `ciBand` false must not touch 015's guard-ux part (today `register.tsx:12` returns early on `guardUx === false`).

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: `claude plugin test plugins/repo` passes, with stories 1–3 on `terminal` and `desktop`.
- **SC-002**: After a real push to this repository, the band shows the `check` and `Docs CI` runs and clears within the configured limit; a deliberately failing branch raises one toast.
- **SC-003**: A test records every spawned command in a session with a push and asserts none is outside FR-001's list.
- **SC-004**: `claude plugin validate plugins/repo` reports no refusal.

## Out of scope

- Reading logs, classifying causes, re-running, cancelling (`devsecops:watch`, which acts only on request).
- Deploy or environment approvals, branch protection, required checks.
- GitLab, Azure Pipelines, Bitbucket, Jenkins and Forgejo in the first cut, and a `/ci-status` command.
- Pushes made outside the session (another terminal).
- Other agents (opencode, Codex, …).

## Assumptions

- [ASSUMPTION: a mod's `$.process.run` spawns are the engine's `process.run` event, not Bash tool calls, so `plugins/repo/hooks/guard.sh` (a `PreToolUse` guard on Bash) does not see them; the calls are reads regardless. Confirmed from the types of Claude Code 2.1.316 only; a test runs the band with the guards installed.]
- [ASSUMPTION: `reporun.py detect --json` keeps `platforms.<p>.token_in_pass` and `.cli.installed` / `.cli.signed_in` (`cmd_detect`, `cli_state`, `reporun.py:229-253`).]
- [ASSUMPTION: Claude Code 2.1.312's `AbovePrompt`, `$.clock.after` and `$.ui.toast` stay as declared.]

## Decisions (2026-10-31, with the maintainer)

- First cut is GitHub only, through reporun, with no `/ci-status`. GitLab and a text command follow once the GitHub path is proven. The Bash-guard note in 022 applies here too.
- Whether `reporun detect --json` is cheap enough per push (it runs `pass has` and a `gh auth status`, timeout 20 s): cache its answer in an atom for the session and refresh on `session.start` or a failed read (decided 2026-10-31 (the maintainer accepted the default)).
- `ciBandUsePass` defaults to off, so by default the band runs only when `reporun cli` would use the CLI's own login. Since `shared/secrets.md` makes pass the normal place for a token, many users will see the dim "needs the CLI's login" line until they turn it on, accepting a possible pinentry prompt (decided 2026-10-31 (the maintainer accepted the default): off).
- GitHub only and no `/ci-status` in the first cut (maintainer, 2026-10-31); GitLab joins after `glab ci list --sha` is checked against `reporun cli gitlab`'s classification.
- Priority: `failure` while a run failed, `normal` otherwise; the band-priority rule itself is specified separately (maintainer's two-band cap, 2026-10-31).
- Verified 2026-10-31 by running it: `reporun.py detect --json` reports, for this repository, `platforms.github.token_in_pass: false` and `cli: {name: gh, installed: true, signed_in: true}`, so with `ciBandUsePass` off the band works here with gh's own sign-in.
