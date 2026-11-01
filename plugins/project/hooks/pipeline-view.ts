// The pipeline pane as lines: what to say, in what tone, with which button.
// Pure, so the layout (wide, narrow, headless text) is tested without a surface.

import type { Feature, MarkerKind, Snapshot } from '../types'
import { STAGE_NAME, letter, progress, stageStrip, word } from './pipeline-model'
import type { StageStep } from './pipeline-model'

// 013's six, plus `info` (an active state) for the dashboard.
export type Tone = 'plain' | 'dim' | 'bold' | 'good' | 'warn' | 'bad' | 'info'

export type Line = {
  key: string
  text: string
  tone: Tone
  // A button beside the text; `id` says what it does (hooks/register.tsx).
  button?: { id: 'markers' | 'status'; label: string }
}

// Under this many columns the features table is `NNN name P/W/I`.
export const NARROW = 40
const SHOWN_MARKERS = 20

const SIDE_LABEL: Record<string, string> = { frontend: 'F', backend: 'B', status: 'S', qa: 'Q' }

function fit(text: string, columns: number): string {
  return text.length > columns ? `${text.slice(0, Math.max(1, columns - 1))}…` : text
}

export function view(snapshot: Snapshot, columns: number, expanded: Readonly<Record<string, boolean>>): Line[] {
  const lines: Line[] = []
  const add = (text: string, tone: Tone = 'plain', button?: Line['button']): void => {
    lines.push({ key: `l${lines.length}`, text: fit(text, columns - (button ? button.label.length + 3 : 0)), tone, button })
  }
  const narrow = columns < NARROW

  add('Pipeline', 'bold')
  if (snapshot.artifacts.every(a => !a.isPresent)) {
    add('No pipeline artifacts yet. Start with /project:init.', 'warn')
  } else {
    add('Artifacts', 'bold')
    for (const a of snapshot.artifacts) {
      if (a.isPresent && a.staleSince) add(`~ ${a.label}: older than ${a.staleSince.path} (${a.staleSince.date})`, 'warn')
      else if (a.isPresent) add(`✓ ${a.label}${narrow ? '' : `  ${a.path}`}`, 'good')
      else if (a.isProblem) add(`✗ ${a.label}: missing, a later stage already ran`, 'bad')
      else add(`· ${a.label}`, 'dim')
    }
    if (!snapshot.hasGit) add('Staleness needs git.', 'dim')
  }

  add('Features', 'bold')
  if (!snapshot.hasReadme) {
    add('No specs/README.md yet.', 'dim')
  } else if (snapshot.features.length === 0) {
    add('specs/README.md lists no feature.', 'dim')
  }
  for (const f of snapshot.features) {
    const mark = f.bucket === 'done' ? '✓' : f.bucket === 'ready' ? '▶' : '⏸'
    const tone: Tone = f.bucket === 'done' ? 'dim' : f.bucket === 'ready' ? 'bold' : 'plain'
    const sides = Object.entries(f.sides)
    if (narrow) {
      add(`${mark} ${f.number} ${f.name} ${sides.map(([, v]) => letter(v)).join('/')}`, tone)
    } else {
      const cells = sides.map(([k, v]) => `${SIDE_LABEL[k] ?? k}:${v || '—'}`).join(' ')
      const waits = f.waiting.length > 0 ? ` · waits for ${f.waiting.join(', ')}` : ''
      add(`${mark} ${f.number} ${f.name}  ${f.priority}  ${cells}${waits}`, tone)
    }
  }

  const { counts, items, isTruncated } = snapshot.markers
  const isOpen = expanded.markers === true
  const many = (n: number, one: string): string => `${n} ${one}${n === 1 ? '' : 's'}`
  add(
    `${many(counts.clarification, 'clarification')} · ${many(counts.assumption, 'assumption')} · ${many(counts.gap, 'gap')}${isTruncated ? ' (more not counted)' : ''}`,
    counts.clarification > 0 ? 'warn' : 'plain',
    items.length > 0 ? { id: 'markers', label: isOpen ? 'Hide' : 'Show' } : undefined,
  )
  if (isOpen) {
    for (const m of items.slice(0, SHOWN_MARKERS)) add(`  ${m.file}:${m.line} ${m.text}`, 'dim')
    if (items.length > SHOWN_MARKERS) add(`  +${items.length - SHOWN_MARKERS} more`, 'dim')
  }

  const { next } = snapshot
  add(`Next (a hint): ${next.stage}, ${next.reason}${next.ref ? ` [${next.ref}]` : ''}`, 'bold', { id: 'status', label: '/project:status' })
  return lines
}

