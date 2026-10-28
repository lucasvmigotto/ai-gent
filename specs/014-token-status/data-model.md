# Data model — token status

| Entity | Fields | Source |
|---|---|---|
| Ledger card | id (backticked cell), state (text), projected (`~420k`, or null), used (`391k`, or null) | `.ai-gent/orchestrate-status.md` card table; the Tokens cell `write_ledger` writes |
| Budget verdict | verdict (`proceed` · `shrink` · `stop`), why, fit (cards that fit), caps[] `{name: session·week, cap, used, left}`, cards[] `{label, model, projected, p75, confidence}` | `token_report.py budget --cards … --json` (new, FR-002) |
| Status | text (one line), warn (verdict ≠ proceed), refreshedAt | derived |

## `budget --json` (proposed)

```json
{"verdict": "shrink", "why": "only 2.1M usable; the first 1 card(s) fit (1.4M)",
 "fit": 1,
 "caps": [{"name": "session", "cap": 12000000, "used": 9100000, "left": 2900000},
          {"name": "week", "cap": 60000000, "used": 38000000, "left": 22000000}],
 "cards": [{"label": "004-a", "model": "sonnet", "projected": 420000, "p75": 550000, "confidence": "high"}],
 "batches": [["004-a"]]}
```

## State atoms (`plugins/agents/types/index.d.ts`)

| Key | Type | Purpose |
|---|---|---|
| `tokens.lastVerdict` | string or null | toast once per change |
| `tokens.toastedOverruns` | string[] | one overrun toast per card per session |
| `tokens.refreshedAt` | number | throttle (`$.clock` ms) |
