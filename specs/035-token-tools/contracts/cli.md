# CLI contract: `ai-gent tokens`

Parity baseline: `plugins/agents/scripts/token_report.py` `build_parser` (lines 1862-1934). What is stated here was captured from blessed Python runs (`rust/fixtures/tokens/`, task T004), not guessed.

Global: `--json` outputs are one JSON document on stdout, diagnostics on stderr. Slugs start with `-`, so `--project=SLUG` and `--project -slug` both parse.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | success, including a `shrink` or `stop` budget verdict and "no sessions found" for `report`/`budget` |
| 1 | operational failure with a message on stderr (the Python `sys.exit("...")` cases: no cards named for `budget`, unreadable required input) |
| 2 | usage error (unknown flag, missing required flag, `report` with neither `--out` nor `--summary`, `tasks --json` without `--dry-run`) |

## Subcommands

| Command | Flags (default) | Writes |
|---|---|---|
| `tokens report` | `--project SLUG` (required), `--session ID`..., `--out PATH`, `--summary`, `--top N`, `--snapshots PATH` (`token-snapshots.json`) | `--out` file only |
| `tokens tasks` | `--project` (required), `--session`..., `--specs-dir specs`, `--rollup-out PATH`, `--history docs/product/token-history.jsonl`, `--ledger .ai-gent/orchestrate-status.md`, `--name`, `--snapshots`, `--no-legacy`, `--dry-run`, `--summary`, `--top N`, `--json` (needs `--dry-run`) | task lines, tracker.md, ledger, history, legacy files; nothing with `--dry-run` |
| `tokens estimate` | `--role builder\|thinker\|scout` or `--model M` (default sonnet), `--effort low\|medium\|high` (medium), `--layer`, `--tags "..."`, `--tasks N`, `--history FILE`..., `--label`, `--rollup`, `--json` | nothing |
| `tokens budget` | `--cards ID,ID` or `--projected [model:]400k,...`, `--specs-dir specs`, `--budget .ai-gent/budget.yaml`, `--project SLUG`, `--session ID`..., `--history FILE`..., `--json` | nothing |
| `tokens index` | `--repo DIR\|FILE`..., `--scan DIR`..., `--index PATH` (`~/.ai-gent/token-history.jsonl`) | the index |
| `tokens seed` | `--project SLUG`..., `--codes-dir ~/codes`, `--index PATH`, `--dry-run` | the index |
| `tokens snapshot` | `--label` (required), `--at ISO8601`, `--snapshots` | snapshots file |
| `tokens backfill-git` | `--repo PATH\|-` (required), `--ref origin/dev`, `--snapshots` | snapshots file |

`--label` and `--rollup` on `estimate` are deprecated and kept for parity (legacy fallback, test `test_estimate_falls_back_to_the_rollup_when_history_is_empty`).

## `budget --json`

```json
{"verdict":"proceed|shrink|stop","why":"...","fit":2,"total":520000,"need":780000,
 "rate":{"per_hour":120000,"window_minutes":30,"messages":14}|null,
 "caps":[{"name":"session|week","cap":12000000,"used":3000000,"left":9000000,"eta_minutes":null|75}],
 "cards":[{"label":"...","model":"sonnet","projected":260000,"p75":350000|null,"confidence":"high|medium|low|null","how":"..."}],
 "batches":[["card-a","card-b"],["card-c"]]}
```
Key order as above (Python dict order). Consumers: `plugins/agents/hooks/token-model.ts`, `plugins/project/hooks/handoff.ts`. `rate` is null when fewer than 3 messages and not idle; idle gives `{"per_hour":0,...,"messages":0}`.

## `tasks --dry-run --json`

`{"cards":[{"card","feature","model","projected","used","start","end","agents","calls"}],"unattributed":N}`; timestamps `YYYY-MM-DDTHH:MM:SSZ`; `projected` an integer. Consumers: 014 story 5, 020.

## `estimate --json`

One JSON document (default `json.dumps`: `ensure_ascii`, `", "` and `": "` separators), key order as captured:

- history or starting table: `{"query", "median", "p75", "n", "basis": "history|starting table", "tier": str|null, "mix": {confidence: count}, "confidence_label": "low"|null}`
- tag path (`--tags` matched): `{"query", "median", "p75", "n", "basis": "tags", "tier", "level", "confidence_label": "high|medium|low", "mix", "context": [factor, "learned|prior"], "bias": {"factor", "from": [..]}}`

`query` is `<family> · effort <E>[ · layer L][ · N task(s)][ · tags ...]` (the `·` is escaped as `\u00b7`). Fixtures: `estimate-history-json`, `estimate-tags-json`, `estimate-starting-table-opus-json`.

No-session paths (T004): `report`, `tasks` and `budget` over a slug with no sessions exit 0 and print zero totals; `budget` with no cards exits 1 (`budget: name the wave's cards with --cards ID,ID or --projected [model:]400k,...` on stderr); `report` with neither `--out` nor `--summary` and `tasks --json` without `--dry-run` exit 2. Fixtures: `budget-no-cards-fails`, `report-needs-out-or-summary`, `tasks-json-needs-dry-run`.

## Environment

`CLAUDE_CONFIG_DIR` (single config dir), `AI_GENT_HOME` (index location), `NO_COLOR` unused (plain text). No variable controls the clock. Tests use a hidden `--now ISO` flag on `tokens` (decision T003) for the "Generated" line, `snapshot` without `--at` and the budget windows. The Python tool has no such flag, so the fixtures write timestamps as `@NOW-<seconds>@`, expanded at run time, and both runners use the real clock.
