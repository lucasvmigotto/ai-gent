// What the pipeline pane shows, computed from text and file facts the module
// read. Pure: no `$`, so the pane and the tests share it as it is. It reads
// specs/README.md the way plugins/repo/scripts/specsync.py (a feature's state)
// and plugins/agents/scripts/wt.py (ready, blocked, done) do, and follows
// project:status §4 rules 1-3 for Next. project:status stays the report of
// record; nothing here decides anything.

import type { ArtifactState, Feature, FeatureState, Marker, MarkerKind, Next } from '../types'

// The artifacts of shared/pipeline.md's table that project:status §1 checks, in
// chain order. specs/README.md is never an `after`: build and QA stages rewrite
// its status columns all the time, so it would make everything look stale. scripts/consistency.py fails if a path here is not in that table.
// `neededBy` lists the artifacts whose presence makes a missing one a problem
// (project:status: "only a problem when a later stage already ran").
export type ArtifactSpec = {
  path: string
  label: string
  stage: string
  after: string[]
  neededBy: string[]
}

export const BRIEF = 'docs/product/brief.md'
export const README = 'specs/README.md'

export const ARTIFACTS: readonly ArtifactSpec[] = [
  { path: BRIEF, label: 'brief', stage: '/project:init', after: [], neededBy: ['docs/product/architecture.md', README, 'docs/product/ux-vision.md'] },
  { path: 'docs/product/architecture.md', label: 'architecture', stage: '/project:architecture', after: [BRIEF], neededBy: [README] },
  { path: 'docs/product/ux-vision.md', label: 'ux-vision', stage: '/frontend:uiux', after: [BRIEF], neededBy: [] },
  { path: README, label: 'specs index', stage: '/project:spec', after: [BRIEF, 'docs/product/architecture.md'], neededBy: ['contracts/openapi.yaml', 'docs/product/test-strategy.md', 'docs/product/delivery.md'] },
  { path: 'docs/product/domain-model.md', label: 'domain-model', stage: '/backend:domain', after: ['docs/product/architecture.md'], neededBy: [] },
  { path: 'contracts/openapi.yaml', label: 'contract', stage: '/backend:spec', after: ['docs/product/architecture.md'], neededBy: [] },
  { path: 'docs/product/test-strategy.md', label: 'test-strategy', stage: '/qa:strategy', after: ['docs/product/architecture.md'], neededBy: [] },
  { path: 'docs/product/delivery.md', label: 'delivery', stage: '/devsecops:pipeline', after: ['docs/product/architecture.md'], neededBy: [] },
]

// Where markers are counted.
export const MARKER_DIRS = ['docs/product', 'specs']

// ------------------------------------------------------------------ README

// The columns that carry a status, as specsync.py and wt.py read them.
const SIDES = ['frontend', 'backend', 'status', 'qa']

export type Row = Pick<Feature, 'number' | 'name' | 'priority' | 'depends' | 'sides'> & { notes: string }

export function parseReadme(text: string): Row[] {
  const rows: Row[] = []
  let header: string[] | null = null
  for (const line of text.split(/\r?\n/)) {
    if (!line.startsWith('|')) {
      header = null
      continue
    }
    const cells = line.trim().replace(/^\||\|$/g, '').split('|').map(cell => cell.trim())
    if (header === null) {
      header = cells.map(cell => cell.toLowerCase())
      continue
    }
    if (/^[-: ]*$/.test(cells.join('')) || !/^\d{3}$/.test(cells[0] ?? '')) continue
    const row: Record<string, string> = {}
    header.forEach((name, i) => {
      row[name] = cells[i] ?? ''
    })
    const deps = row['depends on'] ?? row.depends ?? row.dependencies ?? ''
    rows.push({
      number: cells[0] ?? '',
      name: row.feature ?? '',
      priority: row.priority ?? '',
      depends: deps.match(/\d{3}/g) ?? [],
      sides: Object.fromEntries(SIDES.filter(side => side in row).map(side => [side, row[side] ?? ''])),
      notes: row.notes ?? '',
    })
  }
  return rows
}

const LEVELS: Record<string, number> = { planned: 0, 'in progress': 1, partial: 1, implemented: 2, verified: 3 }
const STATE_OF: Record<string, FeatureState> = { planned: 'todo', 'in progress': 'doing', partial: 'doing', implemented: 'review', verified: 'done' }
const RETIRED = ['retired', 'superseded', 'removed', 'dropped', 'cancelled', 'canceled']
const ORDER: FeatureState[] = ['todo', 'doing', 'review', 'done']

