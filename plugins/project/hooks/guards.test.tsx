import { expect, test } from 'claude-code/testing'

import type { GuardRow } from '../types'

type Handlers = Parameters<Parameters<typeof test>[1]>[1]

const START = { cwd: '/repo', surface: 'terminal', isInteractive: true } as const
const PANE = { plugin: 'project', component: 'Pane', requestId: 'guards', props: { bodyColumns: 100, bodyRows: 30, dock: 'right', view: {} } } as const
const LOG = '/state/guard-log/s-1.jsonl'

const at = (h: number, m: number) => new Date(2026, 9, 31, h, m).getTime()
const row = (over: Partial<GuardRow>): GuardRow => ({ at: at(14, 2), guard: 'git', decision: 'ask', entry: 'git push publishes', tool: 'Bash', verb: 'git push', outcome: 'asked', ...over })

// The session's guard log as the guards write it: one JSON row per line, oldest first.
const lines = (rows: unknown[]) => rows.map(r => JSON.stringify(r)).join('\n') + '\n'

// A plain object the test edits between steps; `log` null is a session no guard wrote for.
function world(on: Handlers, rows: unknown[] | null) {
  const state = { log: rows === null ? null : lines(rows), opened: 0, placed: true, grant: null as string | null, logDir: '/state/guard-log' as string | undefined }
  on('session.start', (_, e) => ({ cwd: e.cwd }))
  on('session.cwd', () => ({ value: '/repo' }))
  on('session.id', () => ({ value: 's-1' }))
  on('env.get', (_, e) => ({ value: e.name === 'AI_GENT_GUARD_LOG_DIR' ? state.logDir : undefined }))
  on('command.register', (_, e) => ({ value: { command: e.name } }))
  on('ui.open', () => {
    state.opened += 1
    return { value: { isPlaced: state.placed } }
  })
  on('ui.close', () => undefined)
  on('tool.call', () => ({ result: {} }) as never)
  on('process.run', () => ({ value: { exitCode: 0, stdout: '/repo\n', stderr: '' } }))
  on('fs.stat', () => ({ deny: 'ENOENT' }))
  on('fs.exists', () => ({ value: false }))
  on('fs.read', (_, e) => {
    if (state.grant !== null && e.path === '/repo/.ai-gent/autonomy') return { value: state.grant }
    if (state.log !== null && e.path === LOG) return { value: state.log }
    return { deny: 'ENOENT' }
  })
  return state
}

const run = async ($: any, args = '') => String((await $.command.run({ command: 'guards', args } as never)).text ?? '')

const ALL: GuardRow[] = [
  row({ at: at(13, 41), entry: 'git push publishes' }),
  row({ guard: 'db', at: at(13, 50), entry: 'bypasses dbrun', verb: 'psql' }),
  row({ at: at(13, 58), decision: 'deny', entry: 'hooks must not be bypassed', verb: 'git commit', outcome: null }),
  row({ at: at(14, 2) }),
  row({ guard: 'containers', at: at(14, 5), entry: 'generic', verb: null, tool: 'Read' }),
]

test('/guards lists the newest rows of every guard, newest first, with counts', async ($, on) => {
  world(on, ALL)
  await $.session.start(START)
  const out = await run($)
  expect(out).toContain('guards — 5 decisions this session (1 deny · 4 ask)')
  const lines = out.split('\n')
  const first = lines.findIndex(l => l.includes('14:05'))
  expect(first).toBe(2)
  expect(lines[0]).toBe('autonomy: none')
  expect(lines[3]).toContain('14:02')
  expect(out.indexOf('13:41')).toBeGreaterThan(out.indexOf('13:58'))
  expect(out).toContain('generic')
  expect(out).toContain('Most frequent')
  expect(out).toMatch(/2×\s+git:workflow\s+git push publishes\s+ask\s+\(git push ×2\)/)
})

test('no log for this session: no guard decisions, never a failure', async ($, on) => {
  world(on, null)
  await $.session.start(START)
  expect(await run($)).toBe('autonomy: none\nno guard decisions this session')
})

test('the reader resolves the directory the guards write to', async ($, on) => {
  const w = world(on, ALL)
  w.logDir = undefined
  await $.session.start(START)
  expect(await run($)).toContain('no guard decisions this session')
})

test('only the newest 200 rows count', async ($, on) => {
  world(on, [...Array.from({ length: 50 }, () => row({ guard: 'db', entry: 'bypasses dbrun', verb: 'psql' })), ...Array.from({ length: 200 }, () => row({}))])
  await $.session.start(START)
  expect(await run($)).toContain('200 decisions this session')
})

