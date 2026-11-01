# Data Model: Token tools

All shapes are those of `plugins/agents/scripts/token_report.py` at 2.26.0; the Rust types serialise to the same bytes.

## Entities

| Entity | Fields | Source |
|---|---|---|
| Usage | `input_tokens`, `cache_creation_input_tokens`, `cache_read_input_tokens`, `output_tokens` (non-negative integers; missing = 0) | transcript `message.usage` |
| Message | id, utc timestamp `YYYY-MM-DDTHH:MM:SS`, model string, Usage | assistant record in a transcript; keyed by `message.id`, the larger `output_tokens` wins |
| Row (subagent) | agent id (`aid`), description, prompt (first user message, 8000 chars), edits (tool_use id to file and lines), stops (utc timestamps of `<synthetic>` messages), messages | `agent-*.jsonl` and `.meta.json` |
| Card | id (e.g. `004-us1-core`), feature, tasks, executor, reviewer, effort, dispatch description, projected, tags | `specs/*/cards.md` |
| Group | the rows attributed to one card or task set: card, feature, tasks, confidence (`exact|feature|agent`), model, model_id, billable, cache_read, calls, files, lines, events, event_cost, start, end | `build_groups` |
| Stage marker | `{"at": ISO UTC, "label": str}` sorted by `at`, unique by (at, label) | `token-snapshots.json` |
| Record | see below | history JSONL |
| Budget config | `max_opus`, `max_agents`, `reserve_pct`, `coordinator_overhead_pct`, `rate_window_minutes`, optional `session_billable`, `week_billable`, `session_window_hours` | `.ai-gent/budget.yaml` |

## History record (one JSON object per line)

`id, source ("measured"|"seed"), confidence, project, feature, card, tasks, layer, effort, tags (object of facet to value or list), tags_source ("planned"|"planned+inferred"|"inferred"), executor_role, executor_model, model_id, files, lines, projected, used, used_clean, events, cache_read, coordinator, calls, agents (sorted ids), desc (120 chars), date (YYYY-MM-DD)`.
Identity: `<project>:<feature>:<card or tasks key>`, or `<project>:agent:<agent id>` for agent-level; `seed:` prefix for seeds. Upsert by `id`; a measured record supersedes a seed record of the same agent. Key order as listed (Python dict order); to be re-confirmed against a blessed fixture in T012.

## Files written (all idempotent)

| File | Shape |
|---|---|
| `specs/NNN-*/tasks.md` | first task line of a card ends ` · Tokens: projected ~420k \| used 391k`; other tasks of the card ` · Tokens: with T001`; compact numbers via `fmt_tok` (`391k`, `1.24M`) |
| `specs/NNN-*/tracker.md` | a `Tokens` block: per card projected, used, error (`pct_error`: `+12%` or an em dash), subagent total, coordinator share, feature total |
| `.ai-gent/orchestrate-status.md` | `Tokens` column appended to the card table; cell `~420k / 391k`, `— / 391k` when no projection |
| `docs/product/token-history.jsonl` | records above |
| `~/.ai-gent/token-history.jsonl` | the cross-project index (`$AI_GENT_HOME`) |
| `token-snapshots.json` | JSON array, `indent=1`, `ensure_ascii=False` |
| `docs/product/token-usage.md`, `specs/*/tokens.md` | legacy outputs, kept until `--no-legacy` becomes default (out of scope here) |

## Vocabulary

`shared/token-tags.json`: 8 facets (`kind, layer, surface, lang, novelty, verification, size, stack`), values with descriptions; `required`/`multi` flags per facet. Embedded in the binary; `shared/token-tags.md` is the human twin and is edited together with the JSON (CLAUDE.md rule).

## Tables

Starting table (median, p75) per model and effort; previous Sonnet table; size scaling; role to model; context-factor priors; thresholds (`LOGLINEAR_MIN` 300, `MIN_RATE_MESSAGES` 3, 5-neighbour minimum, bias clamp 0.5-2.0, idle gap 300 s and 20,000 floor). Values are in `spec.md` FR-010..FR-012 and `token_report.py` lines 1292-1302, 1356, 1635-1637.
