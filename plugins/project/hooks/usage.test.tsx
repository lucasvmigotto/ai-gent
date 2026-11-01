import { expect, mock, test } from 'claude-code/testing'

import type { Band } from './band-priority'

type Handlers = Parameters<Parameters<typeof test>[1]>[1]
type Ctx = Parameters<Parameters<typeof test>[1]>[0]

const ROOT = '/repo'
const NOW = 1_800_000_000_000
const START = { cwd: ROOT, surface: 'terminal', isInteractive: true } as const
const HINT = { isDraft: false, isWorking: false, hint: '? for shortcuts' }
const PROPS = { hasSurvey: false, isWorking: false, maxRows: 10, bodyColumns: 100, scroll: { offset: 0, bodyRows: 10 }, view: {} }
const CI_OK: Band = { id: 'ci', plugin: 'repo', priority: 'normal', since: 1, summary: { text: 'CI passing', tone: 'ok' } }

type Limit = { kind: string; percentUsed: number; resetsAt?: string }
type World = { percent: unknown; limits: unknown; usageCalls: number }

const iso = (seconds: number) => new Date(NOW + seconds * 1000).toISOString()

function world(on: Handlers, init: Partial<World> = {}, bands: Band[] = []): World {
  const w: World = { percent: 18, limits: [], usageCalls: 0, ...init }
  mock.clock(on, { now: NOW })
  on('session.start', (_, e) => ({ cwd: e.cwd }))
  on('session.cwd', () => ({ value: ROOT }))
  on('session.measure', (_, e) => ({ changed: e.changed }))
  on('turn.complete', (_, e) => ({ text: e.text }))
  on('session.usage', () => {
    w.usageCalls++
    return { value: { startedAt: 0, context: { window: 200_000, percent: w.percent }, rateLimits: w.limits } } as never
  })
  on('command.register', (_, e) => ({ value: { command: e.name } }))
  on('fs.stat', () => ({ deny: 'ENOENT' }))
  on('fs.exists', () => ({ value: false }))
  on('ui.toast', () => ({ value: undefined }))
  on('process.run', () => ({ value: { exitCode: 0, stdout: '/repo\n', stderr: '' } }))
  on('state.get', async (_, e, next) => {
    if (e.plugin === 'project') return next(e)
    return { value: { value: e.plugin === 'repo' ? bands : undefined, version: bands.length > 0 && e.plugin === 'repo' ? 1 : 0 } }
  })
  on('ui.render', ($, e) => {
    const { Text } = $.ui.resolve(e)
    return <Text>{e.component === 'PromptHint' ? `${e.props.hint}${(e.props as { tail?: string }).tail ?? ''}` : 'beneath'}</Text>
  })
  return w
}

const turn = ($: Ctx) => $.turn.complete({ text: 'done', reason: 'answer' } as never)
const hint = ($: Ctx, columns = 200, isWorking = false) =>
  $.ui.mount({ plugin: 'project', component: 'PromptHint', surface: 'terminal', props: { ...HINT, isWorking }, viewport: { columns, rows: 30 } } as never)
const above = ($: Ctx, bodyColumns = 100) => $.ui.mount({ plugin: 'project', component: 'AbovePrompt', surface: 'terminal', props: { ...PROPS, bodyColumns } } as never)
const drawn = async (ui: Awaited<ReturnType<typeof hint>>) => ((await ui.find({ type: 'Text' } as never)) as { text?: string } | undefined)?.text ?? ''
const texts = async (ui: Awaited<ReturnType<typeof hint>>) => ((await ui.findAll({ type: 'Text' } as never)) as { text?: string }[]).map(t => t.text ?? '')

const FIVE: Limit = { kind: 'five_hour', percentUsed: 41, resetsAt: iso(4 * 3600 + 12 * 60) }
const WEEK: Limit = { kind: 'seven_day', percentUsed: 12, resetsAt: iso(2 * 86400) }

