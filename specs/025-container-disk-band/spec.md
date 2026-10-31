# Feature Specification: Container and disk band

**Feature Branch**: `docs/mod-specs-2` (specification only; built on its own branch)

**Created**: 2026-10-31

**Status**: Implemented

**Input**: A Claude Code mod that keeps this project's container leftovers and a low disk in view all session, not only as the toast at its start: a band above the prompt fed by what `plugins/devcontainer/hooks/leftovers.sh` already reports, now also as JSON (`leftovers.sh --json`, a separate change landed first): a disk under the floor, dangling images of this project, volumes of worktrees that are gone, buildah working containers in the store; refreshed after container work, plus `/containers` printing the full report. Report only: nothing here removes anything.

**Depends on**: 017 (repo hygiene) — the `devcontainer` module, its `leftovers.sh` relay and `hook-lines.ts`; the prerequisite change `leftovers.sh --json` (below); 011 (worktrees) — `wt.py resources`, which names the commands the report points at; 012 (checkpoint mod) — layout, `check.sh` wiring, versioning and install.

## Context and ground rules

- **What exists.** `leftovers.sh` is the plugin's `SessionStart` hook (`plugins/devcontainer/hooks/hooks.json`). It reads the engine (`CONTAINER_ENGINE`, else podman, else docker), computes the project label `me.lucasvmigotto.ai-gent.project=<project>` from the repository's name, and prints `ai-gent containers (report only, nothing removed):` followed by up to four lines: `disk: N GB free on the container store (<path>), under the F GB floor; …`, `N dangling image(s) of <project>: …`, `N volume(s) of <project> belong to worktrees that are gone: …`, `N buildah working container(s) in the store (interrupted builds, any project): …`. It is silent when there is nothing to say, no engine, or not a git checkout, and `AI_GENT_NO_LEFTOVERS_CHECK=1` disables it. Spec 017 made the report a toast, once per session (`plugins/devcontainer/hooks/leftovers.ts`). The order of its lines, which the band keeps: disk, dangling images, volumes, buildah.
- **Prerequisite change (Planned, lands before this mod): `leftovers.sh --json`.** Default output is untouched (the SessionStart hook and 017's toast keep reading the same text, silent when clean). With `--json` the script always prints one JSON object when it ran: `{"checked": true, "project": "shop", "findings": [{"kind": "disk", "free_gb": 14, "floor_gb": 20, "store": "/path"}, {"kind": "dangling", "count": 2}, {"kind": "volumes", "count": 1}, {"kind": "buildah", "count": 21}]}` (an empty `findings` array when clean), and `{"checked": false, "reason": "no-engine" | "not-a-checkout" | "disabled"}` when it could not check, so the mod never mistakes "couldn't check" for "clean". `scripts/test-containers-guard.sh` gains `--json` cases (findings, clean, disabled by `AI_GENT_NO_LEFTOVERS_CHECK=1`, a fake engine), and `scripts/consistency.py` pins the key names the mod reads. The JSON carries counts and numbers only, never the advice sentences (the mod's pointer text is its own, FR-002).
- **Supersedes 017's FR-005 for this plugin.** 017 FR-005 says the `SessionStart` scripts "MUST NOT be run a second time" (its toast reads the stored hook row instead). This feature runs `leftovers.sh --json` again, on purpose: the band must refresh after container work, which no stored row can give. The session-start toast keeps its single read of the stored row; the band's reads are separate, throttled and JSON.
- **The rule it serves.** `shared/containers.md` §9a: resources are labelled with the project's label and removed **by ID or by label**, never by a prune; what isn't the project's own is listed with sizes and **asked**; the disk floor is 20 GB or 10 % of the disk, whichever is larger; "report what's left" ends every handoff that ran containers. The band makes §9a's last bullet continuous instead of a one-off.
- **Same stance as 017.** The mod relays the script's own findings and computes no check of its own; it never removes, stops or builds anything, and what it names to run is only what the script already names (`wt.py finish`, `wt.py resources`, the project's `clean` recipe), all by-ID or by-label.
- Owner: the `devcontainer` plugin; its module `plugins/devcontainer/hooks/register.tsx` (shared with 015's and 017's parts).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Leftovers stay visible (Priority: P1)

Once `leftovers.sh` has something to report, a one-line band above the prompt says so until the cause is gone:

`containers: disk 14 GB free, floor 20 · 2 dangling images · 1 volume of a gone worktree · 21 buildah (any project) — /containers`