// What `/pipeline` prints where no surface can draw the pane.
export function text(snapshot: Snapshot): string {
  return view(snapshot, 200, { markers: true })
    .map(line => line.text)
    .join('\n')
}

// ------------------------------------------------------------ the dashboard
// A structured model of the pane (spec 029): a pure function of the snapshot
// and a small context. `view` and `text` above stay the compact and headless output.

export type Cell = { text: string; glyph?: string; tone?: Tone; align?: 'left' | 'right' }
export type Column = { key: string; label: string; width: number; align?: 'left' | 'right' }
export type Row = {
  key: string
  cells: Cell[]
  // The left-edge status mark (artifact rows).
  edge?: Tone
  // A dim line under the row (why an artifact is stale).
  sub?: string
  tone?: Tone
  // Lines shown when the row is open (open items).
  detail?: Cell[][]
  isOpen?: boolean
}
export type Section = { id: string; title: string; columns: Column[]; rows: Row[]; empty?: string; more?: string }
export type Mode = 'compact' | 'condensed' | 'full'
export type Metric = { key: string; value: number; label: string; isLarge: boolean; tone?: Tone }
export type Filter = 'all' | 'ready' | 'blocked' | 'done'
export type BarSegment = { key: string; glyph: string; tone: Tone; length: number }

export type PipelineView = {
  mode: Mode
  // Which columns the feature matrix has at this width.
  matrix: { tier: Tier; words: boolean }
  // The Features section is collapsed until the person opens it (the `features` key of the expanded atom).
  featuresOpen: boolean
  ascii: boolean
  // No artifact exists yet: the dashboard is not drawn, 013's lines say so.
  isEmpty: boolean
  header: { title: 'PIPELINE'; badge: 'LIVE' | 'STALE'; caption: string }
  summary: { artifacts: number; ofArtifacts: number; features: number; clarifications: number; assumptions: number; gaps: number; metrics: Metric[]; line: string }
  sections: Section[]
  progress: {
    implemented: number
    planned: number
    other: number
    total: number
    // Implemented as a whole percent of total; the counts line says what it is made of.
    percent: number
    line: string
    width: number
    bar: BarSegment[]
    pills: { key: string; text: string; tone: Tone }[]
  }
  stages: (StageStep & { glyph: string; tone: Tone })[]
  filter: { active: Filter; counts: Record<Filter, number> }
  next: {
    badge: 'Needs attention' | 'Suggested' | 'Up to date'
    // The recommendation as `recommend` made it: a title (its stage), why, what it points at.
    title: string
    explanation?: string
    ref?: string
    // The recommended command, when the stage is one; the button that fills it.
    action?: string
    // `${stage}: ${reason}`, the one-line form.
    text: string
    hasNext: boolean
    hint: string
    command: string
  }
  hasGit: boolean
}

export type Context = {
  columns: number
  expanded: Readonly<Record<string, boolean>>
  filter: string
  // When the poll last checked, the time to judge it against, and the interval (0: not polling).
  checkedAt: number
  now: number
  pollMs: number
  ascii: boolean
  maxRows: number
  // Widest the progress bar may be; unset, it fills the width (the drawn pane). The plain text caps it.
  barMax?: number
}

// Under NARROW the compact text; through WIDE the condensed grid; above it the full matrix.
export const WIDE = 90

// The plain text keeps its bar at most this wide; the drawn pane's bar fills the row.
const TEXT_BAR_MAX = 60

// One table of glyphs: Unicode first, its ASCII fallback second. Every glyph
// has a word or a legend beside it; colour never stands alone.
export const GLYPH = {
  verified: ['✓', '+'],
  implemented: ['●', '#'],
  progress: ['◉', '*'],
  partial: ['◐', '/'],
  planned: ['○', 'o'],
  none: ['—', '-'],
  missing: ['✕', 'x'],
  stale: ['↻', '~'],
  ready: ['✓', '+'],
  pending: ['·', '.'],
  current: ['◆', '>'],
  sep: ['·', '|'],
  ellipsis: ['…', '...'],
  edge: ['▎', '|'],
  branch: ['└─', '`-'],
  dep: ['↳', '-'],
  closed: ['▸', '>'],
  open: ['▾', 'v'],
  barDone: ['█', '#'],
  barPlanned: ['▒', '='],
  barOther: ['░', '-'],
} as const satisfies Record<string, readonly [string, string]>

