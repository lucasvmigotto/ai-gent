# Feature Specification: Worktree tool (`ai-gent wt`)

**Feature Branch**: `docs/specs-rust-tools` (specification only)

**Created**: 2026-11-01

**Status**: Draft

**Input**: `docs/product/rust-decisions.md` (D3, D7, D9, D10), `specs/011-worktrees/spec.md` (the parity baseline), `plugins/agents/scripts/wt.py` (570 lines) and `test_wt.py` (402 lines, 16 tests, all passing on 2026-11-01), `shared/containers.md` §9a, `plugins/agents/skills/parallel/SKILL.md`.

**Depends on**: 032 (workspace and binary). Read by 022 (worktree board) and 025 (container/disk band), whose mods shell out to `wt.py list --json` and `leftovers.sh`.

## Context and ground rules

- `wt.py` has seven subcommands, not four: `plan`, `add`, `list`, `remove` (the four the request names) plus `baseline`, `resources` and `finish`, which carry the container half (disk floor, leftovers, dangling images) [OBSERVED: `main`, lines 536-566]. All seven move; the container half is the part `agents:parallel` and `shared/containers.md` §9a depend on.
- Container resources go by ID or label and are never pruned (`shared/containers.md` §9a). Every list is filtered by this project's label; the existing unit test `assert_scoped` (no `prune` in any call; every `ps`/`images`/`volume ls` carries a `label=` filter) is a requirement, not a detail.
- Behaviour baseline: `specs/011-worktrees` plus the tests. Windows is new (D3).
- The tool does not need Claude Code; it is Tier-agnostic (works for every agent that can run a shell command).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Add and list isolated worktrees (Priority: P1)

`ai-gent wt add <branch> [--base B] [--min-free-gb N]` creates a branch in `.worktrees/<slug>` off the chain's dev, keeps `.worktrees/` out of git via `.git/info/exclude`, assigns a compose project name, a port offset (a multiple of 100, never reusing a live one) and ownership labels, and prints them as JSON. `list [--json]` shows branch, path, dirty, ahead and behind versus dev.

**Why this priority**: it is the whole of `agents:parallel` and what this very specification session uses.

**Independent Test**: fixtures from `test_add_list_remove`, `test_refusals`, `test_offsets_never_reuse_a_live_one`, `test_from_inside_a_worktree`, `test_add_prints_labels_and_checks_disk`, `test_list_json_keeps_the_keys_the_worktree_board_reads`.

**Acceptance Scenarios**:

1. **Given** a repo with `dev`, **When** `add feat/a` runs, **Then** `.worktrees/feat-a` exists, `.git/info/exclude` lists `/.worktrees/` and `compose.worktree.yaml` once, and stdout is the JSON object `{branch, base, path, compose_project, port_offset, labels}` with labels `me.lucasvmigotto.ai-gent.project` and `me.lucasvmigotto.ai-gent.worktree`.
2. **Given** a live track at offset 100, **Then** the next one gets 200; a removed track's offset is reusable.
3. **Given** the branch or the directory exists, **Then** it refuses (exit 1) with the Python wording.
4. **Given** both `dev` and `develop` exist (or neither) and no `--base`, **Then** it refuses and asks for `--base`.
5. **Given** it is run from inside a worktree, **Then** it resolves the main checkout (`git rev-parse --git-common-dir`) and works.
6. **Given** `list --json`, **Then** each row has exactly `branch, path, dirty, ahead, behind` (ahead/behind null with no dev branch; detached HEAD shows `(detached)`), the keys the worktree board reads.

### User Story 2 - Disk floor and volume cap before a new track (Priority: P1)

When a container engine exists, `add` refuses a track that would leave the engine store below the floor, or when live tracks already reach `max_volumes` (default `max_parallel`, default 3).

**Independent Test**: `test_add_reads_settings_and_caps_volumes`, `test_add_prints_labels_and_checks_disk`.

**Acceptance Scenarios**:

1. **Given** free space less than `(live+1) * volume_estimate_gb` plus the floor, **Then** exit 1, the engine's `system df` printed to stderr, and the message names `wt resources` and `--min-free-gb`.
2. The floor is `--min-free-gb`, else `disk_floor_gb` from `.ai-gent/parallel.yaml`, else `max(20 GB, 10% of the store)`.
3. **Given** no engine, **Then** the check is skipped with a stderr note, the worktree is still created.
4. The floor can be raised by the owner but this tool and the agents never lower it silently.

