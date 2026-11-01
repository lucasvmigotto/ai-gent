# Data model — pipeline pane, dashboard layout

The domain data is 013's (`Snapshot`, unchanged; see `specs/013-pipeline-pane/data-model.md`). This file adds the presentation model, the display tables and the three new state atoms. Nothing is written to disk.

## Presentation model (`pipeline-view.ts`, pure; Planned)

```ts
type Tone = 'plain' | 'dim' | 'bold' | 'good' | 'warn' | 'bad' | 'info' // 013's six, plus info (active)

type Cell = { text: string; glyph?: string; tone?: Tone; align?: 'left' | 'right' }

type Column = { key: string; label: string; width?: number; align?: 'left' | 'right' } // no width = takes the rest

type Row = {
  key: string            // 'artifact:docs/product/brief.md', 'feature:018', 'marker:clarification:3'
  cells: Cell[]
  edge?: Tone            // the left-edge status mark (artifacts)
  detail?: Cell[][]      // lines shown when the row is expanded
  isOpen?: boolean
}

type Section = { id: string; title: string; columns: Column[]; rows: Row[]; empty?: string; more?: string }

type PipelineView = {
  mode: 'compact' | 'condensed' | 'full'
  header: { title: 'PIPELINE'; badge: 'LIVE' | 'STALE'; caption: string }
  summary: { artifacts: number; ofArtifacts: number; features: number; clarifications: number; assumptions: number; gaps: number }
  sections: Section[]           // artifacts, stages, progress, features, items
  progress: { implemented: number; planned: number; other: number }
  stages: { key: string; label: string; state: 'done' | 'current' | 'pending' }[]
  filter: { active: 'all' | 'ready' | 'blocked' | 'done'; counts: Record<string, number> }
  next: { badge: 'Needs attention' | 'Suggested' | 'Up to date'; text: string; ref?: string }
}

// build(snapshot, ctx) → PipelineView
type Context = { columns: number; expanded: Record<string, boolean>; filter: string; checkedAt: number; now: number; pollMs: number; ascii: boolean; maxRows: number }
```

