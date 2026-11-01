import { expect, test } from 'claude-code/testing'

import type { Band } from './band-priority'
import { isFoldable, parseBands, split, visible } from './band-priority'
import { modeOf, toggleGlyph, workspaceView } from './workspace-model'

let n = 0
const band = (id: string, plugin: string, priority: Band['priority'], summary?: Band['summary']): Band => ({ id, plugin, priority, since: ++n, ...(summary ? { summary } : {}) })

const CI_OK = band('ci', 'repo', 'normal', { text: 'CI passing', tone: 'ok' })
const RELEASE = band('release', 'git', 'info', { text: 'Release 2.23.0', tone: 'info' })
const LEFT = band('containers', 'devcontainer', 'info', { text: '21 container leftovers', tone: 'warn' })
const BEHIND = band('hygiene', 'git', 'normal', { text: 'main behind origin', tone: 'warn' })
const HOLD = band('holds', 'project', 'hold')
const CI_FAIL = band('ci', 'repo', 'failure')
const DISK = band('containers', 'devcontainer', 'normal')
const view = (bands: Band[], columns = 100, ascii = false) => workspaceView(bands, { columns, ascii })

test('nothing folds without a summary, and a hold or a failure never folds even with one', () => {
  expect(view([HOLD, CI_FAIL, DISK])).toMatchObject({ count: 0, line: null, warnings: null, details: [] })
  const sneaky: Band[] = [{ ...HOLD, summary: { text: 'x', tone: 'ok' } }, { ...CI_FAIL, summary: { text: 'y', tone: 'ok' } }]
  expect(sneaky.map(isFoldable)).toEqual([false, false])
  expect(view(sneaky)).toMatchObject({ count: 0, line: null })
})

test('several plugins fold into one line and one warnings line, in band order', () => {
  const v = view([CI_OK, RELEASE, LEFT, BEHIND])
  expect(v?.line).toBe('Workspace: ✓ CI passing · Release 2.23.0')
  expect(v?.warnings).toBe('⚠ main behind origin · 21 container leftovers')
  expect(v?.count).toBe(4)
})

test('healthy, release and a lone warning each read alone', () => {
  expect(view([CI_OK])?.line).toBe('Workspace: ✓ CI passing')
  expect(view([CI_OK])?.warnings).toBeNull()
  expect(view([LEFT])?.line).toBeNull()
  expect(view([LEFT])?.warnings).toBe('⚠ 21 container leftovers')
})

test('wide labels the line, medium and narrow drop the label, and ASCII has no Unicode', () => {
  expect(modeOf(100)).toBe('wide')
  expect(modeOf(79)).toBe('medium')
  expect(modeOf(49)).toBe('narrow')
  expect(view([RELEASE], 70)?.line).toBe('Release 2.23.0')
  const ascii = view([CI_OK, RELEASE, LEFT], 100, true)
  expect(ascii?.line).toBe('Workspace: ok CI passing | Release 2.23.0')
  expect(`${ascii?.line}${ascii?.warnings}`).not.toMatch(/[^\x20-\x7e]/)
  expect(toggleGlyph(false, false) + toggleGlyph(true, false)).toBe('▸▾')
  expect(toggleGlyph(false, true) + toggleGlyph(true, true)).toBe('>v')
})

test('no row passes the width: segments drop into +N, then the first is cut', () => {
  for (const columns of [120, 80, 60, 50, 40, 30, 20, 12, 5]) {
    const v = view([CI_OK, RELEASE, LEFT, BEHIND], columns)
    for (const row of [v?.line, v?.warnings, ...(v?.details ?? [])]) {
      if (row) expect([...row].length + [...row].filter(c => c === '⚠').length).toBeLessThanOrEqual(Math.max(columns, 12))
    }
  }
  expect(view([CI_OK, RELEASE, LEFT, BEHIND], 40)?.line).toMatch(/\+1$/)
  expect(view([BEHIND], 20)?.warnings).toMatch(/…$/)
})

test('details list every folded band whole, whatever the line dropped', () => {
  expect(view([CI_OK, RELEASE, LEFT, BEHIND])?.details).toEqual([
    'repo: CI passing',
    'git: main behind origin',
    'git: Release 2.23.0',
    'devcontainer: 21 container leftovers',
  ])
})

test('priority and cap: folded bands do not count, a hold and a failure still draw within the cap', () => {
  const all = [HOLD, CI_FAIL, DISK, RELEASE, BEHIND]
  const { folded, rest } = split(all, true)
  expect(folded.map(b => b.id)).toEqual(['release', 'hygiene'])
  expect(visible(rest, 2)).toEqual(['holds', 'ci'])
  // the low disk is next in line, hidden by the cap but never folded away
  expect(visible(rest, 3)).toEqual(['holds', 'ci', 'containers'])
  expect(split(all, false).rest).toHaveLength(5)
})

test('a failed CI, a hold and a low disk are never in the line even when many summaries compete', () => {
  const v = view([HOLD, CI_FAIL, DISK, CI_OK, RELEASE, LEFT, BEHIND])
  expect(`${v?.line}${v?.warnings}${v?.details.join('')}`).not.toMatch(/hold|disk|fail/i)
})

test('parseBands keeps a valid summary and drops a malformed one without losing the band', () => {
  const parsed = parseBands([
    { id: 'a', plugin: 'p', priority: 'info', since: 1, summary: { text: 'ok', tone: 'ok' } },
    { id: 'b', plugin: 'p', priority: 'info', since: 2, summary: { text: '', tone: 'ok' } },
    { id: 'c', plugin: 'p', priority: 'info', since: 3, summary: { text: 'x', tone: 'red' } },
    { id: 'd', plugin: 'p', priority: 'info', since: 4, summary: 'nope' },
  ])
  expect(parsed.map(b => b.summary?.text)).toEqual(['ok', undefined, undefined, undefined])
  expect(parsed).toHaveLength(4)
})
