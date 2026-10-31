// The pane's structured view model (spec 029 groups 1 and 3): pure, so no surface.

import { expect, test } from 'claude-code/testing'

import type { Snapshot } from '../types'
import { bigReadme, snapshotOf } from './pipeline-view-fixtures'
import { ARTIFACTS, STAGE_NAME, progress, stageStrip } from './pipeline-model'
import { DASHBOARD_120, DASHBOARD_120_ASCII, DASHBOARD_80, DASHBOARD_80_ASCII, VIEW_39 } from './pipeline-golden'
import { build, dashboardText, statusCell } from './pipeline-view'
import type { Context, Section } from './pipeline-view'

const CTX: Context = { columns: 120, expanded: {}, filter: 'all', checkedAt: 1000, now: 1000, pollMs: 5000, ascii: false, maxRows: 60 }
const ctx = (patch: Partial<Context> = {}): Context => ({ ...CTX, ...patch })
const section = (snapshot: Snapshot, id: string, patch: Partial<Context> = {}): Section => {
  const found = build(snapshot, ctx(patch)).sections.find(s => s.id === id)
  if (!found) throw new Error(`no section ${id}`)
  return found
}
const SINGLE = '| # | Feature | Priority | Depends on | Status |\n|---|---|---|---|---|\n| 001 | one | P1 | — | Planned |\n| 002 | two | P2 | 001 | Implemented |\n'

test('every stage an artifact names has a display name, and the steps and buckets follow the snapshot', () => {
  for (const spec of ARTIFACTS) expect(STAGE_NAME[spec.stage]).toBeDefined()
  expect(STAGE_NAME['/project:init']).toBe('Project init')
  expect(STAGE_NAME['/devsecops:pipeline']).toBe('DevOps')

  const s = snapshotOf()
  expect(progress(s.features)).toEqual({ implemented: 3, planned: 4, other: 2 })
  expect(stageStrip(s.artifacts, s.features).map(x => `${x.label}:${x.state}`)).toEqual([
    'Init:done', 'Architecture:current', 'Spec:done', 'Build:pending', 'QA:pending', 'Delivery:pending',
  ])
})

test('the summary counts what the snapshot holds, and the mode follows the width', () => {
  const m = build(snapshotOf(), ctx())
  expect(m.summary).toMatchObject({ artifacts: 2, ofArtifacts: 8, features: 9, clarifications: 1, assumptions: 1, gaps: 2 })
  expect(m.summary.metrics.map(x => `${x.value} ${x.label}${x.isLarge ? '!' : ''}`)).toEqual([
    '2 Artifacts ready!', '9 Features!', '1 Clarification!', '1 Assumption', '2 Gaps',
  ])
  expect(build(snapshotOf(), ctx({ columns: 80 })).summary.line).toBe('2 ready · 9 features · 1 clarification · 1 assumption · 2 gaps')
  expect([39, 40, 90, 91].map(columns => build(snapshotOf(), ctx({ columns })).mode)).toEqual(['compact', 'condensed', 'condensed', 'full'])
})

test('artifact rows are Ready, Stale, Missing or Pending, with a stage and an edge mark', () => {
  const rows = section(snapshotOf(), 'artifacts').rows
  const by = (key: string) => rows.find(r => r.key === `artifact:${key}`)
  expect(by('docs/product/brief.md')?.cells.map(c => c.text)).toEqual(['brief', 'Project init', 'Ready'])
  expect(by('docs/product/brief.md')).toMatchObject({ edge: 'good' })
  expect(by('docs/product/architecture.md')?.cells[2]).toMatchObject({ text: 'Missing', glyph: '✕', tone: 'bad' })
  expect(by('docs/product/architecture.md')).toMatchObject({ edge: 'bad' })
  expect(by('specs/README.md')?.cells[2]).toMatchObject({ text: 'Stale', glyph: '↻', tone: 'warn' })
  expect(by('specs/README.md')?.sub).toBe('older than docs/product/brief.md (2026-10-09)')
  expect(by('docs/product/domain-model.md')?.cells[2]).toMatchObject({ text: 'Pending', glyph: '·', tone: 'dim' })
  expect(by('docs/product/domain-model.md')?.cells[1]?.text).toBe('Backend')
  expect(section(snapshotOf(), 'artifacts').columns.map(c => c.key)).toEqual(['artifact', 'stage', 'status'])
  expect(section(snapshotOf(), 'artifacts', { columns: 80 }).columns.map(c => c.key)).toEqual(['artifact', 'status'])
  expect(section(snapshotOf(), 'artifacts', { columns: 80 }).rows[0]?.cells).toHaveLength(2)
})

