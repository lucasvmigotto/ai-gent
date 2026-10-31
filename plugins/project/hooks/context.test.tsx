import { expect, mock, test } from 'claude-code/testing'

type Handlers = Parameters<Parameters<typeof test>[1]>[1]

const ROOT = '/repo'
const START = { cwd: ROOT, surface: 'terminal', isInteractive: true } as const
const OPTIONS = { options: { contextMeter: true } }
const HINT = { isDraft: false, isWorking: false, hint: '? for shortcuts' }

type World = { percent: unknown; usageFails: boolean; usageCalls: number; toasts: string[]; tails: (string | undefined)[] }

// What sits beneath the plugin is the engine: its PromptHint draws the hint and the `tail` it was handed.
function world(on: Handlers, init: Partial<World> = {}): World {
  const w: World = { percent: 42, usageFails: false, usageCalls: 0, toasts: [], tails: [], ...init }
  mock.clock(on, { now: 1_800_000_000_000 })
  on('session.start', (_, e) => ({ cwd: e.cwd }))
  on('session.cwd', () => ({ value: ROOT }))
  on('turn.complete', (_, e) => ({ text: e.text }))
  on('session.usage', () => {
    w.usageCalls++
    if (w.usageFails) return { deny: 'no usage' }
    return { value: { startedAt: 0, context: { window: 200_000, percent: w.percent }, rateLimits: [] } } as never
  })
  on('fs.stat', () => ({ deny: 'ENOENT' }))
  on('fs.exists', () => ({ value: false }))
  on('ui.toast', (_, e) => {
    w.toasts.push(e.text)
    return { value: undefined }
  })
  on('process.run', () => ({ value: { exitCode: 0, stdout: '/repo\n', stderr: '' } }))
  on('ui.render', ($, e) => {
    const { Text } = $.ui.resolve(e)
    if (e.component === 'PromptHint') w.tails.push((e.props as { tail?: string }).tail)
    return <Text>{e.component === 'PromptHint' ? `${e.props.hint}${(e.props as { tail?: string }).tail ?? ''}` : 'beneath'}</Text>
  })
  return w
}

const turn = ($: Parameters<Parameters<typeof test>[1]>[0]) => $.turn.complete({ text: 'done', reason: 'answer' } as never)
const hint = ($: Parameters<Parameters<typeof test>[1]>[0], columns = 100, surface: 'terminal' | 'desktop' = 'terminal') =>
  $.ui.mount({ plugin: 'project', component: 'PromptHint', surface, props: HINT, viewport: { columns, rows: 30 } } as never)
const drawn = async (ui: Awaited<ReturnType<typeof hint>>) => ((await ui.find({ type: 'Text' } as never)) as { text?: string } | undefined)?.text ?? ''

for (const surface of ['terminal', 'desktop'] as const) {
  test(`a normal figure adds label, bar, used and remaining to the hint (${surface})`, OPTIONS, async ($, on) => {
    world(on)
    await $.session.start({ ...START, surface })
    const ui = await hint($, 100, surface)
    expect(await drawn(ui)).toMatch(/^\? for shortcuts {2}Context █{8}░{12} 42% used · 58% remaining$/)
    await ui.unmount()
  })
}

test('the meter follows the figure at each turn end, with one usage read per turn', OPTIONS, async ($, on) => {
  const w = world(on, { percent: 30 })
  await $.session.start(START)
  expect(w.usageCalls).toBe(1)
  const ui = await hint($)
  expect(await drawn(ui)).toContain('30% used')
  w.percent = 70
  await turn($)
  expect(w.usageCalls).toBe(2)
  expect(await drawn(ui)).toContain('70% used')
  expect(await drawn(ui)).toContain('⚠ approaching')
  w.percent = 91
  await turn($)
  expect(w.usageCalls).toBe(3)
  expect(await drawn(ui)).toContain('✖ critical')
  // drawing again reads nothing
  await ui.unmount()
  const again = await hint($)
  await drawn(again)
  expect(w.usageCalls).toBe(3)
  await again.unmount()
})

