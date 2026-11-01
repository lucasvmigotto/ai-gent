# Data model — card timeline pane

| Entity | Fields | Source |
|---|---|---|
| Ledger card | id, state, branch (optional), worktree (optional), projectedText, usedText | `.ai-gent/orchestrate-status.md` through `parseLedger` (014; `Branch` and `Worktree` are optional columns the skill does not define) |
| Transition | id, from, to, at (local time) | observed by the mod between two ledger reads; `$.state`, capped at 200 |
| Span | card, start, end (UTC), used, projected, agents, calls | `token_report.py tasks --dry-run --json` (new), only on Measure |
| Overlap | peak concurrency, per card the cards it overlaps | derived from the spans (time arithmetic only) |
| Snapshot | counts by state, rows, log, spans or null, measuredAt | derived |

## `tasks --dry-run --json` (prerequisite change)

```json
{"cards": [{"card": "004-us1-core", "feature": "004-x", "model": "sonnet",
            "projected": 420000, "used": 391000,
            "start": "2026-10-31T14:02:11Z", "end": "2026-10-31T14:41:50Z",
            "agents": 2, "calls": 114}],
 "unattributed": 1}
```

`--json` without `--dry-run` is rejected by the argument parser (exit 2); no file is written in either case. `start` and `end` carry a `Z`: the script's own times are UTC without a suffix, which JavaScript parses as local time. The mod passes `--project=<slug>` (cwd with characters outside `A-Za-z0-9-` replaced by `-`).

## State atoms (`plugins/agents/types/index.d.ts`)

| Key | Type | Purpose |
|---|---|---|
| `cardsOpen` | boolean | whether the pane is open (polls only then) |
| `cardsSeen` | Record<string, string> | each card's state at the last read, to detect transitions |
| `cardsLog` | `{id, from, to, at}`[] | the observed transitions, newest last, capped at 200 |
| `cardsMeasured` | the spans object or null | the last Measure result and when |
