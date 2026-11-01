# Implementation Plan: Worktree tool (`ai-gent wt`)

**Branch**: `docs/specs-rust-tools` | **Date**: 2026-11-01 | **Spec**: `spec.md`

## Summary

Port `plugins/agents/scripts/wt.py` to `ai-gent-core::wt` plus `ai-gent wt`, shelling out to `git` and to the container engine, proving parity with the 16 Python tests as shared fixtures and a portable fake engine, then cut over `agents:parallel` and the other 29 files that name `wt.py` and delete the script.

## Technical Context

**Crates** (candidates): `clap`, `serde`, `serde_json`, `regex` (or hand parsing), `sysinfo` or `fs4` + a process list for disk and builds, `tempfile`. No git library, no async.
**Git strategy decision**: shell out to `git`. Reasons: (1) the tool's contract is the output of `git worktree`/`cherry`/`rev-list`, and the Python runs the same commands, so behaviour is identical by construction; (2) `git cherry` (patch-id comparison, needed for rebase-merged branches) and `worktree add/remove/prune` are core to the tool, and whether a pure-Rust library covers them fully was not verified here [ASSUMPTION: libgit2 bindings and gitoxide have partial worktree support; check before reversing this decision]; (3) a native library adds a C build (libgit2) or a large dependency tree and a second place where Windows path rules differ from the user's git; (4) git is already a hard requirement of ai-gent and is present wherever worktrees are used. Cost: process spawn per call (`list` spawns 2 per worktree; the 022 board already limits rows). Revisit if `list` over 20 worktrees exceeds 500 ms on the reference machine.
**Container engine**: subprocess too (podman/docker CLI), same as Python.
**Testing**: Rust integration tests create a temp repo with `git init`, use a portable fake engine binary (`ai-gent-fake-engine`, test-support feature) that keeps its state in a JSON file like the Python one and records every call; fixtures in `rust/fixtures/wt/` (format from 035 `contracts/parity-fixtures.md`).

## Module layout

```
rust/ai-gent-core/src/wt/
  mod.rs        Failed error type, Settings (parallel.yaml), slug, project name
  git.rs        git() runner, root(), here(), chain_dev(), exclude(), worktrees() (porcelain parser)
  ports.rs      load/save/next offset
  readme.rs     readme_rows(), level(), plan()
  engine.rs     engine(), ctr(), ids(), owned(), release(), external(), labelled_volumes(), dangling()
  disk.rs       store_usage(), preflight()
  procs.rs      builds_running() (cross-platform)
  commands.rs   add, list, remove, baseline, resources, finish returning data; no printing
rust/ai-gent/src/cmd/wt.rs   clap, text/JSON rendering, exit codes
rust/fixtures/wt/            parity fixtures
rust/ai-gent/src/bin/fake_engine.rs   test-support only
```

## Constraint check

| Constraint | Result |
|---|---|
| D7 | cutover list below |
| D9 | `--json` on `plan`, `list`, `resources`; `add` already prints JSON; documented exit codes |
| D10 | 16 tests become fixtures (table) plus Windows cases |
| D3 | FR-008..FR-012 |
| `containers.md` §9a (ID or label, never prune) | SC-003 asserts on the fake engine's call log |
| Secrets | none handled |
| AGENTS.md | edits listed for approval; no version bump |

## Parity: Python tests to fixtures

`plugins/agents/scripts/test_wt.py`, 16 tests in one class `WtTest`:

| Test | Fixture | Windows variant |
|---|---|---|
| test_plan, test_plan_reads_the_worktree_it_runs_in | plan-* | none |
| test_add_list_remove, test_from_inside_a_worktree, test_refusals, test_offsets_never_reuse_a_live_one | add-*, list-* | path separators in JSON |
| test_remove_after_a_rebase_merge | remove-rebase | git on Windows |
| test_add_prints_labels_and_checks_disk, test_add_reads_settings_and_caps_volumes | add-disk-* | fake `store_usage` via `AI_GENT_WT_FAKE_USAGE` test hook (replaces the Python monkeypatch) |
| test_remove_releases_the_tracks_containers_and_volumes, test_remove_keeps_the_worktree_when_a_volume_stays | remove-containers-* | |
| test_resources_reports_orphans_and_never_removes, test_finish_removes_dangling_and_new_buildah_containers_only, test_finish_leaves_buildah_containers_while_a_build_runs | resources-*, finish-* | builds hook `AI_GENT_WT_FAKE_BUILDS` |
| test_list_json_keeps_the_keys_the_worktree_board_reads | list-keys | the TS fixtures `worktrees-fixtures.ts` are compared too |
| test_without_an_engine_containers_are_skipped | no-engine | |

