# Fixture map: 037

Which Python test (or behaviour) each shared fixture stands for. Fixtures: `rust/fixtures/spec/`, `rust/fixtures/repo/`; runners: `scripts/test-parity-spec.py`, `scripts/test-parity-repo.py` (oracle) and `cargo test -p ai-gent --test parity_spec --test parity_repo` (binary).

Counts (verified 2026-11-01): `python3 -m unittest plugins/repo/scripts/test_reporun.py plugins/repo/scripts/test_specsync.py` runs 53 tests (reporun 45: RemoteTest 2, ClassifyTest 2, EntryTest 3, ApiTest 11, CliTest 3, ProjectStatusTest 15, BatchTest 6, ChainTest 3; specsync 8); `test_compact.py` has 9 (not started, group D).

## spec (specsync, 8 tests + live corpus)

| Fixture | Derived from |
|---|---|
| `errors-exit-codes` | specsync.main (Failed -> 1) |
| `features-list` | specsync.cmd_features |
| `live-corpus` | live corpus: this repository's specs/ at 5606091 |
| `plan-all-tasks-checked-is-review` | SpecsyncTest::test_all_tasks_checked_is_review |
| `plan-in-sync-and-path-forms` | SpecsyncTest::test_tracker_round_trip_and_actions |
| `plan-introspec-story-list` | SpecsyncTest::test_introspec_story_list |
| `plan-items-and-story-assignment` | SpecsyncTest::test_items_and_story_assignment |
| `plan-orphans-reported` | SpecsyncTest::test_orphans_reported |
| `plan-parser-edge-cases` | SpecsyncTest (parser edge cases beyond the Python tests) |
| `plan-retired-from-notes` | SpecsyncTest::test_retired_from_notes |
| `plan-states-in-progress` | SpecsyncTest::test_states |
| `plan-states-least-advanced-side-wins` | SpecsyncTest::test_states |
| `plan-states-verified-closes-every-story` | SpecsyncTest::test_states |
| `plan-status-with-notes-after-it` | SpecsyncTest::test_status_with_notes_after_it |
| `plan-update-and-state-after-a-checked-task` | SpecsyncTest::test_tracker_round_trip_and_actions |
| `record-creates-the-tracker` | SpecsyncTest::test_tracker_round_trip_and_actions |

## repo (reporun, 45 tests)

