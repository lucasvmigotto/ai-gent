# Data model — token burn rate

| Entity | Fields | Source |
|---|---|---|
| Rate | per_hour (billable tokens), window_minutes, messages; null when fewer than `MIN_RATE_MESSAGES` | `token_report.py budget --json` (new) |
| Cap ETA | eta_minutes (integer, or null: no cap, idle, or too few messages) | `budget --json`, `caps[].eta_minutes` (new) |
| ETA warning | cap name, under (boolean) | derived; `$.state` |

## `budget --json` additions (proposed)

```json
{"verdict": "proceed", "...": "014's keys unchanged",
 "rate": {"per_hour": 7400000, "window_minutes": 30, "messages": 112},
 "caps": [{"name": "session", "cap": 12000000, "used": 8900000, "left": 3100000, "eta_minutes": 25},
          {"name": "week", "cap": 60000000, "used": 38000000, "left": 22000000, "eta_minutes": 178}]}
```

`eta_minutes = round(left / per_hour * 60)`.

## `.ai-gent/budget.yaml` (one new key)

| Key | Default | Meaning |
|---|---|---|
| `rate_window_minutes` | 30 | the window the rate averages over |

## State atoms (`plugins/agents/types/index.d.ts`)

| Key | Type | Purpose |
|---|---|---|
| `tokenEtaWarned` | string[] | caps currently under the threshold; a toast fires only when a cap enters it |