test('no git means no Stale and a line that says so', () => {
  const s = snapshotOf(undefined, { hasGit: false })
  const text = dashboardText({ ...s, artifacts: s.artifacts.map(a => ({ ...a, staleSince: null })) }, ctx()).join('\n')
  expect(text).toContain('Staleness needs git.')
  expect(text).not.toMatch(/↻|\bStale\b/)
})

test('the matrix columns are the sides the README has, with fixed widths', () => {
  const full = section(snapshotOf(), 'features')
  expect(full.columns.map(c => c.label)).toEqual(['ID', 'FEATURE', 'PRI', 'READY', 'FRONTEND', 'BACKEND', 'QA'])
  expect(full.columns.map(c => c.width)).toEqual([3, 56, 3, 7, 13, 13, 13])
  expect(full.columns[2]?.align).toBe('right')
  expect(section(snapshotOf(SINGLE), 'features').columns.map(c => c.label)).toEqual(['ID', 'FEATURE', 'PRI', 'READY', 'STATUS'])
  const none = section(snapshotOf('# no rows'), 'features')
  expect(none.rows).toEqual([])
  expect(none.empty).toBe('specs/README.md lists no feature.')
  expect(section(snapshotOf(undefined, { hasReadme: false, features: [] }), 'features').empty).toBe('No specs/README.md yet.')

  const condensed = section(snapshotOf(), 'features', { columns: 80 })
  expect(condensed.columns.map(c => c.label)).toEqual(['ID', 'FEATURE', 'PRI', 'READY', 'F', 'B', 'Q'])
  expect(condensed.columns.map(c => c.width)).toEqual([3, 58, 3, 7, 1, 1, 1])
})

test('every row of every table fits its columns', () => {
  for (const columns of [120, 91, 90, 80, 56, 40]) {
    for (const sec of build(snapshotOf(bigReadme(5)), ctx({ columns })).sections) {
      for (const row of sec.rows) {
        row.cells.forEach((cell, i) => {
          const width = sec.columns[i]?.width ?? 0
          expect([...`${cell.glyph ? `${cell.glyph} ` : ''}${cell.text}`].length).toBeLessThanOrEqual(Math.max(width, 0) || 999)
        })
      }
    }
  }
})

test('readiness and implementation status are two columns with two vocabularies', () => {
  const rows = section(snapshotOf(), 'features').rows
  const row = (n: string) => rows.find(r => r.key === `feature:${n}`)
  const eight = row('008')?.cells ?? []
  expect(eight[3]).toMatchObject({ text: 'ready', tone: 'info' })
  expect(eight[3]?.glyph).toBeUndefined()
  expect(eight[5]).toMatchObject({ text: 'Planned', glyph: '○', tone: 'warn' })
  expect(row('003')?.cells[3]).toMatchObject({ text: 'blocked', tone: 'bad' })
  expect(row('001')?.cells[3]).toMatchObject({ text: 'done', tone: 'dim' })
  // retired: dim row, READY says done, the cells say Retired
  expect(row('007')?.cells.map(c => c.text).slice(3)).toEqual(['done', 'Retired', 'Retired', '—'])
  expect(row('007')?.tone).toBe('dim')
  // a cell with no status is a plain dash, never markup
  expect(row('008')?.cells[4]).toMatchObject({ text: '—', tone: 'dim' })
  expect(row('004')?.cells[4]?.text).toBe('Implemented')
})