type GlyphKey = keyof typeof GLYPH
const glyph = (key: GlyphKey, ascii: boolean): string => GLYPH[key][ascii ? 1 : 0]

// Cut to `width` with an ellipsis. With `smart`, cut after the last word
// break (-, _, space) when that keeps most of the width, so a name ends on a
// whole word instead of mid-word.
function cut(text: string, width: number, ascii: boolean, smart = false): string {
  const chars = [...text]
  if (chars.length <= width) return text
  const dots = glyph('ellipsis', ascii)
  const keep = Math.max(1, width - [...dots].length)
  let head = chars.slice(0, keep)
  if (smart) {
    // A break right after the kept part ends on a whole word already.
    if (!'-_ '.includes(chars[keep] ?? 'x')) {
      for (let i = keep - 1; i >= keep * 0.7 && i > 0; i -= 1) {
        if ('-_ '.includes(chars[i] as string)) {
          head = chars.slice(0, i)
          break
        }
      }
    }
  }
  return `${head.join('')}${dots}`
}

// The feature matrix's columns, chosen from the width actually available:
// ID, Feature and Status first (narrow); then PRI, READY and the frontend and
// backend sides (medium); then every side (wide), with words where they fit.
export type Tier = 'narrow' | 'medium' | 'wide'
export const FEATURE_MIN = 24
const STATUS_WIDTH = 13
const SIDE_PRIORITY = ['frontend', 'backend', 'status', 'qa']

export function matrixPlan(columns: number, sides: readonly string[], gap: number): { tier: Tier; sides: string[]; words: boolean } {
  const ordered = SIDE_PRIORITY.filter(side => sides.includes(side))
  const needs = (widths: number[]): number => widths.reduce((a, b) => a + b, 0) + gap * (widths.length - 1)
  const base = [3, FEATURE_MIN, 3, 7]
  const wide = (cell: number): boolean => needs([...base, ...ordered.map(() => cell)]) <= columns
  if (ordered.length > 0 && wide(STATUS_WIDTH)) return { tier: 'wide', sides: ordered, words: true }
  if (ordered.length > 0 && wide(1)) return { tier: 'wide', sides: ordered, words: false }
  const medium = ordered.filter(side => side === 'frontend' || side === 'backend')
  const kept = medium.length > 0 ? medium : ordered.slice(0, 1)
  if (needs([...base, ...kept.map(() => 1)]) <= columns) return { tier: 'medium', sides: kept, words: false }
  return { tier: 'narrow', sides: [], words: true }
}

const DISPLAY: Record<string, string> = {
  planned: 'Planned',
  'in progress': 'In progress',
  partial: 'Partial',
  implemented: 'Implemented',
  verified: 'Verified',
}
const STATUS_GLYPH: Record<string, [GlyphKey, Tone]> = {
  planned: ['planned', 'warn'],
  'in progress': ['progress', 'info'],
  partial: ['partial', 'info'],
  implemented: ['implemented', 'good'],
  verified: ['verified', 'good'],
}

// A README status cell as a glyph, its normalised word and a tone: the full
// vocabulary stays distinct, and an empty cell is a plain dash.
export function statusCell(value: string, ascii: boolean): Cell {
  const w = word(value)
  const known = STATUS_GLYPH[w]
  if (known) return { glyph: glyph(known[0], ascii), text: DISPLAY[w] ?? w, tone: known[1] }
  if (w !== '') return { glyph: glyph('none', ascii), text: 'Retired', tone: 'dim' }
  if (/n\/a/i.test(value)) return { glyph: glyph('none', ascii), text: 'N/A', tone: 'dim' }
  return { text: glyph('none', ascii), tone: 'dim' }
}

function plural(n: number, one: string, many = `${one}s`): string {
  return n === 1 ? one : many
}