The Python tests monkeypatch `wt.store_usage` and `wt.builds_running`; the binary needs two test-only hooks (env vars honoured only when built with the `test-support` feature, never in release binaries) for the same effect. The fake engine script becomes the Rust fake engine binary; state file format kept (`containers, volumes, images, calls`).

## Cutover (D7), exact steps

References counted with `git grep -c 'wt\.py'` excluding `specs/`, `docs/product/`, `CHANGELOG.md` on 2026-11-01: 99 lines in 30 files (includes the script and its test). Files:

- Skills and references: `plugins/agents/skills/parallel/SKILL.md` (13; the `WT="python3 <plugin>/scripts/wt.py"` variable becomes `WT="ai-gent wt"`), `plugins/agents/skills/orchestrate/SKILL.md` (3), `plugins/backend/skills/build/SKILL.md` (1), `plugins/frontend/skills/build/SKILL.md` (1), `plugins/devcontainer/skills/workflow/SKILL.md` (1), `plugins/devcontainer/skills/workflow/references/guardrails.md` (2), `shared/containers.md` (5), `README.md` (2).
- Mods and hooks: `plugins/agents/hooks/{worktrees.ts (6), worktrees-model.ts (6), worktrees.test.tsx (10), worktrees-fixtures.ts (1), register.tsx (1)}`, `plugins/agents/types/index.d.ts` (3), `plugins/devcontainer/hooks/{containers-model.ts (1), containers-model.test.ts (5), containers.test.tsx (4), leftovers.sh (3)}`, `plugins/project/hooks/{pipeline-model.ts (4), pipeline.test.tsx (1), pipeline-fixtures.ts (1)}`, `plugins/project/types/index.d.ts` (1).
- Manifests: `plugins/agents/.claude-plugin/plugin.json`, `plugins/devcontainer/.claude-plugin/plugin.json`.
- Scripts: `scripts/check.sh` (3), `scripts/consistency.py` (2), `scripts/test_consistency.py` (2), `plugins/project/scripts/test_scripts.py` (1; it parity-tests `wt` against the pipeline pane, so it needs the binary or a fixture).
- Guards: none names `wt.py` in `guard.sh` or `opencode/guard` [verified: absent from the grep list].
- Delete: `plugins/agents/scripts/wt.py`, `plugins/agents/scripts/test_wt.py`.
- Release: proposed together with 035 at the cutover release (3.0.0; 035 open question 2). `wt` is the best first port: smallest, no secrets, no transcripts, most references (so the cutover also exercises the reference-editing procedure).

## Risks

| Risk | Mitigation |
|---|---|
| Git for Windows path forms in porcelain | normalise drive-letter forward-slash paths; Windows CI fixtures |
| Symlinks in worktrees on Windows | FR-009 warning |
| MAX_PATH | FR-010 |
| Engine VM storage (Docker Desktop, Podman machine) | open question 2; report approximate |
| Process listing differs per OS | FR-011 fallback counts all builds as the project's, which only makes `finish` more cautious |
| The fake engine hides real-engine differences | a manual check against real podman in T060 (not CI) |

## Phased approach

1. Phase 0: fixture format and fake engine (portable).
2. Phase 1 (US4): `plan` and the README parser (pure, quick win).
3. Phase 2 (US1): `add`/`list` with git, ports, exclude.
4. Phase 3 (US2, US3): engine, disk floor, `remove`.
5. Phase 4 (US5): `baseline`, `resources`, `finish`, process listing.
6. Phase 5: Windows/macOS runs.
7. Phase 6: cutover.