test('the status vocabulary keeps its words, glyphs and tones apart', () => {
  const words = ['Planned', 'In progress', 'Partial', 'Implemented', 'Verified', 'N/A', 'Retired', '']
  const cells = words.map(w => statusCell(w, false))
  expect(cells.map(c => `${c.glyph ?? ''}${c.text}`)).toEqual(['○Planned', '◉In progress', '◐Partial', '●Implemented', '✓Verified', '—N/A', '—Retired', '—'])
  expect(new Set(cells.slice(0, 5).map(c => c.glyph)).size).toBe(5)
  expect(cells.map(c => c.tone)).toEqual(['warn', 'info', 'info', 'good', 'good', 'dim', 'dim', 'dim'])
  expect(statusCell('Implemented (tests untested)', false).text).toBe('Implemented')
  expect(statusCell('Verified', true)).toMatchObject({ glyph: '+' })
  expect(statusCell('', true)).toMatchObject({ text: '-' })
})

test('a long name is cut with an ellipsis, in a column that never wraps', () => {
  const s = snapshotOf(bigReadme(2))
  for (const columns of [56, 120]) {
    const name = section(s, 'features', { columns }).rows[0]?.cells[1]?.text ?? ''
    const width = section(s, 'features', { columns }).columns[1]?.width ?? 0
    expect(name.endsWith('…')).toBe(true)
    expect([...name]).toHaveLength(width)
  }
})

test('the filter is the bucket and nothing else', () => {
  const s = snapshotOf()
  const counts = build(s, ctx()).filter.counts
  expect(counts).toEqual({ all: 9, ready: 3, blocked: 2, done: 4 })
  const keys = (filter: string) => section(s, 'features', { filter }).rows.map(r => r.key.slice(8))
  expect(keys('ready')).toEqual(['002', '006', '008'])
  expect(keys('blocked')).toEqual(['003', '009'])
  expect(keys('done')).toEqual(['001', '004', '005', '007'])
  expect(keys('all')).toHaveLength(9)
  expect(build(s, ctx({ filter: 'bogus' })).filter.active).toBe('all')
  const noneReady = snapshotOf(undefined, { features: s.features.filter(f => f.bucket !== 'ready') })
  expect(section(noneReady, 'features', { filter: 'ready' }).empty).toBe('No ready features.')
})

test('the matrix draws at most the row cap, then says how many are not shown', () => {
  const s = snapshotOf(bigReadme(200))
  const sec = section(s, 'features')
  expect(sec.rows).toHaveLength(60)
  expect(sec.more).toBe('+140 more; filter to see them')
  expect(section(s, 'features', { maxRows: 5 }).rows).toHaveLength(5)
  expect(section(snapshotOf(), 'features').more).toBeUndefined()
})

test('progress counts the buckets, the strip never feeds Next', () => {
  const m = build(snapshotOf(), ctx())
  expect(m.progress).toMatchObject({ implemented: 3, planned: 4, other: 2, total: 9 })
  expect(m.progress.bar.reduce((n, seg) => n + seg.length, 0)).toBe(m.progress.width)
  expect(m.progress.bar.every(seg => seg.length >= 1)).toBe(true)
  const base = snapshotOf()
  const ahead = snapshotOf(undefined, { artifacts: base.artifacts.map(a => ({ ...a, isPresent: true, isProblem: false, staleSince: null })) })
  expect(build(base, ctx()).next.text).toBe(`${base.next.stage}: ${base.next.reason}`)
  expect(build(ahead, ctx()).next.text).toBe(`${ahead.next.stage}: ${ahead.next.reason}`)
})