function clock(ms: number): string {
  const d = new Date(ms)
  const two = (n: number): string => String(n).padStart(2, '0')
  return `${two(d.getHours())}:${two(d.getMinutes())}:${two(d.getSeconds())}`
}

const FILTERS: Filter[] = ['all', 'ready', 'blocked', 'done']
const SIDE_ORDER = ['frontend', 'backend', 'status', 'qa']
const SIDE_HEAD: Record<string, string> = { frontend: 'FRONTEND', backend: 'BACKEND', status: 'STATUS', qa: 'QA' }
const KINDS: { kind: MarkerKind; title: string }[] = [
  { kind: 'clarification', title: 'Clarifications' },
  { kind: 'assumption', title: 'Assumptions' },
  { kind: 'gap', title: 'Gaps' },
]
// The word a feature's overall state is shown as when only one status fits.
const STATE_WORD: Record<Feature['state'], string> = { todo: 'planned', doing: 'in progress', review: 'implemented', done: 'implemented', retired: '' }
const READY_TONE: Record<Feature['bucket'], Tone> = { ready: 'info', blocked: 'bad', done: 'dim' }

// Segment lengths of a bar `width` cells wide: every non-zero count gets at
// least one cell, the rest by largest remainder.
function barLengths(counts: number[], width: number): number[] {
  const total = counts.reduce((a, b) => a + b, 0)
  if (total === 0) return counts.map(() => 0)
  const raw = counts.map(c => (c / total) * width)
  const lengths = raw.map((r, i) => ((counts[i] ?? 0) > 0 ? Math.max(1, Math.floor(r)) : 0))
  let diff = width - lengths.reduce((a, b) => a + b, 0)
  while (diff !== 0) {
    const step = diff > 0 ? 1 : -1
    const order = lengths
      .map((len, i) => ({ i, gap: (raw[i] ?? 0) - len, len }))
      .filter(x => (step > 0 ? (counts[x.i] ?? 0) > 0 : x.len > 1))
      .sort((a, b) => (step > 0 ? b.gap - a.gap : a.gap - b.gap))
    const pick = order[0]
    if (!pick) break
    lengths[pick.i] = (lengths[pick.i] ?? 0) + step
    diff -= step
  }
  return lengths
}

