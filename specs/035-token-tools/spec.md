# Feature Specification: Token tools (`ai-gent tokens`)

**Feature Branch**: `docs/specs-rust-tools` (specification only; built later on its own branch)

**Created**: 2026-11-01

**Status**: Draft

**Input**: `docs/product/rust-decisions.md` (D7, D9, D10, D16, D17), `docs/product/release-3-notes.md`, `shared/token-tracking.md`, `shared/token-tags.md`, `plugins/agents/scripts/token_report.py` (1949 lines) and `plugins/agents/scripts/test_token_report.py` (911 lines, 68 tests, all passing on 2026-11-01).

**Depends on**: 032 (the Rust workspace and the `ai-gent` binary skeleton; written in parallel, not read here). Read by 014, 018, 019, 020, 021 (the mods that shell out to the tool).

## Context and ground rules

- `token_report.py` is **Claude Code only**: it reads Claude Code's own session transcripts (`~/.claude*/projects/<slug>/*.jsonl`, `<session>/subagents/agent-*.jsonl` and their `.meta.json` sidecars). No other agent has an equivalent, so under D2 this subcommand is a Tier 1 feature. On a machine with no `~/.claude*` directory it reports "no sessions found" and exits 0; it never fails Codex or Tier 3 users, who simply have no data. The skills that call it (`agents:orchestrate`, `agents:parallel`) already say the tracking is Claude-only; that sentence stays.
- The subcommand list in the request ("report/tasks/history/budget/projection") maps to the real tool as follows. There is **no `history` subcommand** and no `projection` subcommand in `token_report.py` [OBSERVED: `build_parser`, lines 1862-1934]. The real set is `report`, `tasks`, `estimate` (the projection), `budget`, `index` and `seed` (the history index), `snapshot` and `backfill-git` (stage markers). History is the file `docs/product/token-history.jsonl` that `tasks` writes. This spec keeps the real set; it does not invent new subcommands (D7: parity first).
- Behaviour is the parity baseline (D10). Every output a mod parses (`budget --json`, `tasks --dry-run --json`, the ledger cell text, `estimate` text) is a contract; byte-for-byte parity is required for the JSON and for files the tool writes; human text may change only where the contract says.
- The tool never writes a secret and reads none. It reads transcripts, which can contain secrets a user pasted; it extracts only usage numbers, model ids, timestamps, the first user message of an agent (first 8000 chars, used only for attribution and truncated to 120 chars in `desc`) and Edit/Write sizes. It must never log or print transcript text beyond `desc`.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Cost of a finished card, written where the pipeline reads it (Priority: P1)

`ai-gent tokens tasks` attributes each subagent to its card and task ids, and writes `used` onto `tasks.md` lines, a Tokens block in `tracker.md`, a Tokens column in `.ai-gent/orchestrate-status.md` and records in `docs/product/token-history.jsonl`, exactly as `token_report.py tasks` does.

**Why this priority**: it is the loop's "record" step; the history it writes feeds every estimate and the mods (014, 019, 020).

**Independent Test**: the shared fixtures derived from `TasksTest`, `TasksJsonTest`, `EventsTest`, `LegacyTest` run against both implementations.

**Acceptance Scenarios**:

1. **Given** the fixture session of `TasksFixture` (subagents `a1`..`a3` with known usage), **When** `tasks --project=SLUG` runs, **Then** the task lines, tracker block, ledger column and history line equal the Python outputs byte for byte (`test_tasks_writes_lines_tracker_history_ledger`).
2. **Given** the tool already ran, **When** it runs again after a new subagent finished, **Then** numbers are updated and nothing is duplicated: one tag per line, one tracker block, one history line (`test_rerun_updates_numbers_without_duplicating`).
3. **Given** one agent resumed across two config dirs, **Then** it counts once (`test_resumed_agent_counts_once_across_config_dirs`).
4. **Given** `--dry-run`, **Then** no file is written (`test_dry_run_writes_nothing`); with `--json` the stdout is exactly one object `{"cards": [...], "unattributed": N}` with keys `card, feature, model, projected, used, start, end, agents, calls` in that order and `Z` timestamps (`test_json_shape_and_z_timestamps`); `--json` without `--dry-run` is rejected (`test_json_without_dry_run_is_rejected`).
5. **Given** a transcript with a dangling or invalid JSON line, **Then** that line is skipped and the rest counts.

### User Story 2 - Report and idle-gap summary (Priority: P1)

`ai-gent tokens report --project=SLUG --summary` prints the few lines (totals, top labels, idle cache rebuilds over 5 minutes that re-wrote 20k or more); `--out PATH` writes the project roll-up.