test('the nudge and the meter share one read, and the nudge threshold sets the colours', { options: { contextMeter: true, handoffNudgeContextPercent: 50 } }, async ($, on) => {
  const w = world(on, { percent: 55 })
  await $.session.start(START)
  const before = w.usageCalls
  await turn($)
  expect(w.usageCalls - before).toBe(1)
  expect(w.toasts.some(t => t.startsWith('Context 55% full'))).toBe(true)
  const ui = await hint($)
  expect(await drawn(ui)).toContain('✖ critical')
  await ui.unmount()
})

test('0 and 100 draw; a figure over 100 is clamped', OPTIONS, async ($, on) => {
  const w = world(on, { percent: 0 })
  await $.session.start(START)
  const ui = await hint($)
  expect(await drawn(ui)).toContain('0% used · 100% remaining')
  w.percent = 100
  await turn($)
  expect(await drawn(ui)).toContain('100% used · 0% remaining')
  w.percent = 130
  await turn($)
  expect(await drawn(ui)).toContain('100% used')
  await ui.unmount()
})

test('a missing, NaN, invalid or failed figure shows nothing, and clears an earlier one', OPTIONS, async ($, on) => {
  const w = world(on, { percent: 50 })
  await $.session.start(START)
  const ui = await hint($)
  expect(await drawn(ui)).toContain('50% used')
  for (const bad of [undefined, Number.NaN, 'x', null]) {
    w.percent = bad
    await turn($)
    expect(await drawn(ui)).toBe('? for shortcuts')
    w.percent = 50
    await turn($)
    expect(await drawn(ui)).toContain('50% used')
  }
  w.usageFails = true
  await turn($)
  expect(await drawn(ui)).toBe('? for shortcuts')
  await ui.unmount()
})

for (const [columns, pattern] of [
  [100, /Context █+░+ 42% used · 58% remaining/],
  [60, /Context █+░+ 42% used/],
  [30, /Context 42% used|ctx 42%/],
  [26, /ctx 42%/],
] as const) {
  test(`${columns} columns: the tail fits the row beside the hint`, OPTIONS, async ($, on) => {
    world(on)
    await $.session.start(START)
    const ui = await hint($, columns)
    const row = await drawn(ui)
    expect(row).toMatch(pattern)
    expect([...row].length).toBeLessThanOrEqual(columns)
    await ui.unmount()
  })
}

test('a row with no room for even the short form leaves the hint as it is', OPTIONS, async ($, on) => {
  world(on)
  await $.session.start(START)
  const ui = await hint($, 20)
  expect(await drawn(ui)).toBe('? for shortcuts')
  await ui.unmount()
})

test('chromeAscii draws the meter with ASCII only', { options: { contextMeter: true, chromeAscii: true } }, async ($, on) => {
  world(on, { percent: 90 })
  await $.session.start(START)
  const ui = await hint($)
  const row = await drawn(ui)
  expect(row).toContain('#')
  expect(row).toContain('!! critical')
  expect(row).not.toMatch(/[^\x20-\x7e]/)
  await ui.unmount()
})

test('contextMeter off: no hint change and no usage read beyond the nudge\'s own', { options: { contextMeter: false } }, async ($, on) => {
  const w = world(on)
  await $.session.start(START)
  expect(w.usageCalls).toBe(0)
  const ui = await hint($)
  expect(await drawn(ui)).toBe('? for shortcuts')
  await ui.unmount()
})

test('on by default: the hint line carries the meter', async ($, on) => {
  const w = world(on)
  await $.session.start(START)
  await turn($)
  expect(w.usageCalls).toBeGreaterThan(0)
  const ui = await hint($)
  expect(await drawn(ui)).toContain('used')
  await ui.unmount()
})

test('headless: no render and no error', OPTIONS, async ($, on) => {
  const w = world(on)
  await $.session.start({ cwd: ROOT, surface: null, isInteractive: false } as never)
  await turn($)
  expect(w.usageCalls).toBeGreaterThan(0)
})
