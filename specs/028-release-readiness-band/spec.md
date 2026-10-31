# Feature Specification: Release readiness band

**Feature Branch**: `docs/mod-specs-2` (specification only; built on its own branch)

**Created**: 2026-10-31

**Status**: Implemented

**Input**: A Claude Code mod that keeps the next release in view: a band above the prompt saying what `scripts/release.py --dry-run --json` would do now (`next: 2.23.0 (minor) · 3 plugins bump`), so the person sees, before promoting to `main`, whether the commits since the last tag release anything. It extends 017's `/release-preview` by showing the same plan without a command. Grounded in `scripts/release.py` and `shared/release-single.py` (their plan and exit codes; both gain `--json`, Planned) and `plugins/git/hooks/hygiene.ts` (`releasePreview`).

**Depends on**: 012 (checkpoint mod) — module layout; 017 (repo hygiene) — the git module, `releasePreview`, the `AbovePrompt` composition and the ref-moving refresh. Implemented.

## Context and ground rules

- A mod is an optional, Claude Code-only layer. `scripts/release.py` and `.github/workflows/release.yml` stay the only things that release; the band runs the script with `--dry-run --json` only (`releasePreview` runs it with `--dry-run`) and never without `--dry-run`. The new `--json` flag is an error without `--dry-run` (`parser.error`, exit 2), so it cannot release. It never fetches, pulls, commits, tags or pushes, and does not decide whether to promote: that is `git:workflow`, asked first.
- It reads only what the script prints as JSON (the shape is in `data-model.md`) and exit code 3 for "no release needed". The mod computes no version or bump of its own and parses no prose.
- Owner: the `git` plugin; code beside `hygiene.ts`, composed from `register.tsx`. It applies only where `scripts/release.py` exists (this repository, and projects set up from `shared/release-single.py`), and only when HEAD is a chain branch (`CHAIN` in `hygiene-model.ts`: `main`, `master`, `dev`, `develop`; the maintainer's wording is "dev or main").
- The band is one of the bands above the prompt and obeys the band-priority rule (at most two visible, holds and failures first; specified separately, Planned). Its priority is `info`, the lowest: it yields to a checkpoint hold (012), a failing CI run (023) and 017's unpulled-release band.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - See what the next release would be (Priority: P1)

When commits since the last tag are releasable, a band reads `next release 2.23.0 (minor, since 2.22.0) · 3 plugins bump — /release-preview for the plan`. When nothing is releasable, no band is drawn.

**Why this priority**: it answers "would promoting now release anything, and what" without a command, and shows when a docs-only run of commits will not (CLAUDE.md "Releases").

**Independent Test**: a `claude plugin test` case mocks `$.process.run` to answer `release.py --dry-run` with a recorded plan on stdout (exit 0) and mounts the band on `terminal` and `desktop`.

**Acceptance Scenarios**:

1. **Given** the dry run exits 0 with JSON `{version: "2.23.0", bump: "minor", since: "2.22.0", targets: [three of kind "plugin"]}`, **When** the band draws, **Then** it shows the version, the bump kind, the base tag and the plugin count.
2. **Given** exit code 3, **When** drawn, **Then** no band (nothing releasable).
3. **Given** exit code 1 or 2, a start failure, or stdout that is not the expected JSON, **When** drawn, **Then** a single dim line says the preview failed, with the script's first stderr line (the script prints `release.py: error: …` there), and does not retry until HEAD changes.
4. **Given** no `scripts/release.py`, **When** the session starts, **Then** nothing runs and nothing shows.
5. **Given** HEAD is a feature branch (not in `CHAIN`), **When** the session starts or a turn completes, **Then** the script is not run and no band is drawn (the dry run reads HEAD's history, so a feature branch's plan is not the next release).
6. **Given** `targets` holds no `plugin` entry (a project set up from `shared/release-single.py`: one `source` target), **When** the band draws, **Then** it omits the plugin count (`next release 1.4.0 (minor, since 1.3.2)`).

---

### User Story 2 - It stays current, cheaply (Priority: P1)

The dry run runs again only when HEAD or the tags merged into it changed, never on a timer.

**Independent Test**: advance the mocked clock and raise turn completions; assert the script ran once per distinct HEAD.

**Acceptance Scenarios**:

1. **Given** the band shown for HEAD `abc` and tags `T`, **When** a turn completes with HEAD still `abc` and the same tags, **Then** the script is not run again.
2. **Given** a Bash `git commit`, `git merge`, `git pull`, `git checkout`/`switch` or `git tag` completes (017's `MOVES_REFS` plus `tag`), **When** it finishes, **Then** HEAD and the merged tag list are read again and the script runs only if either changed.
3. **Given** a new tag from a release pulled in, **When** HEAD moved to it, **Then** the band updates (usually to none).

---

### User Story 3 - Open the full plan, and turn it off (Priority: P3)

The band's text names `/release-preview`, which prints the full dry run (the changelog section included). `releaseBand` (boolean, default true) turns the band off; `/release-preview` stays governed by 017's `releasePreview`.

**Acceptance Scenarios**:

1. **Given** `releaseBand` off, **When** commits change, **Then** the module runs nothing for it, and `/release-preview` still works.
2. **Given** the user presses Hide, **When** the prompt redraws, **Then** the band stays hidden until HEAD changes.

### Edge Cases

- **Hygiene band also shown** (unpulled release): both compose; when `main`/`dev` are behind their remote the preview is computed on stale history, so the band adds "(local branch is behind origin)" and never suggests a pull itself.
- **No tags yet**: the JSON's `since` is `null`; the band says "first release". The mod does not pass `--initial` (the script's default 0.1.0 applies, as in `releasePreview`).
- **Cost**: `release.py` runs several git commands; one run per distinct HEAD, with a 60 s timeout as `releasePreview` uses.
- **Working tree dirty or mid-rebase**: the script reads commits, not the tree; the band is unaffected, and a git failure shows the dim line.
- **Narrow terminal**: below `bodyColumns` 60, the band reads `next 2.23.0 (minor)`.
- **Hot reload**: the parsed plan, the key it was computed for (HEAD plus tags) and the hidden flag live in `$.state` atoms (`data-model.md`).
- **Script without `--json`** (an older `release-single.py` copy in another project): the run exits 2 (unknown argument); the band shows the dim failure line once and stops, and says to re-run `devsecops:pipeline` to refresh the script.
- **Release job running** (a push to `main` just made): until the bot commit lands and is pulled, the band may still show the pending version; 017's band is the one that says to pull.
- **Headless (`claude -p`)**: nothing draws; `/release-preview` still answers as text.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The mod MUST run `scripts/release.py` only with `--dry-run --json`, MUST NOT run it any other way, and MUST NOT fetch, pull, commit, tag or push.
- **FR-002**: The band's figures MUST come from the script's JSON (`data-model.md`) and exit code (3 = no release). `scripts/release.py` and `shared/release-single.py` MUST gain `--json` together as one prior change, with the same keys; `scripts/consistency.py` fails when their keys differ or when `release-model.ts`'s key list differs from them, and both release test scripts cover the flag.
- **FR-003**: The script MUST run at most once per distinct key (HEAD plus `git tag --list --merged HEAD`), only when HEAD is a chain branch, detached from `session.start`, with the 60 s timeout `releasePreview` uses.
- **FR-004**: The band MUST compose with the bands beneath (`AbovePrompt`), as 012 and 017 do, and MUST NOT draw while the model is working (`isWorking`) or a survey is open, as 017's band does.
- **FR-005**: `/release-preview`'s behavior (017) MUST be unchanged; the band reuses its runner (`releasePreview`'s script call) instead of a second one.
- **FR-006**: `releaseBand` MUST turn the band off without touching 017's band, toasts or `/release-preview`.
- **FR-007**: The band MUST declare its priority (`info`) to the band-priority rule and MUST NOT draw when two higher-priority bands are visible.
- **FR-008**: The plugin count MUST count only `targets` of kind `plugin`; a plan with none omits the count, and the `CHANGELOG.md` line is never a target.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: `claude plugin test plugins/git` passes, with stories 1–3 on `terminal` and `desktop`.
- **SC-002**: On this repository with releasable commits on `dev`, the band's version equals the first line of `scripts/release.py --dry-run` and `--dry-run --json`'s `version`; with only `docs:` commits, no band.
- **SC-003**: Over a 10-minute session with no new commits, the script runs at most once.
- **SC-004**: `claude plugin validate plugins/git` reports no refusal; `scripts/consistency.py` still passes with the added model file.