test('the five-hour bar replaces the context bar on the hint line', async ($, on) => {
  world(on, { limits: [FIVE] })
  await $.session.start(START)
  const ui = await hint($, 80)
  const row = await drawn(ui)
  expect(row).toMatch(/^\? for shortcuts {2}◷ [█░]+ 41% ↻ in 4h 12m$/)
  expect(row).not.toContain('◐')
  await ui.unmount()
})

test('without a five-hour reading the context meter stands in', async ($, on) => {
  world(on, { limits: [WEEK] })
  await $.session.start(START)
  const ui = await hint($, 100)
  expect(await drawn(ui)).toMatch(/◐ [█░]+ 18%$/)
  await ui.unmount()
})

test('session.measure pushes the windows into the tail with no extra usage read', async ($, on) => {
  const w = world(on)
  await $.session.start(START)
  expect(w.usageCalls).toBe(1)
  const ui = await hint($, 100)
  expect(await drawn(ui)).toContain('◐')
  await $.session.measure({ context: { window: 200_000, percent: 18 }, rateLimits: [{ ...FIVE, percentUsed: 76 }], changed: ['rateLimits'] } as never)
  expect(await drawn(ui)).toMatch(/◷ [█░]+ 76% ↻ in 4h 12m ⚠$/)
  await $.session.measure({ context: { window: 200_000, percent: 18 }, rateLimits: [], changed: ['rateLimits'] } as never)
  expect(await drawn(ui)).toContain('◐')
  expect(w.usageCalls).toBe(1)
  await ui.unmount()
})

test('the turn-end read seeds the windows too, one read per turn', async ($, on) => {
  const w = world(on)
  await $.session.start(START)
  w.limits = [{ ...FIVE, percentUsed: 95 }]
  await turn($)
  expect(w.usageCalls).toBe(2)
  const ui = await hint($, 100)
  expect(await drawn(ui)).toContain('✖')
  await ui.unmount()
})

test('invalid readings clear the window', async ($, on) => {
  const w = world(on, { limits: [FIVE] })
  await $.session.start(START)
  w.limits = [{ kind: 'five_hour', percentUsed: Number.NaN }]
  await turn($)
  const ui = await hint($, 100)
  expect(await drawn(ui)).toContain('◐')
  await ui.unmount()
})

for (const [columns, week] of [[200, true], [140, true], [100, true], [80, true], [60, true], [40, false]] as const) {
  test(`${columns} columns: ${week ? 'both windows' : 'the five-hour window only'}; working, the engine's whole row is counted`, async ($, on) => {
    world(on, { limits: [FIVE, WEEK] })
    await $.session.start(START)
    const idle = await hint($, columns, false)
    const tail = (await drawn(idle)).slice('? for shortcuts'.length)
    expect(tail.includes('7d')).toBe(week)
    if (tail !== '') expect(tail.includes('◷')).toBe(true)
    expect([...tail].length - 2 + [...'? for shortcuts'].length + 3).toBeLessThanOrEqual(columns)
    await idle.unmount()
    const busy = await hint($, columns, true)
    const busyTail = (await drawn(busy)).slice('? for shortcuts'.length)
    // room = columns - max(hint, 60) - 3: nothing the engine would cut
    expect(Math.max(0, [...busyTail].length - 2)).toBeLessThanOrEqual(Math.max(0, columns - 60 - 3))
    if (columns < 100) expect(busyTail).not.toContain('7d')
    if (columns <= 63) expect(busyTail).toBe('')
    await busy.unmount()
  })
}

