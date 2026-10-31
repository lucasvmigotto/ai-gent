# Data Model: Hand-off nudge

Atom in `plugins/project/types/index.d.ts` (plugin `project`), shape-tagged; lives in `$.state`, so a hot reload keeps it.

| Atom | Value | Notes |
|---|---|---|
| `handoffToasted` | `{ context: boolean, compact: boolean, caps: Record<string, boolean>, lastBudgetAt: number \| null }` | which conditions already toasted this session (a cap by its `name`), and when `budget` last ran, for the 5-minute throttle |

Inputs it does not store: `context.percent` (`$.session.usage()`), the `checkpoint` atom (012), the commit count and tree state (`git`).
