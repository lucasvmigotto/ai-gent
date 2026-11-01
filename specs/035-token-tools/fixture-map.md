# Fixture map: Python tests to shared fixtures

Every `def test` of `plugins/agents/scripts/test_token_report.py` (68 tests, 16 classes; `python3 -m unittest plugins/agents/scripts/test_token_report.py` prints `Ran 68 tests`) and where the Rust port is held to it.

- A **fixture** is `rust/fixtures/tokens/<case>.json`, run by `scripts/test-parity-tokens.py` (oracle: `token_report.py`; `--bin` runs the binary) and by `cargo test -p ai-gent --test parity_tokens` (the binary). It holds the file tree, the commands, the stdout and exit code and every file the run changed.
- A **unit test** is a Rust test in `ai-gent-core` (pure functions the Python tests call directly; the pct table is read from `plugins/agents/hooks/pct-fixtures.ts`, the same file the Python and TypeScript tests read).
- A Python test asserting through internals (`tr.attribute`, `tr.split_coordinator`) is observed through the history, tracker and JSON outputs of the matching `tasks` fixture.

| Class | Test | Held by | Status |
|---|---|---|---|
| TokenReportTest | `test_discover_sessions_finds_the_one_session` | tokens::transcripts::tests::dedup_by_message_id_counts_once_and_discovery | done |
| TokenReportTest | `test_dedup_by_message_id_counts_once` | tokens::transcripts::tests::dedup_by_message_id_counts_once_and_discovery | done |
| TokenReportTest | `test_label_of_prefers_leading_task_ids` | tokens::transcripts::tests::label_of_prefers_leading_task_ids | done |
| TokenReportTest | `test_gather_row_carries_prompt_and_edit_size` | tokens::transcripts::tests::gather_row_carries_prompt_and_edit_size | done |
| TokenReportTest | `test_project_name_comes_from_the_slug_not_a_hardcoded_project` | tokens::transcripts::tests::project_name_comes_from_the_slug_not_a_hardcoded_project | done |
| TokenReportTest | `test_report_writes_project_wide_rollup` | fixture `report-rollup` | done |
| TokenReportTest | `test_report_summary_is_a_few_lines_and_top_trims_the_file` | fixture `report-summary-and-top` | done |
| TokenReportTest | `test_report_without_out_or_summary_is_rejected` | fixture `report-needs-out-or-summary` | done |
| TokenReportTest | `test_idle_rebuilds_counts_cache_writes_after_gaps_over_five_minutes` | fixture `report-summary-idle-rebuilds` | done |
| TokenReportTest | `test_snapshot_and_backfill_merge_without_duplicates` | fixture `snapshot-and-backfill` | done |
| TasksTest | `test_attribution_levels` | fixture `tasks-attribution-levels` | done |
| TasksTest | `test_expand_ids_and_tokens_format` | tokens::util::tests::expand_ids_and_tokens_format | done |
| TasksTest | `test_tasks_writes_lines_tracker_history_ledger` | fixture `tasks-writes-lines-tracker-history-ledger` | done |
| TasksTest | `test_rerun_updates_numbers_without_duplicating` | fixture `tasks-rerun-is-idempotent`; fixture `tasks-rerun-replaces-the-tag` | done |
| TasksTest | `test_resumed_agent_counts_once_across_config_dirs` | fixture `tasks-resumed-agent-across-config-dirs` | done |
| TasksTest | `test_coordinator_split_by_window_weights` | fixture `tasks-coordinator-split-by-window-weights` | done |
| TasksTest | `test_dry_run_writes_nothing` | fixture `tasks-dry-run-writes-nothing` | done |
| EstimateTest | `test_few_records_fall_back_to_the_starting_table` | fixture `estimate-starting-table-opus-json`; fixture `estimate-starting-table` | done |
| EstimateTest | `test_previous_sonnet_defaults_are_kept_and_the_measured_ones_documented` | tokens::estimate::tests::previous_sonnet_defaults_are_kept_and_the_measured_ones_documented | done |
| EstimateTest | `test_history_takes_over_with_median_p75_and_widens_the_match` | fixture `estimate-history-json`; fixture `estimate-widens-to-effort-model-size` | done |
| EstimateTest | `test_confidence_weights_pull_the_median_to_exact_records` | fixture `estimate-confidence-weights` | done |
| EstimateTest | `test_load_history_drops_a_seed_the_repo_already_measured` | fixture `estimate-seed-dedup` | done |
| EstimateTest | `test_estimate_command_prints_projection_basis_and_card_line` | fixture `estimate-history-text` | done |
| TagEstimateTest | `test_full_match_is_high_confidence_with_median_and_p75` | fixture `estimate-tags-json` | done |
| TagEstimateTest | `test_backoff_drops_size_then_stack_then_verification_then_layer` | fixture `estimate-tags-backoff-layer`; fixture `estimate-tags-backoff-size` | done |
| TagEstimateTest | `test_too_few_matches_even_at_the_last_level_fall_back` | fixture `estimate-tags-too-few-fall-back`; fixture `estimate-tags-too-few-opus` | done |
| TagEstimateTest | `test_stack_overlap_adds_weight_without_excluding` | fixture `estimate-tags-stack-overlap` | done |
| TagEstimateTest | `test_inferred_tags_weigh_half` | fixture `estimate-tags-inferred-half` | done |
| TagEstimateTest | `test_context_factor_is_learned_from_peer_groups_and_applied` | fixture `estimate-tags-context-learned`; fixture `estimate-tags-context-prior` | done |
| TagEstimateTest | `test_bias_correction_needs_five_records_per_tag` | fixture `estimate-tags-bias` | done |
| TagEstimateTest | `test_events_stay_out_of_task_cost` | fixture `estimate-tags-events-out-of-cost` | done |
| TagEstimateTest | `test_command_prints_level_confidence_and_context` | fixture `estimate-tags-text` | done |
| TasksJsonTest | `test_json_shape_and_z_timestamps` | fixture `tasks-json-shape` | done |
| TasksJsonTest | `test_tasks_summary_and_top_shorten_the_text` | fixture `tasks-summary-and-top` | done |
| TasksJsonTest | `test_history_record_carries_the_full_model_id` | fixture `tasks-history-carries-model-id` | done |
| TasksJsonTest | `test_json_writes_nothing` | fixture `tasks-json-writes-nothing` | done |
| TasksJsonTest | `test_json_without_dry_run_is_rejected` | fixture `tasks-json-needs-dry-run` | done |
| TasksJsonTest | `test_text_output_unchanged_without_json` | fixture tasks-summary-and-top (text output of --dry-run) | done |
| IndexSeedTest | `test_seed_marks_confidence_and_writes_only_the_index` | fixture `seed-marks-confidence-and-is-idempotent` | done |
| IndexSeedTest | `test_seed_infers_coarse_low_confidence_tags_from_manifests_and_touched_files` | fixture `seed-infers-tags-from-manifests-and-files`; fixture `seed-rust-stack` | done |
| IndexSeedTest | `test_index_merges_repo_histories_keeps_seeds_and_drops_superseded_ones` | fixture `index-merges-and-drops-superseded-seeds` | done |
| LegacyTest | `test_tasks_still_writes_tokens_md_and_the_unmapped_section_unless_no_legacy` | fixture `legacy-no-legacy`; fixture `legacy-tokens-md-and-unmapped` | done |
| LegacyTest | `test_estimate_falls_back_to_the_rollup_when_history_is_empty` | fixture `estimate-rollup-fallback` | done |
| TagsTest | `test_parse_format_and_aliases` | tokens::tags::tests::parse_format_and_aliases | done |
| TagsTest | `test_repo_info_reads_manifests_read_only` | tokens::tags::tests::repo_info_reads_manifests_read_only | done |
| TagsTest | `test_infer_tags_from_description_files_and_repo` | tokens::tags::tests::infer_tags_from_description_files_and_repo | done |
| EventsTest | `test_resume_after_a_limit_stop_and_fix_rounds_stay_out_of_clean_cost` | fixture `events-resume-fix-rounds-stay-out-of-clean-cost` | done |
| UsageSinceTest | `test_usage_since_sums_billable_after_the_cutoff_and_dedupes_by_id` | fixture `budget-dedupes-by-message-id-across-sessions` | done |
| UsageRateTest | `test_steady_rate_counts_only_the_window_and_only_billable_tokens` | tokens::budget::tests::usage_rate_counts_only_the_window_and_only_billable_tokens | done |
| UsageRateTest | `test_idle_is_zero_messages` | tokens::budget::tests::usage_rate_counts_only_the_window_and_only_billable_tokens | done |
| UsageRateTest | `test_window_override` | tokens::budget::tests::usage_rate_counts_only_the_window_and_only_billable_tokens | done |
| UsageRateTest | `test_minimum_messages_is_three` | tokens::budget::tests::usage_rate_counts_only_the_window_and_only_billable_tokens (MIN_RATE_MESSAGES) | done |
| BudgetTest | `test_parse_budget_reads_flat_keys_with_suffixes_and_defaults` | tokens::budget::tests::parse_budget_reads_flat_keys_with_suffixes_and_defaults | done |
| BudgetTest | `test_verdicts_proceed_shrink_stop_after_what_the_session_used` | fixture `budget-verdicts-proceed-shrink-stop` | done |
| BudgetTest | `test_json_is_one_object_with_the_same_verdict_and_nothing_else` | fixture `budget-json-matches-text` | done |
| BudgetTest | `test_budget_reports_the_rate_and_each_caps_eta` | fixture `budget-rate-and-eta` | done |
| BudgetTest | `test_budget_idle_has_a_zero_rate_and_no_eta` | fixture `budget-idle` | done |
| BudgetTest | `test_budget_below_the_minimum_messages_has_no_rate_and_no_eta` | fixture `budget-too-few-messages` | done |
| BudgetTest | `test_budget_without_caps_still_prints_the_rate` | fixture `budget-without-caps-prints-rate` | done |
| BudgetTest | `test_budget_rate_window_comes_from_budget_yaml` | fixture `budget-rate-window-from-yaml` | done |
| BudgetTest | `test_budget_session_window_reads_the_week_once` | fixture `budget-session-window` | done |
| BudgetTest | `test_cards_come_from_cards_md_and_the_week_cap_counts_too` | fixture `budget-cards-from-cards-md-and-week-cap` | done |
| BudgetTest | `test_no_caps_means_proceed_with_a_hint` | fixture `budget-no-caps-proceeds` | done |
| BudgetTest | `test_two_opus_cards_are_queued_beyond_the_cap` | fixture `budget-two-opus-queued` | done |
| BudgetTest | `test_parallel_opus_adds_event_risk_and_tags_drive_the_card_projection` | fixture `budget-parallel-opus-and-tag-estimates` | done |
| PctParityTests | `test_pct_error_gives_each_fixture_percentage` | tokens::util::tests::pct_error_gives_each_fixture_percentage | done |
| PctParityTests | `test_cells_and_exact_figures_may_differ_by_a_point` | tokens::util::tests::cells_and_exact_figures_may_differ_by_a_point | done |
| PctParityTests | `test_ledger_cell_prints_what_the_toast_reads` | tokens::writers::tests::ledger_cell_prints_what_the_toast_reads | done |

68 rows. New Rust-only cases (Windows slug, CRLF, invalid UTF-8, huge line, read-only target, `--project=-slug`) are T050 and are not done (they need a Windows runner).

## Disagreements with the Python tool (intentional or found by the port)

| Where | Python | Rust | Decision |
|---|---|---|---|
| `report` and `tasks` (legacy roll-up) with stage markers | `render()` raises `TypeError`: the loop variable `top` of the By-stage table shadows the `--top` argument, so every run with a non-empty snapshots file or git landings fails | renders the By stage table | fix kept in Rust; no parity case (Python cannot produce one); unit test `tokens::report::tests::by_stage_table_renders_with_markers_and_top`. The Python tool is unchanged (D7). |
| "Generated YYYY-MM-DD HH:MM" line | local time | UTC | normalised in the comparison; the clock is `--now` (hidden) in tests |
| Python tracebacks on unreadable input (bad `--at`, missing git ref, non-UTF-8 file) | exit 1 with a traceback | exit 1 with one line on stderr | not compared (stderr) |
