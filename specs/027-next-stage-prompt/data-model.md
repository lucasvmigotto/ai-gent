# Data Model: Next-stage prompt

Atoms in `plugins/project/types/index.d.ts` (plugin `project`), shape-tagged; they live in `$.state`, so a hot reload keeps them.

| Atom | Value | Notes |
|---|---|---|
| `nextStageLast` | `{ rule: number \| null, stage: string, reason: string, ref?: string } \| null` | the last `snapshot.next` seen; seeded at `session.start` without a toast |
| `nextStageShown` | `string[]` | `stage + reason` keys already toasted this session, so the same recommendation is not repeated |
| `nextStageMtimes` | `Record<string, number>` | `mtimeMs` of the watched paths at the last turn end, for writes made by other means |

Toast wording: `<label> written — next: <stage> (<reason>)`; rule 1: `ask the user (<reason>) — <ref>`; no rule: `next: /project:status (<reason>)`. Always ends with `hint only — /project:status for the report`.