### User Story 3 - Remove a landed, clean track and its containers (Priority: P1)

`remove <branch> [--keep-containers]` deletes a worktree only when it is clean and every commit is on dev (by hash or by content, `git cherry`), releases the track's labelled containers then volumes by ID, verifies by recount, removes the worktree and branch, prunes worktree metadata and frees the port offset.

**Independent Test**: `test_remove_after_a_rebase_merge`, `test_remove_releases_the_tracks_containers_and_volumes`, `test_remove_keeps_the_worktree_when_a_volume_stays`.

**Acceptance Scenarios**:

1. Uncommitted changes: refuse. Unlanded commits (`+` in `git cherry`): refuse, naming the count.
2. A rebase-merged branch (different hashes, same patches): removable; `branch -d` failing falls back to `-D` only because `git cherry` proved landing.
3. A container or volume still there after removal: exit 1, worktree kept, message names what remains.
4. Matching is by worktree label `.../<project>/<slug>` or compose label `<project>-<slug>` only.

### User Story 4 - Plan from the feature table (Priority: P2)

`plan [--json]` reads `specs/README.md` of the checkout it runs in and lists features whose dependencies are Implemented or Verified, those waiting, and those done. JSON: `{"ready": [...], "blocked": [...], "done": [...]}`.

**Independent Test**: `test_plan`, `test_plan_reads_the_worktree_it_runs_in`.

**Acceptance Scenarios**: a side's status word is matched by prefix (`planned`, `in progress`, `partial`, `implemented`, `verified`; `N/A`, an em dash and `retired` are not sides); a feature is done when every side is at least Implemented; a dependency counts once all its sides are at least Implemented. Header names `depends on`/`depends`/`dependencies` all work. Missing README: error `no specs/README.md: ...`.

### User Story 5 - Leftovers report and end-of-wave cleanup (Priority: P2)

`baseline` records the store's external (buildah) containers; `resources [--json]` reports this project's images, dangling images, labelled volumes (with orphan flag), containers, new external containers and the disk against the floor, removing nothing; `finish` removes this project's dangling images (never `-f`) and the external containers new since the baseline, unless an image build is running.

**Independent Test**: `test_resources_reports_orphans_and_never_removes`, `test_finish_removes_dangling_and_new_buildah_containers_only`, `test_finish_leaves_buildah_containers_while_a_build_runs`, `test_without_an_engine_containers_are_skipped`.

**Acceptance Scenarios**: as the test names; `finish` returns 1 only when a removal "didn't take".

## Requirements *(mandatory)*