**Independent Test**: `test_report_writes_project_wide_rollup`, `test_report_summary_is_a_few_lines_and_top_trims_the_file`, `test_idle_rebuilds_counts_cache_writes_after_gaps_over_five_minutes`, `test_dedup_by_message_id_counts_once`.

**Acceptance Scenarios**:

1. **Given** messages with repeated ids, **Then** each id counts once with its largest `output_tokens` usage; `<synthetic>` model messages are skipped.
2. **Given** neither `--out` nor `--summary`, **Then** exit 2 with a usage message (`test_report_without_out_or_summary_is_rejected`).

### User Story 3 - Projection from history (`estimate`) (Priority: P1)

`ai-gent tokens estimate --role builder --effort high --tags "extend, backend, ..."` prints the median, the p75, the neighbours used, the match level and the confidence, from history (this repo's and `~/.ai-gent/token-history.jsonl`), else the starting table.

**Independent Test**: `EstimateTest`, `TagEstimateTest`, `TagsTest` (13 tests).

**Acceptance Scenarios**:

1. **Given** fewer than 5 records, **Then** the starting table applies: Sonnet 55k/80k, 128k/179k, 220k/275k (low/medium/high, median/p75); Opus 250k/400k, 450k/650k, 700k/950k; Haiku 60k/120k, 120k/200k, 200k/320k; size S x0.6, L x1.6 (`test_few_records_fall_back_to_the_starting_table`).
2. **Given** tags, **Then** the match backs off in the documented order and the level sets the confidence (`test_backoff_drops_size_then_stack_then_verification_then_layer`), stack overlap adds weight but never excludes, inferred tags weigh half, bias correction needs 5 records per tag.
3. **Given** `STARTING_PREVIOUS` (Sonnet 150k/250k, 350k/550k, 600k/850k), **Then** it stays in the code as a reference constant (`test_previous_sonnet_defaults_are_kept_and_the_measured_ones_documented`).
4. **Given** `--json`, **Then** the object has the keys listed in `contracts/cli.md` (the Python `--json` of `estimate` is the contract; to be captured from the source in T004, not guessed).

### User Story 4 - Budget verdict for a wave, with burn rate (Priority: P1)

`ai-gent tokens budget --cards ID,ID --json` returns one JSON object (verdict `proceed|shrink|stop`, why, fit, total, need, rate, caps with `eta_minutes`, cards, batches) that the Claude Code mods (014 token status, 018 burn rate, 021 handoff nudge) parse.

**Independent Test**: `BudgetTest` (13 tests), `UsageSinceTest`, `UsageRateTest`, `PctParityTests`.

**Acceptance Scenarios**:

1. **Given** `.ai-gent/budget.yaml` caps and a session that used part of them, **Then** the verdict is `proceed`, `shrink` (with the first `fit` cards) or `stop` per `test_verdicts_proceed_shrink_stop_after_what_the_session_used`.
2. **Given** `--json`, **Then** stdout is one object and nothing else (`test_json_is_one_object_with_the_same_verdict_and_nothing_else`); the text lines are suppressed.
3. **Given** fewer than 3 messages in the rate window (`MIN_RATE_MESSAGES`), **Then** `rate` is null and no `eta_minutes` (`test_budget_below_the_minimum_messages_has_no_rate_and_no_eta`); idle gives a zero rate and no ETA.
4. **Given** the week scan, **Then** one pass over the transcripts serves the week cap, the rolling session window and the rate (`test_budget_session_window_reads_the_week_once`).
5. **Given** no cards named, **Then** exit 1 with the message `budget: name the wave's cards with --cards ID,ID or --projected [model:]400k,...`.

### User Story 5 - History index and seed; stage markers (Priority: P2)

`index` rebuilds `~/.ai-gent/token-history.jsonl` from repos' committed histories (a measured record supersedes a seed of the same agent); `seed` creates low-confidence records from past sessions (read-only on sessions and repos); `snapshot` and `backfill-git` write `token-snapshots.json` stage markers.

**Independent Test**: `IndexSeedTest` (3), `test_snapshot_and_backfill_merge_without_duplicates`.

**Acceptance Scenarios**: as the named tests; `seed --dry-run` writes nothing; `seed` writes only the index.

### User Story 6 - Fast enough to run in a status line (Priority: P2)

A week of transcripts is scanned faster than the Python tool does today, so the mods can call `budget --json` without lag.

**Independent Test**: `scripts/bench-tokens.sh` (see FR-020 and plan "Measurement").

**Acceptance Scenarios**:

1. **Given** the same corpus, **When** `budget --json` runs under both implementations, **Then** the Rust median wall time is at most the Python median, and the target is 3x faster or better on a corpus of at least 1 GB [ASSUMPTION: the ratio is a hypothesis to measure, not a fact].

## Requirements *(mandatory)*

- **FR-001**: `ai-gent tokens <sub>` provides `report`, `tasks`, `estimate`, `budget`, `index`, `seed`, `snapshot`, `backfill-git` with the flags, defaults and help text of the Python parser (`contracts/cli.md`).
- **FR-002**: Billable = `input_tokens + cache_creation_input_tokens + output_tokens`; `cache_read_input_tokens` is reported, never billed (`FIELDS`, `BILLABLE`).
- **FR-003**: Transcript discovery: `$CLAUDE_CONFIG_DIR` if set, else every `~/.claude*` directory; sessions are `projects/<slug>/*.jsonl` plus `projects/<slug>/<sid>/subagents/agent-*.jsonl`; a session whose main transcript is gone but has a `subagents` directory is still found (`discover_sessions`).
- **FR-004**: Messages are deduplicated by message id across all files and config dirs, keeping the entry with the greatest `output_tokens`; model `<synthetic>` is skipped but its timestamp is kept as a limit/error stop for resume detection.
- **FR-005**: Attribution levels exact / feature / agent and the five rules of `shared/token-tracking.md` "How attribution works"; anything else is unattributed and never guessed. Task id ranges (`T006-T009`) expand (`expand_ids`).
- **FR-006**: The coordinator split by stage window, weighted by attributed subagent billable in the window; stage markers come from `token-snapshots.json` or, when absent, from first-parent landings read from git in memory (`git_markers`).
- **FR-007**: Files written: task-line tag ` · Tokens: projected ~420k | used 391k` (replaced, never doubled; later tasks of a card say `· Tokens: with T001`), `tracker.md` Tokens block, ledger Tokens column `~projected / used` (`ledger_cell`), history JSONL (one line per card, upsert by `id`), optional roll-up and legacy `specs/*/tokens.md` unless `--no-legacy`.
- **FR-008**: History record keys exactly as in `data-model.md` (`make_record`), including `model_id` (the lead agent's most frequent full model id), `used_clean`, `events`.
- **FR-009**: Tag vocabulary is read from `shared/token-tags.json` (8 facets; facets `kind, layer, surface, lang, novelty, verification, size` required one value each, `stack` multi and free-form with aliases folded). The Rust binary must not hard-code a copy: the JSON is embedded at build time with `include_str!` from a path inside the crate (a generated copy kept equal by `scripts/consistency.py`, the mechanism already used for `secret-data.ts`), so the package and the crate agree. `scripts/consistency.py` (card tag lint) keeps reading the JSON.
- **FR-010**: Estimation per `shared/token-tracking.md` "History and estimates": tagged nearest-neighbour with back-off levels, Jaccard stack weight (up to +50%), confidence weights exact 1.0 / feature 0.6 / agent 0.3 (halved for inferred tags), size ratio S 0.6 / M 1 / L 1.6, context factor learned from peers (5 comparisons) with priors Opus 1.3 / Haiku 0.5, bias correction from 5 records per tag clamped to 0.5-2.0, untagged fallback, then the starting table. `loglinear_model()` stays an unbuilt hook returning nothing (`LOGLINEAR_MIN` 300).
- **FR-011**: Budget: defaults `max_opus 1, max_agents 2, reserve_pct 10, coordinator_overhead_pct 35, rate_window_minutes 30`; `.ai-gent/budget.yaml` flat `key: value` with `k`/`M` suffixes and `#` comments; caps `session_billable`, `week_billable`; optional `session_window_hours`; verdict logic and batch packing (`pack_batches`) as in `cmd_budget`; the JSON object as in `contracts/cli.md`.
- **FR-012**: Model ids: family is the first of `opus|sonnet|haiku` found in the lowercased model string, else `other`; role to model defaults `thinker -> opus, builder -> sonnet, scout -> haiku`, overridable by `.ai-gent/orchestrate.yaml`. A new model id (such as `claude-sonnet-5-5`) needs no code change; a new family does (open question 3).
- **FR-013**: Exit codes: 0 success (including a `shrink` or `stop` verdict: the verdict is data, not failure), 1 operational failure (the Python `sys.exit("message")` cases), 2 usage error (clap). `--project=SLUG` with a leading `-` in the slug must parse (`allow_hyphen_values`).
- **FR-014**: The tool never writes outside the paths in FR-007 and `~/.ai-gent/` (`$AI_GENT_HOME`), never makes a network call, never reads outside transcripts, `specs/`, `docs/product/`, `.ai-gent/` and git.
- **FR-015**: Windows (D3): transcript root `%USERPROFILE%\.claude*` equivalent via `dirs`-style home lookup; project slug derived from the cwd the way Claude Code derives it (non-alphanumeric to `-`), with the Windows form [ASSUMPTION: `C:\Users\x\repo` becomes `C--Users-x-repo`; verify on a Windows machine with Claude Code before relying on it]; timestamps always UTC; paths in written files use `/`.
- **FR-016**: Atomic writes: files are written to a temp file in the same directory and renamed, so a mod reading `tasks.md` or the ledger never sees a half-written file (Python writes in place; this is an improvement that does not change content).
- **FR-017**: `--json` outputs go to stdout alone; diagnostics to stderr.
- **FR-018**: The mods in `plugins/agents/hooks` (`tokens.ts`, `token-model.ts`, `cards.ts`, `landing.ts`, `ledger.ts`) and `plugins/project/hooks/handoff.ts` switch from `python3 .../token_report.py` to `ai-gent tokens` at cutover; a missing binary degrades each mod to "no data" (they already no-op when the script is absent), never an error (D8 still holds for skills: they fail with the install command).
- **FR-019**: Corrupt or partial inputs never panic: unreadable file, invalid UTF-8, a 100 MB line, a truncated last line are skipped with a count on stderr.
- **FR-020**: A benchmark harness `scripts/bench-tokens.sh` (planned) times both implementations on the same corpus and prints median of N runs (see plan "Measurement").

## Success Criteria *(mandatory)*

- **SC-001**: All 68 tests of `test_token_report.py` exist as shared fixtures and pass on both implementations before the Python file is deleted (D10).
- **SC-002**: `budget --json` and `tasks --dry-run --json` equal the Python output byte for byte on every fixture.
- **SC-003**: Files written by `tasks` equal the Python files byte for byte on every fixture.
- **SC-004**: On the benchmark corpus the Rust `budget --json` median is at most the Python median; goal 3x or better.
- **SC-005**: Passes on Linux, macOS and Windows runners (x86_64 at least; aarch64 under cross build or emulation where a runner exists) with the Windows-specific cases of FR-015.
- **SC-006**: After cutover `git grep -c 'token_report\.py'` outside `CHANGELOG.md`, `specs/` and `docs/product/` returns zero lines.

## Edge Cases

- Two config dirs hold the same session (account switch): counted once.
- A subagent with a `.meta.json` missing: attributed by prompt/description only; `agent-only` if nothing matches.
- A card's description names two features: the first when it leads with it, else unattributed (rule 4).
- Clock skew: `to_utc` converts offsets; a naive timestamp is treated as UTC.
- `tasks.md` has CRLF line endings (Windows checkout): preserve line endings on rewrite.
- A history file with a malformed line: skipped, kept on rewrite where possible, counted on stderr.
- Slug contains a leading `-` (always, since it starts with the path separator): flag parsing.
- 0 transcripts: every subcommand that needs sessions prints a one-line explanation and exits 0 for `report`/`budget` (budget treats usage as 0) and 1 for `tasks` without `--project` match [OBSERVED partly: `budget` handles `no session found`; confirm the other two in T003].

## Out of Scope

- Any new subcommand, a log-linear model, a Codex or other-agent transcript reader (a possible later spec; Codex session format unverified), network price lookups, a TUI.
- Rewriting `shared/token-tracking.md` beyond the cutover edits.

## Open Questions

1. **Shared parity harness**: should the fixture format of `contracts/parity-fixtures.md` here be the workspace-wide format (032 owns the harness)? Recommended: 032 owns the runner, this spec owns the token fixtures; align the format in T001.
2. **Cutover timing** (all tool specs): ship `ai-gent tokens` in 2.28 but switch skills and delete the Python at 3.0.0, or switch in 2.28? Recommended: switch at the release that also makes the package the only install (3.0.0), because 2.x git-mode clones carry no binary. D7 reads "cut over in one release"; this keeps that (cutover = 3.0.0) and only delays it.
3. **New model family**: when Anthropic ships a family other than opus/sonnet/haiku (`other` today), should the table be data (a JSON in `shared/`) rather than code? Recommended: yes, `shared/token-models.json` embedded like the tags, added in this port (small).
4. **Atomic rewrite vs in-place** (FR-016): confirm that a rename (which replaces the inode and can break a file watcher) is acceptable for `tasks.md` and the ledger. Recommended: yes, mods read on interval, not by watch.
5. **Windows slug** (FR-015): not verified here; needs a Windows machine with Claude Code.
