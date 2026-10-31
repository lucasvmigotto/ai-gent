import { expect, test } from 'claude-code/testing'

import type { GuardRow } from '../types'

type Handlers = Parameters<Parameters<typeof test>[1]>[1]

const START = { cwd: '/repo', surface: 'terminal', isInteractive: true } as const
const PANE = { plugin: 'project', component: 'Pane', requestId: 'guards', props: { bodyColumns: 100, bodyRows: 30, dock: 'right', view: {} } } as const
const PLUGINS = ['git', 'db', 'secrets', 'repo', 'devcontainer'] as const

const at = (h: number, m: number) => new Date(2026, 9, 31, h, m).getTime()
const row = (over: Partial<GuardRow>): GuardRow => ({ at: at(14, 2), guard: 'git', decision: 'ask', entry: 'git push publishes', tool: 'Bash', verb: 'git push', outcome: 'asked', ...over })

type Logs = Partial<Record<(typeof PLUGINS)[number], GuardRow[] | 'throws'>>

// A plain object the test edits between steps; a plugin missing from it reads as absent.
function world(on: Handlers, logs: Logs) {
  const state = { logs: { ...logs }, opened: 0, placed: true }
  on('session.start', (_, e) => ({ cwd: e.cwd }))
  on('session.cwd', () => ({ value: '/repo' }))
  on('command.register', (_, e) => ({ value: { command: e.name } }))
  on('ui.open', () => {
    state.opened += 1
    return { value: { isPlaced: state.placed } }
  })
  on('ui.close', () => undefined)
  on('process.run', () => ({ value: { exitCode: 0, stdout: '/repo\n', stderr: '' } }))
  on('fs.stat', () => ({ deny: 'ENOENT' }))
  on('fs.exists', () => ({ value: false }))
  on('state.get', async (_, e, next) => {
    if (e.plugin === 'project') return next(e)
    const held = state.logs[e.plugin as (typeof PLUGINS)[number]]
    // A plugin missing from the map is not installed: the read throws.
    if (held === 'throws' || (held === undefined && !(e.plugin in state.logs))) return { deny: 'no such plugin' }
    return { value: held === undefined ? { value: undefined, version: 0 } : { value: { shape: 'guard-log-1', value: held }, version: 1 } }
  })
  return state
}

const run = async ($: any, args = '') => String((await $.command.run({ command: 'guards', args } as never)).text ?? '')

const ALL: Logs = {
  git: [row({ at: at(13, 41), entry: 'git push publishes' }), row({ at: at(13, 58), decision: 'deny', entry: 'hooks must not be bypassed', verb: 'git commit', outcome: null }), row({ at: at(14, 2) })],
  db: [row({ guard: 'db', at: at(13, 50), entry: 'bypasses dbrun', verb: 'psql' })],
  secrets: [],
  repo: [],
  devcontainer: [row({ guard: 'containers', at: at(14, 5), entry: 'generic', verb: null, tool: 'Read' })],
}

test('/guards lists the newest rows of all five plugins, newest first, with counts', async ($, on) => {
  world(on, ALL)
  await $.session.start(START)
  const out = await run($)
  expect(out).toContain('guards — 5 decisions this session (1 deny · 4 ask)')
  const lines = out.split('\n')
  const first = lines.findIndex(l => l.includes('14:05'))
  expect(first).toBe(1)
  expect(lines[2]).toContain('14:02')
  expect(out.indexOf('13:41')).toBeGreaterThan(out.indexOf('13:58'))
  expect(out).toContain('generic')
  expect(out).toContain('Most frequent')
  expect(out).toMatch(/2×\s+git:workflow\s+git push publishes\s+ask\s+\(git push ×2\)/)
  expect(out).not.toContain('not reporting')
})

test('a subset: an absent or denied plugin is named, the others are listed', async ($, on) => {
  world(on, { git: ALL.git, db: 'throws' })
  await $.session.start(START)
  const out = await run($)
  expect(out).toContain('3 decisions this session')
  expect(out).toContain('not reporting: db, secrets, repo, devcontainer')
  expect(out).toContain('git push')
})

test('a log never written reads as no decisions, not as absent', async ($, on) => {
  const w = world(on, { git: [row({})] })
  w.logs.db = undefined
  await $.session.start(START)
  const out = await run($)
  expect(out).toContain('not reporting: secrets, repo, devcontainer')
  expect(out).not.toContain('not reporting: db')
})

test('none reporting: says so, and never fails', async ($, on) => {
  world(on, {})
  await $.session.start(START)
  const out = await run($)
  expect(out).toContain('not reporting: git, db, secrets, repo, devcontainer')
  expect(out).toContain('no guard decisions this session')
})

test('empty logs: no guard decisions this session', async ($, on) => {
  world(on, { git: [], db: [], secrets: [], repo: [], devcontainer: [] })
  await $.session.start(START)
  expect(await run($)).toBe('no guard decisions this session')
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
  world(on, { git: [row({}), { nonsense: true } as never, null as never] })
  await $.session.start(START)
  expect(await run($)).toContain('1 decisions this session')
})

test('the pane draws the same list on terminal and desktop, and redraws on a new record', async ($, on) => {
  const w = world(on, ALL)
  await $.session.start(START)
  for (const surface of ['terminal', 'desktop'] as const) {
    const ui = await $.ui.mount({ ...PANE, surface })
    expect(await ui.find({ type: 'Text', text: /git push publishes/ })).toBeDefined()
    expect(await ui.find({ type: 'Text', text: /5 decisions/ })).toBeDefined()
    await ui.unmount()
  }
  w.logs.secrets = [row({ guard: 'secrets', decision: 'deny', entry: 'a brand new entry', verb: 'cat', outcome: null })]
  const ui = await $.ui.mount({ ...PANE, surface: 'terminal' })
  expect(await ui.find({ type: 'Text', text: /a brand new entry/ })).toBeDefined()
  await ui.unmount()
})

test('a narrow pane cuts the reason, never the verb or the decision', async ($, on) => {
  world(on, { git: [row({ entry: 'x'.repeat(80) })] })
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