- **FR-001**: Subcommands `plan, add, list, remove, baseline, resources, finish`, flags and defaults as `contracts/cli.md`.
- **FR-002**: Worktrees live in `.worktrees/` at the main checkout root; the slug is the lowercased branch with runs of non-`[a-z0-9]` collapsed to `-`.
- **FR-003**: State files `.worktrees/.ports.json` (sorted keys, indent 2, trailing newline) and `.worktrees/.containers-baseline.json` keep their shape.
- **FR-004**: Settings from `.ai-gent/parallel.yaml`, flat `key: number` lines with optional `#` comment: `max_parallel` (3), `max_volumes`, `volume_estimate_gb` (10), `disk_floor_gb`.
- **FR-005**: Engine selection: `$CONTAINER_ENGINE`, else `podman`, else `docker` on `PATH` (on Windows also `podman.exe`/`docker.exe` through the same lookup); `containers.md` §1.
- **FR-006**: No `prune` anywhere; every list filtered by label or compose project; removals by ID. A unit test over a recording fake engine asserts it, and the Rust fake engine must be a portable test binary (the Python one is a `#!/usr/bin/env python3` script, which does not run on Windows).
- **FR-007**: git is invoked as a subprocess (decision in plan); the same commands as today: `rev-parse --path-format=absolute --git-common-dir`, `--show-toplevel`, `for-each-ref`, `worktree add -q -b`, `worktree list --porcelain`, `status --porcelain`, `rev-list --left-right --count`, `cherry`, `branch -d/-D`, `worktree remove`, `worktree prune`.
- **FR-008**: Windows: paths are handled as `PathBuf`; porcelain paths from git (forward slashes with a drive letter) are normalised before comparison with the root; written JSON uses `/`; branch names with `/` are slugged so the directory has no separator.
- **FR-009**: Windows symlinks: a worktree checkout of this repo contains symlinks (`plugins/*/references/*`, `plugins/*/scripts/passstore.py`). With `core.symlinks=false` git writes them as plain files. `wt add` on Windows must detect `core.symlinks` and, when false, print a warning that skills' relative reference links will be text files in that worktree (contributor concern only; installed packages use the tarball). Not an error.
- **FR-010**: Path length: refuse or warn when `<root>\.worktrees\<slug>` plus the longest tracked path would exceed 260 characters on Windows without long-path support [ASSUMPTION: threshold check; verify behaviour on Windows].
- **FR-011**: `builds_running` replaces the Linux `/proc` scan with a cross-platform process listing (a crate such as `sysinfo`, or `ps`/`tasklist` fallback) that returns this project's builds (cwd inside the root) and others; where the cwd of a process cannot be read (macOS without permission, Windows), every build counts as the project's (the Python `ps` fallback behaviour).
- **FR-012**: `store_usage` finds the engine's storage root from `info --format {{.Store.GraphRoot}}` then `{{.DockerRootDir}}`, else `/` (Windows: the system drive); free/total via a cross-platform disk API. On Docker Desktop or Podman machine the root is inside a VM: report the host drive and say so (open question 2).
- **FR-013**: Exit codes: 0 success; 1 failure (`Failed` in Python) with `wt: message` on stderr [TO CAPTURE the exact stderr prefix from `main`, lines 536-570]; 2 usage error.
- **FR-014**: `list --json` and `plan --json` keep their exact key sets and nulls (the 022 board and its fixtures `plugins/agents/hooks/worktrees-fixtures.ts` are consumers).

## Success Criteria *(mandatory)*

- **SC-001**: The 16 tests of `test_wt.py` are shared fixtures passing on both implementations, plus the Windows cases, before `wt.py` is deleted.
- **SC-002**: `add`, `list`, `plan` JSON equal the Python bytes on every fixture.
- **SC-003**: The fake-engine call log of every fixture contains no `prune` and only label-filtered listings.
- **SC-004**: The tool works from inside a worktree and from a subdirectory.
- **SC-005**: After cutover `git grep -n 'wt\.py'` outside `CHANGELOG.md`, `specs/` and `docs/product/` is empty.
- **SC-006**: `ai-gent wt add` on a Windows runner creates a worktree in a short-path temp repo and `remove` cleans it.

## Edge Cases

- Detached HEAD worktree under `.worktrees/`: listed as `(detached)`, ahead/behind null, never removable by branch name.
- `.git` is a file (running inside a worktree): the common dir is used for exclude and root.
- `specs/README.md` rows with extra columns or a feature number not three digits: skipped.
- Branch name with uppercase or `.`: slug collapses; two branches with the same slug collide: refuse (`exists`).
- Engine present but daemon down: `info` fails; `add` should report the engine error, not crash (Python raises `Failed` from `ctr`).
- Podman rootless storage on a different drive than the repo: the floor uses the store's filesystem.

## Out of Scope

- Creating compose files, starting stacks, or merging branches (the skills do that).
- Replacing `plugins/devcontainer/hooks/leftovers.sh` (it only references `wt.py`: 3 lines; it keeps its own logic and switches the mention).
- A TUI.

## Open Questions

1. **Subcommand name**: `ai-gent wt` (D9 lists `wt`). Recommended: keep `wt`.
2. **Docker Desktop/Podman machine on Windows and macOS**: the store lives in a VM; `GraphRoot` is a VM path. Recommended: report free space of the host drive that holds the VM disk image when it can be found, else of the project drive, and mark the figure "approximate" in `resources`. Needs a Windows/macOS run to design.
3. **Symlink warning** (FR-009) versus making `wt add` set `core.symlinks=true` locally when Developer Mode is on. Recommended: warn only; never change git config.
4. **Cutover release** as in 035 open question 2.
5. **Fake engine** as a Rust test binary versus shipping it under `rust/fixtures/bin/`. Recommended: a `[[bin]]` with `test = false` in the `ai-gent` crate gated by a `test-support` feature so it is never in the package.