// `Implemented (containers; …)` → `implemented`; '' for N/A, — or anything else.
export function word(value: string): string {
  const v = value.toLowerCase().replace(/^[*_ ]+|[*_ ]+$/g, '')
  return [...Object.keys(LEVELS), ...RETIRED].find(w => v.startsWith(w)) ?? ''
}

// specsync.py's feature state, from the README alone: the least advanced side.
// Without a recognized status specsync reads tasks.md; the pane does not, so
// such a feature is `todo` (as specsync has it with no ticked task). Retired
// is judged over every cell of the row, notes too, as specsync does.
export function featureState(sides: Record<string, string>, notes = ''): FeatureState {
  const all = [...Object.values(sides), notes].map(word)
  if (all.some(w => RETIRED.includes(w)) && all.every(w => w === '' || RETIRED.includes(w))) return 'retired'
  const states = Object.values(sides).map(word).filter(w => w in STATE_OF).map(w => STATE_OF[w] as FeatureState)
  return states.length > 0 ? states.reduce((a, b) => (ORDER.indexOf(b) < ORDER.indexOf(a) ? b : a)) : 'todo'
}

// wt.py's level: null for N/A, — and retired, which are not a side to build.
export function level(value: string): number | null {
  const w = word(value)
  return w in LEVELS ? (LEVELS[w] as number) : null
}

export type Split = { ready: string[]; blocked: string[]; done: string[]; waiting: Record<string, string[]> }

// wt.py plan: a feature is done with no side below Implemented; blocked while a
// dependency is not Implemented in every side it builds; else ready.
export function split(rows: readonly Row[]): Split {
  const by = new Map(rows.map(row => [row.number, row]))
  const built = (n: string): boolean => {
    const row = by.get(n)
    if (!row) return false
    const levels = Object.values(row.sides).map(level).filter((l): l is number => l !== null)
    return levels.length > 0 && Math.min(...levels) >= 2
  }
  const out: Split = { ready: [], blocked: [], done: [], waiting: {} }
  for (const row of rows) {
    const todo = Object.values(row.sides).map(level).filter((l): l is number => l !== null && l < 2)
    if (todo.length === 0) {
      out.done.push(row.number)
      continue
    }
    const waiting = row.depends.filter(d => !built(d))
    ;(waiting.length > 0 ? out.blocked : out.ready).push(row.number)
    out.waiting[row.number] = waiting
  }
  return out
}

// ----------------------------------------------------------------- markers

const MARKERS: [RegExp, MarkerKind][] = [
  [/\[UPSTREAM GAP:\s*([^\]]*)/, 'gap'],
  [/\[NEEDS CLARIFICATION:\s*([^\]]*)/, 'clarification'],
  [/\[ASSUMPTION:\s*([^\]]*)/, 'assumption'],
]