export function build(snapshot: Snapshot, ctx: Context): PipelineView {
  const { ascii } = ctx
  const mode: Mode = ctx.columns < NARROW ? 'compact' : ctx.columns <= WIDE ? 'condensed' : 'full'
  const isFull = mode === 'full'
  const gap = isFull ? 2 : 1
  const sep = ` ${glyph('sep', ascii)} `
  const fill = (widths: (number | null)[], min: number, reserved = 0): number[] => {
    const fixed = widths.reduce<number>((a, b) => a + (b ?? 0), 0)
    const rest = Math.max(min, ctx.columns - reserved - fixed - gap * (widths.length - 1))
    return widths.map(w => w ?? rest)
  }

  // ---- summary
  const { counts } = snapshot.markers
  const ready = snapshot.artifacts.filter(a => a.isPresent && !a.staleSince).length
  const metrics: Metric[] = [
    { key: 'artifacts', value: ready, label: 'Artifacts ready', isLarge: true },
    { key: 'features', value: snapshot.features.length, label: plural(snapshot.features.length, 'Feature'), isLarge: true },
    { key: 'clarification', value: counts.clarification, label: plural(counts.clarification, 'Clarification'), isLarge: true, tone: counts.clarification > 0 ? 'warn' : 'dim' },
    { key: 'assumption', value: counts.assumption, label: plural(counts.assumption, 'Assumption'), isLarge: false },
    { key: 'gap', value: counts.gap, label: plural(counts.gap, 'Gap'), isLarge: false },
  ]
  const line = [
    `${ready} ready`,
    `${snapshot.features.length} ${plural(snapshot.features.length, 'feature')}`,
    `${counts.clarification} ${plural(counts.clarification, 'clarification')}`,
    `${counts.assumption} ${plural(counts.assumption, 'assumption')}`,
    `${counts.gap} ${plural(counts.gap, 'gap')}`,
  ].join(sep)

  // ---- artifacts
  const artifactWidths = fill(isFull ? [null, 18, 10] : [null, 10], 8, 2)
  const artifactColumns: Column[] = (isFull
    ? [['artifact', 'ARTIFACT'], ['stage', 'STAGE'], ['status', 'STATUS']]
    : [['artifact', 'ARTIFACT'], ['status', 'STATUS']]
  ).map(([key, label], i) => ({ key: key as string, label: label as string, width: artifactWidths[i] ?? 0 }))
  const artifactRows: Row[] = snapshot.artifacts.map(a => {
    const [name, key, tone]: [string, GlyphKey, Tone] =
      a.isPresent && a.staleSince ? ['Stale', 'stale', 'warn']
      : a.isPresent ? ['Ready', 'ready', 'good']
      : a.isProblem ? ['Missing', 'missing', 'bad']
      : ['Pending', 'pending', 'dim']
    const status: Cell = { text: name, glyph: glyph(key, ascii), tone }
    const label: Cell = { text: cut(a.label, artifactWidths[0] ?? 8, ascii), tone: a.isPresent ? 'plain' : 'dim' }
    const stage: Cell = { text: cut(STAGE_NAME[a.stage] ?? a.stage, 18, ascii), tone: 'dim' }
    const rowKey = `artifact:${a.path}`
    const branch = glyph('branch', ascii)
    const sub = a.isPresent && a.staleSince
      ? `${branch} Older than ${a.staleSince.path} (${a.staleSince.date})`
      : !a.isPresent && a.isProblem
        ? `${branch} A later stage already ran`
        : undefined
    const isOpen = ctx.expanded[rowKey] === true
    const pair = (name: string, value: string): Cell[] => [{ text: name, tone: 'dim' }, { text: value }]
    const detail: Cell[][] = [
      pair('Path', a.path),
      pair('Command', a.stage),
      ...(a.commitDate ? [pair('Committed', a.commitDate)] : []),
      ...(a.staleSince ? [pair('Upstream', `${a.staleSince.path} (${a.staleSince.date})`)] : []),
    ]
    return {
      key: rowKey,
      cells: isFull ? [label, stage, status] : [label, status],
      edge: tone,
      ...(sub ? { sub } : {}),
      isOpen,
      ...(isOpen ? { detail } : {}),
    }
  })
  const hasArtifacts = snapshot.artifacts.some(a => a.isPresent)

  // ---- features
  const active = (FILTERS as string[]).includes(ctx.filter) ? (ctx.filter as Filter) : 'all'
  const filterCounts: Record<Filter, number> = {
    all: snapshot.features.length,
    ready: snapshot.features.filter(f => f.bucket === 'ready').length,
    blocked: snapshot.features.filter(f => f.bucket === 'blocked').length,
    done: snapshot.features.filter(f => f.bucket === 'done').length,
  }
  const present = SIDE_ORDER.filter(side => snapshot.features.some(f => side in f.sides))
  const plan = matrixPlan(ctx.columns, present, gap)
  const { sides, words } = plan
  const narrow = plan.tier === 'narrow'
  const sideWidth = words ? STATUS_WIDTH : 1
  const featureWidths = narrow
    ? fill([3, null, STATUS_WIDTH], 8)
    : fill([3, null, 3, 7, ...sides.map(() => sideWidth)], FEATURE_MIN)
  const nameWidth = featureWidths[1] ?? FEATURE_MIN
  const featureColumns: Column[] = narrow
    ? [
        { key: 'id', label: 'ID', width: 3 },
        { key: 'name', label: 'FEATURE', width: nameWidth },
        { key: 'status', label: 'STATUS', width: STATUS_WIDTH },
      ]
    : [
        { key: 'id', label: 'ID', width: 3 },
        { key: 'name', label: 'FEATURE', width: nameWidth },
        { key: 'priority', label: 'PRI', width: 3, align: 'right' },
        { key: 'ready', label: 'READY', width: 7 },
        ...sides.map((side, i) => ({ key: side, label: words ? (SIDE_HEAD[side] ?? side.toUpperCase()) : (side[0] ?? '').toUpperCase(), width: featureWidths[4 + i] ?? 0 })),
      ]
  const shown = snapshot.features.filter(f => active === 'all' || f.bucket === active)
  const byNumber = new Map(snapshot.features.map(f => [f.number, f]))
  const featureRows: Row[] = shown.slice(0, Math.max(0, ctx.maxRows)).map(f => {
    const isRetired = f.state === 'retired'
    const rowTone: Tone | undefined = isRetired || f.bucket === 'done' ? 'dim' : undefined
    const key = `feature:${f.number}`
    const isOpen = ctx.expanded[key] === true
    const pair = (name: string, value: string): Cell[] => [{ text: name, tone: 'dim' }, { text: value }]
    const detail: Cell[][] = [
      [{ text: f.name }],
      ...Object.entries(f.sides).map(([side, value]) => pair(SIDE_HEAD[side] ?? side, value || glyph('none', ascii))),
      ...(f.depends.length > 0
        ? [
            [{ text: 'Depends on', tone: 'dim' as Tone }],
            ...f.depends.map(n => [{ text: `${glyph('dep', ascii)} ${n}${byNumber.get(n) ? ` ${byNumber.get(n)?.name}` : ''}${f.waiting.includes(n) ? ' (waits for)' : ''}` }]),
          ]
        : []),
      pair('Files', (['spec', 'plan', 'tasks'] as const).map(n => `${n}.md ${f.files[n] ? glyph('ready', ascii) : glyph('missing', ascii)}`).join(` ${glyph('sep', ascii)} `)),
    ]
    const overall = isRetired ? statusCell('retired', ascii) : statusCell(STATE_WORD[f.state], ascii)
    return {
      key,
      ...(isRetired ? { tone: 'dim' as const } : {}),
      isOpen,
      ...(isOpen ? { detail } : {}),
      cells: [
        { text: f.number, tone: rowTone },
        { text: cut(f.name, nameWidth, ascii, true), tone: rowTone },
        ...(narrow
          ? [overall]
          : [
              { text: f.priority, tone: rowTone, align: 'right' as const },
              { text: f.bucket, tone: READY_TONE[f.bucket] },
              ...sides.map(side => {
                const cell = statusCell(f.sides[side] ?? '', ascii)
                // Where words do not fit: the glyph alone, under a legend.
                return words ? cell : { text: cell.glyph ?? cell.text, tone: cell.tone }
              }),
            ]),
      ],
    }
  })
  const featureEmpty = !snapshot.hasReadme
    ? 'No specs/README.md yet.'
    : snapshot.features.length === 0
      ? 'specs/README.md lists no feature.'
      : shown.length === 0
        ? `No ${active} features.`
        : undefined
  const hidden = shown.length - featureRows.length

  // ---- open items
  const { items } = snapshot.markers
  const itemRows: Row[] = KINDS.map(({ kind, title }) => {
    const count = counts[kind]
    const isOpen = ctx.expanded[`markers:${kind}`] === true || ctx.expanded.markers === true
    const row: Row = {
      key: `marker:${kind}`,
      cells: [{ text: title }, { text: String(count), tone: count > 0 ? (kind === 'clarification' ? 'warn' : 'plain') : 'dim', align: 'right' }],
      isOpen,
    }
    if (!isOpen) return row
    const of = items.filter(m => m.kind === kind)
    row.detail = of.slice(0, SHOWN_MARKERS).map(m => [{ text: `${m.file}:${m.line} ${m.text}`, tone: 'dim' as Tone }])
    if (of.length > SHOWN_MARKERS) row.detail.push([{ text: `+${of.length - SHOWN_MARKERS} more`, tone: 'dim' }])
    return row
  })

  // ---- progress
  const counted = progress(snapshot.features)
  const total = counted.implemented + counted.planned + counted.other
  const barWidth = Math.min(ctx.barMax ?? Infinity, Math.max(10, ctx.columns - 2))
  const lengths = barLengths([counted.implemented, counted.planned, counted.other], barWidth)
  const segments: [string, GlyphKey, Tone, string][] = [
    ['implemented', 'barDone', 'good', 'Implemented'],
    ['planned', 'barPlanned', 'warn', 'Planned'],
    ['other', 'barOther', 'dim', 'Other'],
  ]
  const bar: BarSegment[] = segments
    .map(([key, g, tone], i) => ({ key, glyph: glyph(g, ascii), tone, length: lengths[i] ?? 0 }))
    .filter(seg => seg.length > 0)
  const percent = total === 0 ? 0 : Math.round((counted.implemented * 100) / total)
  const pills = segments.map(([key, , tone, label]) => {
    const n = counted[key as keyof typeof counted]
    return { key, text: `${n} ${label}`, tone: n > 0 ? tone : ('dim' as Tone) }
  })

  // ---- the rest
  const stages = stageStrip(snapshot.artifacts, snapshot.features).map(step => ({
    ...step,
    glyph: glyph(step.state === 'done' ? 'ready' : step.state === 'current' ? 'current' : 'pending', ascii),
    tone: (step.state === 'done' ? 'good' : step.state === 'current' ? 'info' : 'dim') as Tone,
  }))
  const next: Snapshot['next'] | undefined = snapshot.next
  const isLive = ctx.pollMs > 0 && ctx.checkedAt > 0 && ctx.now - ctx.checkedAt <= 3 * ctx.pollMs
  const caption = ctx.checkedAt <= 0
    ? 'Not checked yet'
    : isLive
      ? `Checked ${clock(ctx.checkedAt)}${sep}every ${ctx.pollMs / 1000} s`
      : `Last checked ${clock(ctx.checkedAt)}`

  return {
    mode,
    matrix: { tier: plan.tier, words },
    featuresOpen: ctx.expanded.features === true,
    ascii,
    isEmpty: !hasArtifacts,
    header: { title: 'PIPELINE', badge: isLive ? 'LIVE' : 'STALE', caption },
    summary: {
      artifacts: ready,
      ofArtifacts: snapshot.artifacts.length,
      features: snapshot.features.length,
      clarifications: counts.clarification,
      assumptions: counts.assumption,
      gaps: counts.gap,
      metrics,
      line,
    },
    sections: [
      { id: 'artifacts', title: 'Artifacts', columns: artifactColumns, rows: artifactRows },
      {
        id: 'features',
        title: 'Features',
        columns: featureColumns,
        rows: featureRows,
        ...(featureEmpty ? { empty: featureEmpty } : {}),
        ...(hidden > 0 ? { more: `+${hidden} more; filter to see them` } : {}),
      },
      { id: 'items', title: 'Open items', columns: [{ key: 'kind', label: 'ITEM', width: 0 }, { key: 'count', label: 'COUNT', width: 0, align: 'right' }], rows: itemRows },
    ],
    progress: { ...counted, total, percent, line: pills.map(p => p.text).join(sep), width: barWidth, bar, pills },
    stages,
    filter: { active, counts: filterCounts },
    next: {
      badge: next?.rule === 1 ? 'Needs attention' : next && next.rule !== null ? 'Suggested' : 'Up to date',
      title: next ? (STAGE_NAME[next.stage] ?? `${next.stage.slice(0, 1).toUpperCase()}${next.stage.slice(1)}`) : 'No recommendation yet',
      ...(next?.reason ? { explanation: next.reason } : {}),
      ...(next?.ref ? { ref: next.ref } : {}),
      // A command the stage names, other than the report the hint already offers.
      ...(next && next.stage.startsWith('/') && next.stage !== '/project:status' ? { action: next.stage } : {}),
      text: next ? `${next.stage}: ${next.reason}` : '',
      hasNext: next !== undefined,
      hint: `Hint only${sep}Full report:`,
      command: '/project:status',
    },
    hasGit: snapshot.hasGit,
  }
}