test('the badge is LIVE only when a check ran within three intervals', () => {
  const live = build(snapshotOf(), ctx({ checkedAt: 7_000, now: 7_000 + 15_000, pollMs: 5000 }))
  expect(live.header.badge).toBe('LIVE')
  expect(live.header.caption).toMatch(/^Checked \d\d:\d\d:\d\d · every 5 s$/)
  const stale = build(snapshotOf(), ctx({ checkedAt: 7_000, now: 7_000 + 15_001, pollMs: 5000 }))
  expect(stale.header.badge).toBe('STALE')
  expect(stale.header.caption).toMatch(/^Last checked \d\d:\d\d:\d\d$/)
  expect(build(snapshotOf(), ctx({ pollMs: 0 })).header.badge).toBe('STALE')
  expect(build(snapshotOf(), ctx({ checkedAt: 0, now: 0 })).header.caption).toBe('Not checked yet')
})

test('the next action badge follows the rule', () => {
  const s = snapshotOf()
  expect(build(s, ctx()).next).toMatchObject({ badge: 'Needs attention', ref: 'specs/004-x/spec.md:12' })
  expect(build({ ...s, next: { ...s.next, rule: 3 } }, ctx()).next.badge).toBe('Suggested')
  expect(build({ ...s, next: { ...s.next, rule: 2 } }, ctx()).next.badge).toBe('Suggested')
  expect(build({ ...s, next: { ...s.next, rule: null } }, ctx()).next.badge).toBe('Up to date')
})

test('open items are one row per kind and open on their own', () => {
  const rows = section(snapshotOf(), 'items').rows
  expect(rows.map(r => r.key)).toEqual(['marker:clarification', 'marker:assumption', 'marker:gap'])
  expect(rows[0]).toMatchObject({ isOpen: false })
  expect(rows[2]?.cells[1]).toMatchObject({ text: '2' })
  expect(rows[0]?.cells[1]).toMatchObject({ text: '1', tone: 'warn' })
  const none = section(snapshotOf(undefined, { markers: { counts: { gap: 0, clarification: 0, assumption: 0 }, items: [], isTruncated: false } }), 'items').rows
  expect(none.every(r => r.cells[1]?.tone === 'dim')).toBe(true)
  const open = section(snapshotOf(), 'items', { expanded: { markers: true } }).rows
  expect(open[0]?.detail?.[0]?.[0]?.text).toBe('specs/004-x/spec.md:12 which login?')
  expect(open[1]?.detail?.[0]?.[0]?.text).toBe('docs/product/brief.md:3 one tenant')
  expect(open[2]?.detail).toEqual([])
})

test('the compact width is the 013 text, and the ASCII set has no other character', () => {
  const s = snapshotOf()
  expect(dashboardText(s, ctx({ columns: 39 }))[0]).toBe('Pipeline')
  for (const columns of [120, 80]) {
    const text = dashboardText(s, ctx({ columns, ascii: true })).join('\n')
    expect(/^[\x20-\x7e\n]*$/.test(text)).toBe(true)
  }
  expect(dashboardText(s, ctx({ columns: 120 })).join('\n')).toContain('✓')
})

test('every toned cell has a glyph or a word beside its colour', () => {
  for (const columns of [120, 80]) {
    for (const sec of build(snapshotOf(), ctx({ columns })).sections) {
      for (const row of sec.rows) for (const cell of row.cells) if (cell.tone && cell.tone !== 'plain') expect((cell.glyph ?? '') + cell.text).not.toBe('')
    }
  }
})

test('golden: the dashboard text at 120 and 80 columns, in Unicode and in ASCII', () => {
  const draw = (columns: number, ascii: boolean) =>
    dashboardText(snapshotOf(), ctx({ columns, ascii, checkedAt: 1_700_000_000_000, now: 1_700_000_000_000 })).map(l => l.replace(/\d\d:\d\d:\d\d/, 'HH:MM:SS'))
  expect(draw(120, false)).toEqual(DASHBOARD_120)
  expect(draw(120, true)).toEqual(DASHBOARD_120_ASCII)
  expect(draw(80, false)).toEqual(DASHBOARD_80)
  expect(draw(80, true)).toEqual(DASHBOARD_80_ASCII)
  // 39 columns is 013's compact text, byte for byte.
  expect(dashboardText(snapshotOf(), ctx({ columns: 39 })).join('|')).toBe(VIEW_39.map(l => l.split('|')[1]).join('|'))
})