// The markers of one file. A fenced block and an inline `code` span are
// examples of a marker, not one; a bare `…` is the placeholder.
export function scanMarkers(file: string, text: string): Marker[] {
  const found: Marker[] = []
  let fence = false
  text.split(/\r?\n/).forEach((raw, i) => {
    if (/^\s*(```|~~~)/.test(raw)) {
      fence = !fence
      return
    }
    if (fence) return
    // Match on a copy with code spans blanked (same length), show the original.
    const line = raw.replace(/`[^`]*`/g, span => ' '.repeat(span.length))
    for (const [pattern, kind] of MARKERS) {
      const hit = pattern.exec(line)
      if (!hit) continue
      const body = raw.slice(hit.index, hit.index + hit[0].length).replace(/^\[[^:]+:\s*/, '').trim()
      if (body !== '' && body !== '…' && body !== '...') {
        found.push({ kind, file, line: i + 1, text: body.length > 80 ? `${body.slice(0, 79)}…` : body })
      }
    }
  })
  return found
}

// -------------------------------------------------------------------- next

const BUILD: Record<string, string> = { frontend: '/frontend:build', backend: '/backend:build', qa: '/qa:e2e', status: '/project:status' }

function priorityOf(feature: Pick<Feature, 'priority'>): number {
  return Number(/P(\d+)/i.exec(feature.priority)?.[1] ?? 99)
}

// project:status §4, rules 1-3. Rule 1's contradictions and rule 4's later
// stages need evidence the pane does not read: /project:status has them.
export function recommend(artifacts: readonly ArtifactState[], features: readonly Feature[], markers: readonly Marker[]): Next {
  const blocker = markers.find(m => m.kind === 'clarification')
  if (blocker) {
    return { rule: 1, stage: 'ask the user', reason: `an open [NEEDS CLARIFICATION] blocks plan: ${blocker.text}`, ref: `${blocker.file}:${blocker.line}` }
  }
  if (artifacts.every(a => !a.isPresent)) {
    return { rule: 2, stage: '/project:init', reason: 'the project has no pipeline artifacts yet' }
  }
  const missing = artifacts.find(a => !a.isPresent && a.isProblem)
  if (missing) {
    return { rule: 2, stage: missing.stage, reason: `${missing.label} is missing although a later stage already ran`, ref: missing.path }
  }
  const stale = artifacts.find(a => a.isPresent && a.staleSince)
  if (stale?.staleSince) {
    return { rule: 2, stage: stale.stage, reason: `${stale.label} is older than ${stale.staleSince.path} (changed ${stale.staleSince.date})`, ref: stale.path }
  }
  const ready = features.filter(f => f.bucket === 'ready').sort((a, b) => priorityOf(a) - priorityOf(b) || a.number.localeCompare(b.number))[0]
  if (ready) {
    const [side, value] = Object.entries(ready.sides)
      .filter(([, v]) => (level(v) ?? 9) < 2)
      .sort(([, a], [, b]) => (level(a) ?? 9) - (level(b) ?? 9))[0] ?? ['status', '']
    const step = !ready.files.spec ? 'specify' : !ready.files.plan ? 'plan' : !ready.files.tasks ? 'tasks' : null
    const stage = step ? '/project:spec' : (BUILD[side] ?? '/project:status')
    return {
      rule: 3,
      stage,
      reason: step ? `${ready.number} ${ready.name} needs ${step}` : `${ready.number} ${ready.name}: ${side} is ${value || 'Planned'}`,
      ref: ready.number,
    }
  }
  return { rule: null, stage: '/project:status', reason: 'nothing is ready to start; the full report covers QA, docs and CI' }
}

// A glanced status in one character, for a narrow pane.
export function letter(value: string): string {
  const w = word(value)
  return ({ planned: 'P', 'in progress': 'W', partial: 'W', implemented: 'I', verified: 'V' } as Record<string, string>)[w] ?? (RETIRED.includes(w) ? 'x' : /n\/a/i.test(value) ? 'n' : '-')
}

// ------------------------------------------------------------- the dashboard

// A stage's display name, keyed by `ArtifactSpec.stage` (spec 029 FR-005).
export const STAGE_NAME: Readonly<Record<string, string>> = {
  '/project:init': 'Project init',
  '/project:architecture': 'Architecture',
  '/frontend:uiux': 'UI/UX',
  '/project:spec': 'Specification',
  '/backend:domain': 'Backend',
  '/backend:spec': 'API specification',
  '/qa:strategy': 'QA strategy',
  '/devsecops:pipeline': 'DevOps',
}

// How many features are Implemented (least advanced side Implemented or
// Verified), Planned, or neither (in progress, retired). The three sum to the rows.
export function progress(features: readonly Feature[]): { implemented: number; planned: number; other: number } {
  const out = { implemented: 0, planned: 0, other: 0 }
  for (const f of features) {
    if (f.state === 'review' || f.state === 'done') out.implemented += 1
    else if (f.state === 'todo') out.planned += 1
    else out.other += 1
  }
  return out
}

export type StageStep = { key: string; label: string; state: 'done' | 'current' | 'pending' }

// The six steps of the stage strip: an aid to read at a glance, from artifacts
// and buckets already in the snapshot. `recommend` never reads it.
export function stageStrip(artifacts: readonly ArtifactState[], features: readonly Feature[]): StageStep[] {
  const has = (path: string): boolean => artifacts.some(a => a.path === path && a.isPresent)
  const live = features.filter(f => f.state !== 'retired')
  const withQa = live.filter(f => 'qa' in f.sides && word(f.sides.qa ?? '') !== '')
  const done: [string, string, boolean][] = [
    ['init', 'Init', has(BRIEF)],
    ['architecture', 'Architecture', has('docs/product/architecture.md')],
    ['spec', 'Spec', has(README)],
    ['build', 'Build', live.length > 0 && live.every(f => f.bucket === 'done')],
    ['qa', 'QA', has('docs/product/test-strategy.md') && withQa.every(f => word(f.sides.qa ?? '') === 'verified')],
    ['delivery', 'Delivery', has('docs/product/delivery.md')],
  ]
  const current = done.findIndex(([, , isDone]) => !isDone)
  return done.map(([key, label, isDone], i) => ({ key, label, state: isDone ? 'done' : i === current ? 'current' : 'pending' }))
}