## Out of scope

- Releasing, tagging, promoting or opening the release PR (`git:workflow`, `repo:pr`; asked first).
- Changing the bump rules or the text plan of `scripts/release.py` and `shared/release-single.py`. Adding `--json` to both is in scope, as a prior change (plan task 1).
- Container image retagging and platform release objects (`shared/release-platforms.md`).
- Previewing on a remote or in CI.
- Other agents (opencode, Codex, …).

## Assumptions

- [ASSUMPTION: `scripts/release.py --dry-run` exits 3 when nothing is releasable (`NO_RELEASE`, `scripts/release.py:55`) and prints the plan to stdout; `--json` keeps both.]
- [ASSUMPTION: `release.py`'s last tag (`git tag --list --merged HEAD`, highest by version, `last_tag`, :90-92) is what the plan's `since` reports; the mod does not read the tag itself.]
- [ASSUMPTION: Claude Code 2.1.312's `AbovePrompt` props (`isWorking`, `hasSurvey`, `bodyColumns`) stay as 017 reads them.]

## Decisions (2026-10-31, with the maintainer)

- `release.py` and `shared/release-single.py` gain `--json` together, as one change before the mod (the bump rules live in both). The band reads the JSON, not the plan's text.
- Shown only when HEAD is a chain branch; the maintainer said `dev` or `main`, and the mod uses `CHAIN` (adds `master` and `develop`, the names 017 already treats as the chain).
- Key is HEAD plus the merged tag list, not `git describe` (review 2026-10-31: `release.py`'s `last_tag` takes the highest merged version, which `describe` can disagree with).
- Plugin count: only `kind: "plugin"` targets; `release-single.py` plans have one `source` target and no count. The CHANGELOG line is not a target.
- Priority `info`, lowest of the bands (decided 2026-10-31 (the maintainer accepted the default): the band-priority rule is specified in `specs/030-band-priority`).
- Engine pin: Claude Code 2.1.312's `AbovePrompt` props are as 017 reads them; nothing here needs a newer API.
- Verified 2026-10-31 by running it: `scripts/release.py --dry-run` exits 3 with `no release needed: no feat, fix, perf, refactor or breaking commits since 2.22.0` when nothing is releasable.