function pad(text: string, width: number, align: 'left' | 'right' = 'left'): string {
  const n = [...text].length
  const spaces = ' '.repeat(Math.max(0, width - n))
  return align === 'right' ? `${spaces}${text}` : `${text}${spaces}`
}

function cellText(cell: Cell): string {
  return cell.glyph ? `${cell.glyph} ${cell.text}` : cell.text
}

function tableLines(section: Section, mode: Mode, ascii: boolean, edge: boolean): string[] {
  const gap = ' '.repeat(mode === 'full' ? 2 : 1)
  const join = (cells: string[]): string => cells.map((text, i) => pad(text, section.columns[i]?.width ?? 0, section.columns[i]?.align)).join(gap).trimEnd()
  const mark = (row?: Row): string => (edge ? `${row?.edge ? glyph('edge', ascii) : ' '} ` : '')
  const out = [`${mark()}${join(section.columns.map(c => c.label))}`]
  for (const row of section.rows) {
    out.push(`${mark(row)}${join(row.cells.map(cellText))}`)
    if (row.sub) out.push(`${edge ? '  ' : ''}  ${row.sub}`)
    for (const detail of row.detail ?? []) out.push(`${edge ? '  ' : ''}    ${detail.map(cellText).join(' ')}`)
  }
  return out
}

