# Fixture map: 036 worktree tool

The 16 tests of `plugins/agents/scripts/test_wt.py` and the shared fixtures in `rust/fixtures/wt/` that carry them (oracle: `scripts/test-parity-wt.py`; binary: `cargo test -p ai-gent --test parity_wt`). 23 fixtures: the 16 tests plus 7 edge cases (header variants, missing README, no dev branch, detached HEAD, disk_floor_gb, below-floor report, a removal that does not take).

| Python test | Fixtures |
|---|---|
| `test_plan` | plan-json, plan-header-variants, plan-no-readme |
| `test_add_list_remove` | add-list-remove |
| `test_refusals` | add-refusals, add-no-dev |
| `test_offsets_never_reuse_a_live_one` | add-offsets-never-reuse-a-live-one |
| `test_remove_after_a_rebase_merge` | remove-after-a-rebase-merge |
| `test_plan_reads_the_worktree_it_runs_in` | plan-reads-the-worktree-it-runs-in |
| `test_from_inside_a_worktree` | list-from-inside-a-worktree, list-detached-and-empty |
| `test_add_prints_labels_and_checks_disk` | add-disk-labels |
| `test_add_reads_settings_and_caps_volumes` | add-disk-settings, add-disk-floor-setting |
| `test_remove_releases_the_tracks_containers_and_volumes` | remove-containers |
| `test_remove_keeps_the_worktree_when_a_volume_stays` | remove-keeps-the-worktree-when-a-volume-stays |
| `test_resources_reports_orphans_and_never_removes` | resources-orphans, resources-below-floor |
| `test_finish_removes_dangling_and_new_buildah_containers_only` | finish-removes-dangling-and-new-buildah-containers-only, finish-a-removal-that-does-not-take |
| `test_finish_leaves_buildah_containers_while_a_build_runs` | finish-leaves-buildah-containers-while-a-build-runs |
| `test_list_json_keeps_the_keys_the_worktree_board_reads` | list-keys (+ parity_wt::list_json_keeps_the_keys_the_worktree_board_reads against worktrees-fixtures.ts) |
| `test_without_an_engine_containers_are_skipped` | no-engine |