**Why this priority**: the report today is a toast and a model-only line; by the time a build is interrupted again the person has forgotten it, which is how 21 buildah working containers (any project) piled up in this machine's store.

**Independent Test**: a `claude plugin test plugins/devcontainer` case mocks `$.process.run` for `leftovers.sh --json` with a recorded JSON stdout and mounts the band on `terminal` and `desktop`; it asserts the text by key. A second answer with `findings: []` asserts the band draws nothing.

**Acceptance Scenarios**:

1. **Given** the JSON holds a `dangling` and a `volumes` finding, **When** the band draws, **Then** it reads "containers: 2 dangling images · 1 volume of a gone worktree — /containers".
2. **Given** a `disk` finding, **When** drawn, **Then** the band leads with `disk 14 GB free, floor 20` (the script's own order), the text carries a `⚠` mark as well as any colour (the test asserts text by key, not colour), and "don't start parallel builders" is left to `/containers`.
3. **Given** `findings: []` (all clean) **or** `checked: false`, **When** refreshed, **Then** the band draws nothing; for `checked: false` the reason is kept in state for `/containers` to report.
4. **Given** a model turn running (`isWorking`) or a survey (`hasSurvey`), **When** the band would draw, **Then** it yields.
5. **Given** a finding with a `kind` the mod doesn't know (a newer script), **When** drawn, **Then** the band shows `<kind> <count>` from the JSON, never nothing and never a guess; a JSON that doesn't parse shows nothing and `/containers` says the report was unreadable.

---

### User Story 2 - `/containers` prints the whole report (Priority: P2)

`/containers` prints the findings (one line each, from the JSON) and the script's text form (run without `--json`) when the person wants the script's own advice, and a pointer: "`wt.py resources` lists them with sizes (agents:parallel); removal is by ID or by this project's label (containers.md §9a) and asks for anything that isn't this project's". It is output, not a model turn, and removes nothing.

**Independent Test**: raise `command.run` for `containers` with the script mocked; assert the text, that no other process was started, and that the text contains no prune command.

**Acceptance Scenarios**:

1. **Given** two findings, **When** `/containers` runs, **Then** both lines appear in the script's words followed by the pointer.
2. **Given** `findings: []`, **When** it runs, **Then** it says "no leftovers or disk problem reported for this project".
3. **Given** `checked: false`, **When** it runs, **Then** it says nothing was checked and gives the script's `reason` (`no-engine`, `not-a-checkout` or `disabled`), no guessing.

---

### User Story 3 - Fresh after container work (Priority: P2)

The band is read again at session start, after a Bash call that built, ran or removed container resources, and on `turn.complete` at most every two minutes.

**Independent Test**: a `tool.call` for `podman build -t x .` completes with the script mocked to a new answer; assert one extra read and the new text. A call for `ls` triggers none.

**Acceptance Scenarios**:

1. **Given** a completed call whose command starts with `podman`/`docker`/`buildah` and `build`, `run`, `rm`, `rmi`, `volume`, `compose`, or with `wt.py` (`add`, `remove`, `finish`), **When** it finishes, **Then** the band is read again once.
2. **Given** `wt.py finish` removed the dangling images, **When** the next read finds nothing, **Then** the band disappears.
3. **Given** two such calls within a second, **When** they finish, **Then** one read runs (the second joins the first).

### Edge Cases

- **No engine, no git checkout, script missing**: the band draws nothing; `/containers` says the reason the JSON gave, or that the script was missing.
- **A slow engine**: the script runs under `$.process.run` with a 10 s timeout (its hook timeout); on timeout the previous text stays and `/containers` says it timed out.
- **Other projects' buildah containers**: the script counts the whole store ("any project"); the band says `(any project)` and never offers removal; `wt.py baseline` and `wt.py finish` (agents) are what attribute and remove only the new ones.
- **Inside a worktree** (`.worktrees/<slug>`): the script computes the project from the main checkout's git common dir, so the band is the same in each.
- **Refresh cost**: the script runs a few engine calls (`images`, `volume ls`, one `volume inspect` per labelled volume, `ps -a --external`); the throttle and the trigger list keep it off every turn; never a timer while idle.
- **Narrow terminal**: one line sized to `e.props.bodyColumns`; under 60 columns it shortens to `containers ⚠ 3 — /containers`.
- **Non-terminal surfaces**: `AbovePrompt` is drawn where it exists; `/containers` prints text everywhere.
- **Two bands** (checkpoint, hygiene): composes beneath like 017's (`await next(e)` first).
- **Hot reload**: the last text and its read time live in `$.state`; the session-start toast is not repeated.
- **Headless (`claude -p`)**: no band; `/containers` still answers text.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The band's findings MUST come from `leftovers.sh --json`, read through `$.process.run` of the plugin's own script; the mod MUST NOT run an engine command of its own and MUST NOT compute a leftover, size or disk figure.
- **FR-002**: The mod MUST NOT remove, stop, build or pull anything, and no text it draws or prints MAY teach a prune; removal hints are only the script's own (by ID or by this project's label, `shared/containers.md` §9a). A test asserts no band or `/containers` text matches `prune`, and `scripts/consistency.py`'s prune check stays green.
- **FR-003**: Findings MUST be read from the JSON's `kind`, `count`, `free_gb` and `floor_gb` keys (pinned by `scripts/consistency.py`); the mod MUST NOT parse the script's sentences. An unknown `kind` is shown as `<kind> <count>`.
- **FR-004**: `AI_GENT_NO_LEFTOVERS_CHECK=1` silences the band as it silences the script (the mod adds no check of its own).
- **FR-005**: The refresh triggers MUST be: `session.start` (detached, like 014's, so the start isn't held), a completed Bash call whose command starts with a container engine and one of the verbs above or with `wt.py`, and `turn.complete` at most once per `containersRefreshSeconds`.
- **FR-006**: `userConfig`: `containersBand` (boolean, default true), `containersRefreshSeconds` (number, default 120), `containersCommand` (boolean, default true); the existing `leftoversToast` is unchanged.
- **FR-007**: The prerequisite change adds `--json` to `leftovers.sh` and its cases to `scripts/test-containers-guard.sh`; the default text output, `wt.py` and `shared/containers.md` are unchanged.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: `claude plugin test plugins/devcontainer` passes: stories 1–3 on `terminal` and `desktop`, the empty answer, an unrecognized line, the trigger list, the throttle.
- **SC-002**: `scripts/test-containers-guard.sh` passes, with its existing text cases unchanged and the new `--json` cases added.
- **SC-003**: On this machine, with its 21 buildah working containers in the store, the band appears within one refresh and says `21 buildah (any project)`; after `wt.py finish` removes the ones new since a baseline, the next refresh shows the remainder, and it stays absent when the store is clean.