test('ASCII on the hint line', { options: { chromeAscii: true } }, async ($, on) => {
  world(on, { limits: [{ ...FIVE, percentUsed: 95 }, WEEK] })
  await $.session.start(START)
  const ui = await hint($, 200)
  const row = await drawn(ui)
  expect(row).toMatch(/~ #+-* 95% r in 4h 12m !! \| = 7d 12% r /)
  expect(row).not.toMatch(/[^\x20-\x7e]/)
  await ui.unmount()
})

test('usageMeter off: the context bar stays and the windows are not drawn', { options: { usageMeter: false } }, async ($, on) => {
  world(on, { limits: [FIVE, WEEK] })
  await $.session.start(START)
  const ui = await hint($, 100)
  expect(await drawn(ui)).toMatch(/◐ [█░]+ 18%$/)
  await ui.unmount()
})

test('contextMeter off: the five-hour bar alone, no fallback and no Details row', { options: { contextMeter: false } }, async ($, on) => {
  const w = world(on, { limits: [FIVE, WEEK] }, [CI_OK])
  await $.session.start(START)
  const ui = await hint($, 100)
  expect(await drawn(ui)).toContain('◷')
  await ui.unmount()
  w.limits = []
  await turn($)
  const again = await hint($, 100)
  expect(await drawn(again)).toBe('? for shortcuts')
  await again.unmount()
  const bar = await above($)
  await bar.press({ key: 'workspace-details' })
  expect((await texts(bar)).join('\n')).not.toContain('context')
  await bar.unmount()
})

test('Details closed shows nothing extra; open shows the context and the week', async ($, on) => {
  world(on, { limits: [FIVE, WEEK] }, [CI_OK])
  await $.session.start(START)
  const ui = await above($)
  expect((await texts(ui)).join('\n')).not.toContain('context')
  await ui.press({ key: 'workspace-details' })
  const rows = (await texts(ui)).join('\n')
  expect(rows).toMatch(/◐ [█░]+ 18% context · ▦ 7d [█░]+ 12%/)
  await ui.press({ key: 'workspace-details' })
  expect((await texts(ui)).join('\n')).not.toContain('context')
  await ui.unmount()
})

test('a critical context or week shows while Details is closed', async ($, on) => {
  world(on, { percent: 85, limits: [FIVE, { kind: 'seven_day', percentUsed: 93 }] }, [CI_OK])
  await $.session.start(START)
  const ui = await above($)
  const all = (await texts(ui)).join('\n')
  expect(all).toContain('⚠ ✖ context 85% · ✖ 7d 93%')
  expect(all).not.toContain('▦')
  await ui.unmount()
})

test('with no bands the line is just the Details button; with no figures there is no line', async ($, on) => {
  const w = world(on, { limits: [WEEK] })
  await $.session.start(START)
  const ui = await above($)
  expect(await ui.find({ type: 'Button', props: { label: '▸ Details' } } as never)).toBeDefined()
  await ui.unmount()
  w.percent = undefined
  w.limits = []
  await turn($)
  const empty = await above($)
  expect(await empty.find({ type: 'Button' } as never)).toBeUndefined()
  await empty.unmount()
})

test('narrow widths keep the Details rows inside the width', async ($, on) => {
  world(on, { limits: [FIVE, WEEK] }, [CI_OK])
  await $.session.start(START)
  for (const columns of [100, 60, 40, 24]) {
    const ui = await above($, columns)
    await ui.press({ key: 'workspace-details' })
    for (const row of await texts(ui)) if (row.includes('◐') || row.includes('7d')) expect([...row].length).toBeLessThanOrEqual(columns)
    await ui.unmount()
  }
})

test('chromeAscii draws the Details rows with ASCII only', { options: { chromeAscii: true } }, async ($, on) => {
  world(on, { percent: 85, limits: [FIVE, WEEK] }, [CI_OK])
  await $.session.start(START)
  const ui = await above($)
  await ui.press({ key: 'workspace-details' })
  const all = (await texts(ui)).join('\n')
  expect(all).toContain('= 7d')
  expect(all).not.toMatch(/[^\x20-\x7e\n]/)
  await ui.unmount()
})

test('headless: nothing renders, the windows are still recorded without error', async ($, on) => {
  const w = world(on, { limits: [FIVE] })
  await $.session.start({ cwd: ROOT, surface: null, isInteractive: false } as never)
  await $.session.measure({ context: { window: 200_000, percent: 18 }, rateLimits: [FIVE], changed: ['rateLimits'] } as never)
  await turn($)
  expect(w.usageCalls).toBe(2)
})
