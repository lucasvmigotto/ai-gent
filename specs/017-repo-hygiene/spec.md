# Feature Specification: Repo hygiene band, session-start toasts, release preview

**Feature Branch**: `docs/mod-specs` (specification only; built on its own branch)

**Created**: 2026-10-28

**Status**: Implemented

**Input**: Three small Claude Code mod pieces that surface what the repository's rules and scripts already say: (1) a band above the prompt when local `dev`/`main` lag their remote-tracking branches, or a bot release commit hasn't been pulled (CLAUDE.md "Releases": "run `git pull --ff-only` before the next local commit"; `plugins/git/skills/workflow/SKILL.md`, "after a release, `git pull --ff-only` on both"); (2) toasts for what the `SessionStart` hooks `scripts/update-check.sh` and `plugins/devcontainer/hooks/leftovers.sh` print, which today only the model reads; (3) a `/release-preview` command running `scripts/release.py --dry-run`.

**Depends on**: 012 (checkpoint mod) — module layout, `check.sh` wiring, versioning and install, reused unchanged.

## Context and ground rules

- Read-only by default: nothing here fetches, pulls, commits or tags. `git:workflow` and its guard stay the authority on what may run; the band only says what the workflow already requires.
- Owners: `git` (the band, the update toast and `/release-preview`; `update-check.sh` is wired from `plugins/git/hooks/hooks.json`) and `devcontainer` (the leftovers toast; `leftovers.sh` is its `SessionStart` hook). Both modules are shared with 015's parts for those plugins.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - See that a release commit needs pulling (Priority: P1)

After a push to `main`, the release job lands `chore(release): X.Y.Z` on `origin/main` and merges it back into `origin/dev` (`chore(release): merge X.Y.Z back into dev`). Once the remote-tracking refs know it, a band says so until the local branches have it.

**Why this priority**: committing on a stale `dev` is the mistake CLAUDE.md warns about; it forces a merge of the bot's commit later.

**Independent Test**: a `claude plugin test` case with `$.process.run` mocked to answer the git commands (`rev-list --left-right --count`, `log --format=%s`) mounts the band on `terminal` and `desktop` and finds its text by key.

**Acceptance Scenarios**:

1. **Given** `main..origin/main` holds `chore(release): 2.20.0` and local `main` has no commits of its own past `origin/main`, **When** the band draws, **Then** it reads "main is 1 behind origin/main (release 2.20.0) — `git pull --ff-only` on main before the next commit".
2. **Given** `dev` is behind `origin/dev` by the merge-back commit and also by others' work, **When** drawn, **Then** it names the count and the release, and the same command.
3. **Given** a branch both ahead and behind its remote (diverged), **When** drawn, **Then** it says "diverged" and points to `git:workflow` instead of suggesting a pull.
4. **Given** both branches level with their remotes, **When** refreshed, **Then** the band draws nothing.
5. **Given** the remote-tracking refs were last updated more than a day ago (`FETCH_HEAD`'s `mtimeMs`), **When** drawn, **Then** the band adds "as of the last fetch, N days ago" (it never fetches on its own).

---

### User Story 2 - See what the session-start hooks reported (Priority: P2)

`update-check.sh` prints `[ai-gent] update available: …` and `leftovers.sh` prints `ai-gent containers (report only, nothing removed):` plus lines. Both go to the model as context; the person never sees them. The mods toast them once per session.

**Independent Test**: a test appends the stored hook row (`$.session.append`, door `hook-context`, origin event `SessionStart`) holding those lines; assert one toast each, and none for any other row.

**Acceptance Scenarios**:

1. **Given** the update line in the SessionStart context, **When** the session starts, **Then** a toast shows it (the `git` plugin's module).
2. **Given** the leftovers block with two lines, **When** the session starts, **Then** one toast lists them (the `devcontainer` plugin's module).
3. **Given** neither script printed anything, **When** the session starts, **Then** no toast.
4. **Given** `AI_GENT_NO_UPDATE_CHECK` or `AI_GENT_NO_LEFTOVERS_CHECK=1`, **When** the session starts, **Then** the scripts are silent and so are the mods (the mods add no check of their own).

---

### User Story 3 - Preview the next release (Priority: P2)

`/release-preview` runs `python3 scripts/release.py --dry-run` at the repository root and prints the plan as the command's output (a transcript line, not a model turn).

**Acceptance Scenarios**:

1. **Given** releasable commits since the last tag, **When** run, **Then** the output is the script's stdout (version, plugin bumps, changelog section).
2. **Given** exit code 3, **When** run, **Then** the output is "no release needed (nothing releasable since the last tag)".
3. **Given** no `scripts/release.py` in the repository, **When** run, **Then** it says so and names `devsecops:pipeline`, which sets one up from `shared/release-single.py`.
4. **Given** exit 1 or 2, **When** run, **Then** the script's stderr is shown as the error.

### Edge Cases

- **No git / not a checkout**: the band and `/release-preview` say nothing and "not a git repository" respectively.
- **No remote, or no `origin/dev`**: only the branches that have a remote-tracking ref are compared; `master`/`develop` count as the chain's names, as `wt.py` and `repo_state.py` accept them.
- **Inside a worktree** (`.worktrees/<slug>` or `.claude/worktrees/…`): local `dev`/`main` are the main checkout's refs, shared by every worktree; the band is the same in each.
- **Detached HEAD / pinned checkout**: the band still compares the branches; `update-check.sh` already stays silent on a pinned tag.
- **Refresh cost**: on `session.start`, after a Bash tool call whose command starts a `git` subcommand that moves refs (`fetch`, `pull`, `merge`, `rebase`, `commit`, `checkout`, `switch`, `reset`), and on `turn.complete` at most once a minute. Never on a timer while idle.
- **Narrow terminal**: the band is one line sized to `e.props.bodyColumns`; under 50 columns it shortens to "main ↓1 · dev ↓2 — pull --ff-only".
- **Non-terminal surfaces**: `AbovePrompt` is drawn on each surface that has it; toasts everywhere; `/release-preview` prints text.
- **While a model turn runs** (`isWorking`) or a survey holds the band (`hasSurvey`): the band yields (draws nothing).
- **Hot reload**: the last comparison and "toasted this session" flags live in `$.state`; a reload doesn't repeat toasts.
- **Headless (`claude -p`)**: no band or toast; `/release-preview` still answers text.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The band MUST compare only local refs against remote-tracking refs already present, through `$.process.run` git calls, and MUST NOT fetch, pull or change refs.
- **FR-002**: A release commit MUST be recognized by subject only: `^chore\(release\): \d+\.\d+\.\d+$` on `main`/`master`, `^chore\(release\): merge \d+\.\d+\.\d+ back into (dev|develop)$` on the dev branch — the subjects `.github/workflows/release.yml` writes (`chore(release): $version`, `chore(release): merge $version back into $integ`) and `shared/release-platforms.md` prescribes for other projects.
- **FR-003**: The band MUST suggest `git pull --ff-only` only when the local branch has no commits of its own past its remote; otherwise "diverged".
- **FR-004**: Toasts MUST relay the scripts' own lines, never compute an update or leftovers check of their own, and fire once per session.
- **FR-005**: Reading the SessionStart output MUST use the transcript row the engine stores for the hook (`session.append`, door `hook-context`, origin `{ kind: 'hook', event: 'SessionStart' }`, text `SessionStart:startup hook success: <stdout>`). `classic.SessionStart`'s `next(e)` returns `{}` for it, and the scripts MUST NOT be run a second time.
- **FR-006**: `/release-preview` MUST run `python3 scripts/release.py --dry-run` with the repository root as `cwd` and MUST NOT run it without `--dry-run`.
- **FR-007**: `userConfig` booleans: `hygieneBand`, `sessionToasts`, `releasePreview` on `git`; `leftoversToast` on `devcontainer`; all default true.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: `claude plugin test plugins/git` and `plugins/devcontainer` pass, stories 1–3 on `terminal` and `desktop`.
- **SC-002**: `scripts/test-update-check.sh`, `scripts/test-containers-guard.sh` (which covers `leftovers.sh`) and `scripts/test-release.sh` pass unchanged.
- **SC-003**: On this repository right after a release (local `dev` behind `origin/dev` by the merge-back commit), the band appears within one refresh and disappears after `git pull --ff-only`.

## Out of scope

- Fetching (decided: never), pulling or fixing anything.
- Branch protection, CI status (`devsecops:watch`), PRs.
- Changing `update-check.sh`, `leftovers.sh` or `release.py`.

## Decisions (2026-10-28, with the maintainer)

- The band never fetches; it reads what the last fetch left and says how old `FETCH_HEAD` is.
- The command is `/release-preview`: `$.command.register` declares a plain `/<name>` with no plugin prefix (Claude Code 2.1.312 types).

- Found while building (verified on 2.1.312 with a probe plugin and a headless run): `classic.SessionStart`'s `next(e)` returned `{}` for a command hook's stdout, so FR-005's first route does not work. The hook's output is a stored transcript row that `session.append` sees, which relays the script's own lines with no second run. The `session.append` hooks carry a `.catch` that replays `next(e)`, so a bug here can never drop a row.
- Found while building: two bands from different plugins hide each other unless each composes with what is beneath it. The checkpoint band (012) now draws above `await next(e)`, and the hygiene band does the same.
- Found while building: `leftoversBlock`, `hookLines` and `rowText` are used by both the `git` and `devcontainer` plugins, so they are `shared/hook-lines.ts`, copied into both (the engine refuses a symlinked import; `scripts/consistency.py --sync-shared-code`).
- `scripts/consistency.py` also checks that the band's two subject patterns still match every release subject in `.github/workflows/release.yml` and `shared/release-platforms.md`.

## Assumptions

- [ASSUMPTION: The release job keeps the commit subjects `plugins/git/skills/workflow/SKILL.md` documents.]