test('an argument limits the rows and is clamped; the header counts every row', async ($, on) => {
  world(on, ALL)
  await $.session.start(START)
  const out = await run($, '2')
  expect(out).toContain('5 decisions')
  expect(out).toContain('newest 2')
  expect(out).not.toContain('13:41')
  expect(await run($, '0')).toMatch(/newest 1\b/)
  expect(await run($, '9999')).toMatch(/newest 5\b/)
  expect(await run($, 'x')).toMatch(/newest 5\b/)
})

test('guardLogLimit sets the default', { options: { guardLogLimit: 1 } }, async ($, on) => {
  world(on, ALL)
  await $.session.start(START)
  expect(await run($)).toMatch(/newest 1\b/)
})

test('the text states counts only, never a verdict on a guard', async ($, on) => {
  world(on, ALL)
  await $.session.start(START)
  expect(await run($)).not.toMatch(/strict|loosen|too (many|often)|noisy|should/i)
})

test('a malformed row is ignored, not shown', async ($, on) => {
  const w = world(on, [row({}), { nonsense: true }, null])
  w.log += 'not json\n'
  await $.session.start(START)
  expect(await run($)).toContain('1 decisions this session')
})

test('the pane draws the same list on terminal and desktop, and rereads the log on a new record', async ($, on) => {
  const w = world(on, ALL)
  await $.session.start(START)
  for (const surface of ['terminal', 'desktop'] as const) {
    const ui = await $.ui.mount({ ...PANE, surface })
    expect(await ui.find({ type: 'Text', text: /git push publishes/ })).toBeDefined()
    expect(await ui.find({ type: 'Text', text: /5 decisions/ })).toBeDefined()
    await ui.unmount()
  }
  // Opened by /guards-pane and left mounted: a row a guard writes shows after the next tool call.
  expect(String((await $.command.run({ command: 'guards-pane', args: '' } as never)).text)).toBe('Guards pane opened.')
  const ui = await $.ui.mount({ ...PANE, surface: 'terminal' })
  expect(await ui.find({ type: 'Text', text: /5 decisions/ })).toBeDefined()
  w.log += lines([row({ guard: 'secrets', decision: 'deny', entry: 'a brand new entry', verb: 'cat', outcome: null })])
  await $.tool.call({ tool: 'Bash', command: 'cat x' } as never)
  expect(await ui.find({ type: 'Text', text: /a brand new entry/ })).toBeDefined()
  expect(await ui.find({ type: 'Text', text: /6 decisions/ })).toBeDefined()
  await ui.unmount()
})

test('a narrow pane cuts the reason, never the verb or the decision', async ($, on) => {
  world(on, [row({ entry: 'x'.repeat(80) })])
  await $.session.start(START)
  const ui = await $.ui.mount({ ...PANE, surface: 'terminal', props: { ...PANE.props, bodyColumns: 40 } })
  expect(await ui.find({ type: 'Text', text: /ask\s+git\s+git push\s+x+…/ })).toBeDefined()
  await ui.unmount()
})

test('/guards-pane opens the pane, or prints the list where nothing can dock', async ($, on) => {
  const w = world(on, ALL)
  await $.session.start(START)
  expect(String((await $.command.run({ command: 'guards-pane', args: '' } as never)).text)).toBe('Guards pane opened.')
  expect(w.opened).toBe(1)
  w.placed = false
  const text = String((await $.command.run({ command: 'guards-pane', args: '' } as never)).text)
  expect(text).toContain('no pane here — /guards prints the list')
})

test('the autonomy line shows an active grant from the main checkout, in /guards and in the pane', async ($, on) => {
  const w = world(on, ALL)
  const expires = new Date(Date.now() + 5 * 3_600_000 + 60_000).toISOString().slice(0, 19) + 'Z'
  w.grant = `expires: ${expires}\nactions: push pr\n`
  await $.session.start(START)
  expect((await run($)).split('\n')[0]).toMatch(/^autonomy: push pr on develop dev, until \d{4}-\d\d-\d\d \d\d:\d\d UTC \(in 5h\)$/)
  const ui = await $.ui.mount({ ...PANE, surface: 'terminal' })
  expect(await ui.find({ type: 'Text', text: /^autonomy: push pr on develop dev/ })).toBeDefined()
  await ui.unmount()
})

test('an expired grant reads expired', async ($, on) => {
  const w = world(on, ALL)
  w.grant = 'expires: 2020-01-01T00:00:00Z\nactions: push\n'
  await $.session.start(START)
  expect((await run($)).split('\n')[0]).toBe('autonomy: expired')
})

test('the pane is not opened by itself', async ($, on) => {
  const w = world(on, ALL)
  await $.session.start(START)
  expect(w.opened).toBe(0)
})

test('with guardsPane off the pane says so and /guards still prints', { options: { guardsPane: false } }, async ($, on) => {
  world(on, ALL)
  await $.session.start(START)
  expect(String((await $.command.run({ command: 'guards-pane', args: '' } as never)).text)).toContain('pane is off')
  expect(await run($)).toContain('5 decisions')
})