`build` reads nothing but its arguments. `view(snapshot, columns, expanded)` and `text(snapshot)` (013's compact lines and headless text) stay exported and unchanged; the compact mode is that output.

## Modes

| `e.props.bodyColumns` | Mode | Summary | Artifacts table | Matrix |
|---|---|---|---|---|
| under 40 | `compact` | 013's lines | 013's lines | 013's lines (`NNN name P/W/I`) |
| 40 to 90 | `condensed` | one line: `5 ready · 28 features · 10 clarifications · 5 assumptions · 2 gaps` | Artifact, Status (no Stage) | ID, Feature, PRI, READY, glyph-only status cells, one-line legend |
| over 90 | `full` | three large metrics, two small | Artifact, Stage, Status | ID, Feature, PRI, READY, a 13-cell column per side |

## Status display (Planned for the new glyphs; the proposal's eight keep its pairs)

| Meaning | Source | Word | Unicode | ASCII | Tone (theme key) |
|---|---|---|---|---|---|
| Verified | `word()` = verified | Verified | `✓` | `+` | good (`success`) |
| Implemented | implemented | Implemented | `●` (proposed) | `#` (proposed) | good |
| In progress | in progress | In progress | `◉` | `*` | info (`suggestion`) |
| Partial | partial | Partial | `◐` (proposed) | `/` (proposed) | info |
| Planned | planned | Planned | `○` | `o` | warn (`warning`) |
| N/A | `n/a` | N/A | `—` | `-` | dim (`inactive`) |
| Not set | empty, `—` | (none) | `—` | `-` | dim |
| Retired | retired and its synonyms | Retired | `—` | `-` | dim |
| Blocked | `bucket: blocked` | blocked | `⏸` | `!` | bad (`error`) |
| Ready | `bucket: ready` | ready | `▶` (013's mark) | (word only) | info |
| Done | `bucket: done` | done | (word only) | (word only) | dim |
| Missing | `isProblem` | Missing | `✕` | `x` | bad |
| Stale | `staleSince` | Stale | `↻` | `~` | warn |
| Ready (artifact) | present, not stale | Ready | `✓` | `+` | good |
| Pending (artifact) | absent, not a problem | Pending | `·` (013's mark) | `.` | dim |
| Current stage | strip | (the label, bold) | `◆` | `>` | info |
| Dependency | expanded row | `↳ 002 name` | `↳` | `-` | dim |

Notes. **Readiness and status never share a glyph or a column**: READY uses words (`ready`, `blocked`, `done`) with `▶` and `⏸` only beside the word in full mode, and `✓` belongs to status and artifact cells, so `done` carries no `✓`. The artifact `Pending` state uses 013's dim `·` (ASCII `.`) rather than `○`, which is the Planned glyph; and the readiness `ready` mark `▶` falls back to the word alone in ASCII, because the proposal's `>` is the current-stage fallback. A cell with no status draws `—` or `-`, never markup text (image 3's `secondary>-`).

## Matrix columns (Planned)

| Column | Full | Condensed | Align | Source |
|---|---|---|---|---|
| ID | 3 | 3 | left | `Feature.number` |
| FEATURE | rest, min 12, cut with `…` | rest, min 8 | left | `Feature.name` |
| PRI | 3 | 3 | right | `Feature.priority` |
| READY | 7 | 7 | left | `Feature.bucket` |
| one per key of `Feature.sides` (F, B, Q here; STATUS for a single-status README) | 13 | 1 (glyph) | left | the side's cell, through `word()` |

Gaps are 2 cells (full) and 1 (condensed). A 1-cell left-edge mark precedes artifact rows only.

## Stage names (Planned; `pipeline-model.ts`, keyed by `ArtifactSpec.stage`)

| Owning command | Stage name |
|---|---|
| `/project:init` | Project init |
| `/project:architecture` | Architecture |
| `/frontend:uiux` | UI/UX |
| `/project:spec` | Specification |
| `/backend:domain` | Backend (proposed; `Domain model` is more exact) |
| `/backend:spec` | API specification |
| `/qa:strategy` | QA strategy |
| `/devsecops:pipeline` | DevOps |

`scripts/consistency.py` fails when a `stage` in `ARTIFACTS` has no name here.

## Feature progress buckets (Planned)

| Bucket | `Feature.state` |
|---|---|
| Implemented | `review`, `done` (least advanced side Implemented or Verified) |
| Planned | `todo` |
| Other | `doing`, `retired` |

The three counts sum to the README rows.

## Stage progression (Planned; an aid, never read by `recommend`)

| Step | `done` when |
|---|---|
| Init | `docs/product/brief.md` present |
| Architecture | `docs/product/architecture.md` present |
| Spec | `specs/README.md` present |
| Build | the README has rows and every non-retired feature's `bucket` is `done` |
| QA | `docs/product/test-strategy.md` present and every non-retired feature that has a QA cell has it at Verified |
| Delivery | `docs/product/delivery.md` present |

The current step is the first that is not done; later ones are pending. `ux-vision`, `domain-model` and `contract` have no step; they show in the table only.

## Region to element mapping (`pipeline-draw.tsx`; native elements only)

| Region | Elements and props |
|---|---|
| Pane body | `Box flexDirection="column" gap={1}` |
| Header | `Box justifyContent="space-between"`: `Text bold` title; a `Box` with the badge `Text` (`color` tone, `backgroundColor` only on surfaces that paint it, the word always present) and the caption `Text dimColor` |
| Large metrics | `Box gap={4}` of three `Box flexDirection="column"`: a `Text bold size="title"` number (`size` ignored on the terminal) over a `Text dimColor` caption; two small metrics follow in a `Box` with `Text dimColor` |
| Table | a header `Box` of `Text bold dimColor` cells; each row a `Box` of cells, each cell a `Box width={column.width}` (or `flexGrow={1}` for the rest) holding a `Text`; right-aligned cells use `justifyContent="flex-end"` |
| Left-edge mark | a 1-cell `Box width={1} backgroundColor={tone}` before the row, plus the glyph in the status cell (mark is decoration) |
| Status pill | `Text color={tone}` with the glyph and word, padded with spaces; `backgroundColor` only where painted |
| Row press | the row's label cell is a `Button` (`onPress` writes the expanded atom); nothing relies on hover |
| Bar | `Box width="100%"` (a row) of segment `Box flexGrow={count} minWidth={1} backgroundColor={tone} height={1}`; fallback `Text` runs of `█` |
| Stage strip | a row `Box gap={1}` of six `Box flexGrow={1}` each a thin `Box height={1} backgroundColor={tone}` over a `Text` label (current in bold with `◆` or `>`) |
| Filters | a row `Box gap={1}` of four `Button`s; the active one marked `●`/`*` before its label |
| Next action | `Box flexDirection="column"`: a `Box justifyContent="space-between"` with the title and badge, the hint `Text`, then `Text dimColor` `Hint only · Full report:` and a `Button` `/project:status` |
| Section separation | a `Text dimColor` rule or `borderStyle="single"` only on the top edge where the surface supports it; no card borders |

## State atoms (`plugins/project/types/index.d.ts`, Planned additions)

| Key | Type | Written by |
|---|---|---|
| `pipeline.filter` | `'all' \| 'ready' \| 'blocked' \| 'done'` (default `all`) | filter buttons |
| `pipeline.checkedAt` | number (ms; 0 until the first check) | every poll tick, the Edit/Write refresh, the command |
| `pipeline.pollMs` | number (the running interval; 0 when the poll is stopped) | `open`, `close`, `stop` in `pipeline.ts` |

013's `pipelineSnapshot`, `pipelineIsOpen` and `pipelineExpanded` are kept. `pipelineExpanded` gains keys by prefix: `artifact:<path>`, `feature:<number>`, `markers:clarification`, `markers:assumption`, `markers:gap`. `Snapshot` is unchanged, so its shape tag (`pipeline-1`) is unchanged; the three keys are new, so no stored value is misread.

## LIVE verdict

```
badge = pollMs > 0 && now - checkedAt <= 3 * pollMs ? 'LIVE' : 'STALE'
caption = badge == 'LIVE' ? `Checked ${hh:mm:ss} · every ${pollMs/1000} s` : `Last checked ${hh:mm:ss}`   (checkedAt 0: 'Not checked yet')
```

## userConfig (`plugins/project/.claude-plugin/plugin.json`, Planned additions)

| Option | Type | Default | Meaning |
|---|---|---|---|
| `pipelineLayout` | string | `dashboard` | `dashboard`, or `compact` (013's view); any other value reads as `dashboard` |
| `pipelineAscii` | boolean | false | force the ASCII glyph set |
| `pipelineMaxRows` | number | 60 | matrix rows drawn before `+N more` |