## Out of scope

- Removing anything, offering a button to, or suggesting a prune (§9a).
- Sizes per resource (`wt.py resources` and `system df` do that).
- Disk figures beyond the script's below-floor line; other engines' stores.
- Changing the floor (`wt.py`'s `settings` and §9a own it).

## Decisions (2026-10-31, with the maintainer)

- The band reads `leftovers.sh` rather than `wt.py resources --json`: the script is in the same plugin (`$.plugin.root` points at it, as `plugins/secrets/hooks/register.tsx` reads its own `references/`; 015 R4 shows imports stop at the plugin's edge), while `wt.py` lives in `plugins/agents/scripts/` and `devcontainer` has no recorded way to find it.
- `leftovers.sh` gains `--json` as its own change, landed before the mod, so the mod never parses its sentences; `scripts/test-containers-guard.sh` gains the cases and `consistency.py` pins the shape. `--json` always prints an object when the script ran (`findings` empty when clean) and `{checked: false, reason}` when it could not check.
- The session-start toast stays beside the band: it reaches the person at the moment of attention, the band is what remains; turn the toast off with `leftoversToast` if the band is enough. This feature supersedes 017's FR-005 "never run a second time" for the band's own reads.
- The band's order follows the script's: disk, dangling images, volumes, buildah.
- `/containers` is a plain `/<name>` command (017's rule for `/release-preview`).

- Band priority (030): the band records itself in the `devcontainer` plugin's `bands` atom, `normal` while a `disk` finding is present and `info` for leftovers alone, because the spec declared none; it registers no `/bands` command (git and project do).

## Assumptions

- [ASSUMPTION: `$.process.run` passes the session's environment to the script, so `CONTAINER_ENGINE` and `AI_GENT_NO_LEFTOVERS_CHECK` apply as they do for the hook.]
- [ASSUMPTION: the JSON keys listed in the ground rules are the contract; `scripts/consistency.py` fails if the script's keys and the mod's differ (plan, task 2).]
- [ASSUMPTION: the engine lets a Bash `tool.call` hook read the command after `next(e)`, as 017's `MOVES_REFS` hook does.]