// The dashboard as plain lines: what the model says, in the order it is drawn.
// The golden strings of spec 029 SC-005 are this output, and a surface that
// cannot draw the tree could print it. Compact widths get 013's text.
export function dashboardText(snapshot: Snapshot, input: Context): string[] {
  const ctx: Context = { ...input, barMax: input.barMax ?? TEXT_BAR_MAX }
  const m = build(snapshot, ctx)
  if (m.mode === 'compact' || m.isEmpty) return view(snapshot, ctx.columns, ctx.expanded).map(line => line.text)
  const sep = ` ${glyph('sep', m.ascii)} `
  const by = (id: string): Section => m.sections.find(s => s.id === id) as Section
  const lines = [`${m.header.title}  ${m.header.badge}  ${m.header.caption}`, '']

  if (m.mode === 'full') {
    lines.push(m.summary.metrics.filter(x => x.isLarge).map(x => `${x.value} ${x.label}`).join('    '))
    lines.push(m.summary.metrics.filter(x => !x.isLarge).map(x => `${x.value} ${x.label}`).join(sep))
  } else {
    lines.push(m.summary.line)
  }

  lines.push('', 'Artifacts', ...tableLines(by('artifacts'), m.mode, m.ascii, true))
  if (!m.hasGit) lines.push('Staleness needs git.')
  lines.push(`Stages (an aid)  ${m.stages.map(s => `${s.glyph} ${s.label}`).join('  ')}`)

  lines.push('', m.progress.total === 0 ? 'Implementation' : `Implementation   ${m.progress.percent}%`)
  if (m.progress.total === 0) {
    lines.push(snapshot.hasReadme ? 'specs/README.md lists no feature.' : 'No specs/README.md yet.')
  } else {
    lines.push(m.progress.bar.map(seg => seg.glyph.repeat(seg.length)).join(''), m.progress.line)
  }

  const features = by('features')
  if (!m.featuresOpen) {
    const c = m.filter.counts
    lines.push('', `Features  ${[`${c.all} ${plural(c.all, 'feature')}`, `Ready ${c.ready}`, `Blocked ${c.blocked}`, `Done ${c.done}`].join(sep)}  ${glyph('closed', m.ascii)} Show`)
  } else lines.push('', `Features  ${glyph('open', m.ascii)} Hide`, FILTERS.map(f => `${f === m.filter.active ? `${glyph('implemented', m.ascii)} ` : ''}${f[0]?.toUpperCase()}${f.slice(1)} ${m.filter.counts[f]}`).join('  '))
  if (m.featuresOpen && features.rows.length > 0) lines.push(...tableLines(features, m.mode, m.ascii, false))
  if (m.featuresOpen && features.empty) lines.push(features.empty)
  if (m.featuresOpen && features.more) lines.push(features.more)
  if (m.featuresOpen && !m.matrix.words && features.rows.length > 0) {
    lines.push(`${glyph('planned', m.ascii)} Planned  ${glyph('progress', m.ascii)} In progress  ${glyph('partial', m.ascii)} Partial  ${glyph('implemented', m.ascii)} Implemented  ${glyph('verified', m.ascii)} Verified  ${glyph('none', m.ascii)} N/A`)
  }

  lines.push('', 'Open items')
  for (const row of by('items').rows) {
    lines.push(`${pad(cellText(row.cells[0] as Cell), 16)}${row.cells[1]?.text}`)
    for (const detail of row.detail ?? []) lines.push(`  ${detail.map(cellText).join(' ')}`)
  }

  lines.push('', `Next action  ${m.next.badge}`, `${glyph('current', m.ascii)} ${m.next.title}`)
  if (m.next.explanation) lines.push(`  ${m.next.explanation}`)
  if (m.next.ref) lines.push(`  ${m.next.ref}`)
  if (m.next.action) lines.push(`  Run ${m.next.action}`)
  lines.push(`${m.next.hint} ${m.next.command}`)
  return lines
}
