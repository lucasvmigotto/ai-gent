# Data model — pipeline pane

All state is `$.state` (session-scoped, survives a hot reload); nothing is written to disk.

| Entity | Fields | Source |
|---|---|---|
| Artifact | path, label, present, mtimeMs, commitDate (`%cs`, or null without git), stale (upstream path + date, or null) | `shared/pipeline.md` artifact table; `$.fs.stat`; `git log -1 --format=%cs -- <path>` |
| Feature | number (3 digits), name, priority, depends[], frontend, backend, qa (Planned · In progress · Partial · Implemented · Verified · N/A · —), notes | `specs/README.md` row, read as `specsync.py` reads it |
| Marker | kind (gap · clarification · assumption), file, line, text (first 80 chars) | `*.md` under `docs/product/` and `specs/` |
| Next | rule (1 · 2 · 3 · none), stage (`/project:init`, `/speckit-plan`, `backend:build`, …), reason, ref (`file:line` or feature number) | computed from the three above |
| Snapshot | artifacts[], features[], markers{counts, items[] capped}, next, computedAt, truncated | the pane's one drawn value |

## State atoms (`plugins/project/types/index.d.ts`)

| Key | Type | Written by |
|---|---|---|
| `pipeline.snapshot` | `Shaped<Snapshot>` (tag `pipeline-1`) | refresh (poll tick, tool.call after Edit/Write, command) |
| `pipeline.isOpen` | boolean | `/pipeline`, `ui.close` |
| `pipeline.expanded` | `StateFamily<boolean>` keyed by section | presses |

## Refresh

```
pane opens ──▶ full read ──▶ snapshot
   │ every N s: stat each watched path; changed mtimeMs ──▶ re-read that path ──▶ snapshot
   │ tool.call Edit/Write under docs/product/ or specs/ (after next) ──▶ re-read that path
pane closes ──▶ poll stopped
```
