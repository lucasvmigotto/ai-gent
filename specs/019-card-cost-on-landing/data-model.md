# Data model — card cost on landing

| Entity | Fields | Source |
|---|---|---|
| Ledger card | id, state (`queued`, `running`, `review`, `landed`), projectedText, usedText (as written in the Tokens cell) | 014's `parseLedger` over `.ai-gent/orchestrate-status.md` |
| Landing news | card id, projectedText, usedText, percent (signed integer or null), hint (`over`, `under`, none) | derived from the ledger and the atoms below |
| Wave | landed (count), over (count), under (count) since the session started | `landingWave` atom |

## Ledger cell

`~420k / 391k` → projected `420k`, used `391k`; `~420k / —` → no used figure yet (no toast); `— / 391k` → no projection (toast without a percentage); `~0k` → no projection. The percentage is `round_half_even((used / projected − 1) × 100)` over the parsed cell values, shown with a sign.

## State atoms (`plugins/agents/types/index.d.ts`)

| Key | Type | Purpose |
|---|---|---|
| `landingSeen` | `Record<string, string>` | card id → the state last read; a transition into `landed` is news |
| `landingToasted` | string[] | card ids already toasted at their current landing; cleared for a card that leaves `landed` |
| `landingFirstLook` | string[] | card ids already `landed` at the first read of the session, never toasted |
| `landingWave` | `{ landed: number, over: number, under: number }` | the wave toast's counts; reset at session start |

All four survive a hot reload (they are `$.state`, not module variables), which the first-look and wave cases need.