| Fixture | Derived from |
|---|---|
| `api-azure-basic-and-api-version` | ApiTest::test_azure_basic_and_api_version |
| `api-bitbucket-pagination` | ApiTest::test_bitbucket_pagination |
| `api-errors-redacted` | ApiTest::test_errors_redacted |
| `api-forgejo-token-header-and-repo-path` | ApiTest::test_forgejo_token_header_and_repo_path |
| `api-github-bearer-and-placeholders` | ApiTest::test_github_bearer_and_placeholders |
| `api-github-public-without-a-token` | reporun.api_call (github needs no entry) |
| `api-gitlab-project-path-encoded` | ApiTest::test_gitlab_project_path_encoded |
| `api-jira-basic-with-email` | ApiTest::test_jira_basic_with_email |
| `api-jira-without-a-url` | reporun.api_base (jira) |
| `api-missing-entry` | ApiTest::test_missing_entry |
| `api-other-hosts-refused` | ApiTest::test_other_hosts_refused |
| `api-write-needs-flag` | ApiTest::test_write_needs_flag |
| `batch-cli-steps-keep-json-structured` | reporun.cmd_batch (cli steps) |
| `batch-dry-run-calls-nothing` | BatchTest::test_dry_run_calls_nothing |
| `batch-needs-write-once` | BatchTest::test_needs_write_once |
| `batch-plan-errors` | reporun.cmd_batch (validation and reference errors) |
| `batch-query-and-paginate-steps` | reporun.cmd_batch (api options) |
| `batch-reads-need-no-write` | BatchTest::test_reads_need_no_write |
| `batch-references-and-records` | BatchTest::test_references_and_records |
| `batch-stops-at-first-failure` | BatchTest::test_stops_at_first_failure |
| `batch-text-that-quotes-the-syntax-stays` | BatchTest::test_text_that_quotes_the_syntax_stays |
| `canary-process-list` | FR-A5 (process list) |
| `canary-token-never-leaves-through-output` | FR-A5 |
| `chain-ambiguous-asks` | ChainTest::test_ambiguous_asks |
| `chain-full` | ChainTest::test_full_chain |
| `chain-no-integration-branch` | ChainTest::test_no_integration_branch |
| `chain-no-production-and-no-default` | reporun.cmd_chain (nothing to go on) |
| `chain-remote-default-branch` | reporun.cmd_chain (no main or master) |
| `classify-api-through-batch-dry-run` | ClassifyTest::test_api |
| `classify-cli-through-batch-dry-run` | ClassifyTest::test_cli |
| `cli-child-status-and-stderr-redacted` | reporun.cmd_cli / run_cli_capture |
| `cli-forgejo-has-no-cli` | ApiTest::test_forgejo_has_no_cli |
| `cli-not-installed` | reporun.prepare_cli (cli missing) |
| `cli-refusals` | CliTest::test_refusals |
| `cli-token-in-env-only-and-redacted` | CliTest::test_token_in_env_only_and_redacted |
| `cli-write-with-flag` | CliTest::test_write_with_flag |
| `detect-cli-installed-and-signed-in` | reporun.cli_state |
| `detect-jira-from-the-entry` | reporun.cmd_detect (jira) |
| `detect-never-decrypts` | EntryTest::test_detect_never_decrypts |
| `detect-no-remote` | reporun.detect_repo (no remote) |
| `detect-remote-urls` | RemoteTest::test_platforms, ::test_forced_platform |
| `entry-rules` | EntryTest::test_entry_rules |
| `project-status-alias-and-punctuation-match` | ProjectStatusTest::test_alias_match, ::test_punctuation_insensitive_match, ::test_no_match_lists_options, ::test_field_found_case_insensitively |
| `project-status-batch-rejects-other-platforms` | ProjectStatusTest::test_batch_rejects_project_status_on_other_platforms |
| `project-status-batch-step-needs-write` | ProjectStatusTest::test_batch_step_needs_write |
| `project-status-batch-step-written-and-resolves-references` | ProjectStatusTest::test_batch_step_written_and_resolves_references |
| `project-status-explicit-value-bypasses-mapping` | ProjectStatusTest::test_explicit_value_bypasses_mapping |
| `project-status-needs-state-or-value` | ProjectStatusTest::test_needs_state_or_value |
| `project-status-no-owner` | reporun.project_status_call (no owner) |
| `project-status-owner-defaults-to-remote` | ProjectStatusTest::test_owner_defaults_to_remote |
| `project-status-refused-without-write` | ProjectStatusTest::test_refused_without_write |
| `project-status-state-mapped-and-written` | ProjectStatusTest::test_state_mapped_and_written |
| `project-status-unknown-field-fails` | ProjectStatusTest::test_unknown_field_fails, ::test_field_missing, ::test_field_without_options_rejected |

Tests that the CLI cannot reach on their own are covered by Rust unit tests with the same cases: `ClassifyTest::test_cli` (`repo::cli::tests::classification`, and through `batch --dry-run` in `classify-cli-through-batch-dry-run`), `RemoteTest` (`repo::remote::tests::platforms`), the field and option resolvers of `ProjectStatusTest` (`repo::project_status::tests`).

## pass-store

| Rust test (`rust/crates/ai-gent-core/tests/passstore.rs`) | Derived from |
|---|---|
| `entry_rules`, `has_is_a_file_check_and_never_runs_pass` | `shared/passstore.py` `check`/`has`; `EntryTest::test_entry_rules` |
| `read_gives_secret_and_notes_and_registers_for_redaction` | `EntryTest::test_notes`; `RedactTest` behaviour for repo |
| `bad_or_unreadable_entry_message_has_no_value`, `read_times_out_with_the_passphrase_message`, `pass_missing_is_reported` | `PassTest` (bad or unreadable entry) |
| `prefix_override` | `PassTest::test_prefix_override` |
| `write_never_overwrites_and_reports_failure` | `passstore.write` |
| `gopass_is_used_only_without_pass` | new (US2) |
| (`PassTest::test_sources_in_order`, `::test_listing_shows_source_never_value`) | belong to `dbrun` (038): they test the db runner's source order and listing, not the module |
