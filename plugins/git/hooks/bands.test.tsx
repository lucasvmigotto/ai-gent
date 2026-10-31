import { expect, mock, test } from 'claude-code/testing'

import type { Band } from './band-priority'

type Handlers = Parameters<Parameters<typeof test>[1]>[1]

const ROOT = '/repo'
const START = { cwd: ROOT, surface: 'terminal', isInteractive: true } as const
const BAND = {
  plugin: 'git',
  component: 'AbovePrompt',
  props: { hasSurvey: false, isWorking: false, maxRows: 10, bodyColumns: 120, scroll: { offset: 0, bodyRows: 10 }, view: {} },
} as const

const HOLD: Band = { id: 'holds', plugin: 'project', priority: 'hold', since: 1 }
const INFO: Band = { id: 'ci', plugin: 'repo', priority: 'info', since: 1 }

// main is one release behind; `others` stands for what other plugins recorded.
function world(on: Handlers, others: Band[] | 'throws' | 'absent' | Record<string, Band[] | 'throws'> = 'absent') {
  const registered: string[] = []
  const ok = (stdout: string) => ({ value: { exitCode: 0, stdout: `${stdout}\n`, stderr: '' } })
  mock.clock(on, { now: 1000 })
  on('session.start', (_, e) => ({ cwd: e.cwd }))
  on('command.register', (_, e) => {
    registered.push(e.name)
    return { value: { command: e.name } }
  })
  on('fs.stat', () => ({ deny: 'ENOENT' }))
  on('state.get', async (_, e, next) => {
    if (e.plugin === 'git') return next(e)
    // An array stands for project's atom; a record gives each plugin's ('throws' is a denied read).
    const held = others === 'absent' ? undefined : Array.isArray(others) ? (e.plugin === 'project' ? others : undefined) : others === 'throws' ? 'throws' : (others as Record<string, Band[] | 'throws'>)[e.plugin]
    if (held === 'throws') return { deny: 'no such plugin' }
    return { value: held === undefined ? { value: undefined, version: 0 } : { value: held, version: 1 } }
  })
  on('process.run', (_, e) => {
    const argv = [...e.argv]
    const line = argv.join(' ')
    if (line.includes('--git-common-dir')) return ok(`${ROOT}/.git`)
    if (argv[1] === 'for-each-ref') return ok('refs/heads/main\nrefs/remotes/origin/main')
    if (argv[1] === 'rev-list') return ok('0\t1')
    if (argv[1] === 'log') return ok('chore(release): 2.20.0')
    return { value: { exitCode: 1, stdout: '', stderr: 'unexpected' } }
  })
  on('ui.render', ($, e) => {
    const { Text } = $.ui.resolve(e)
    return <Text>beneath</Text>
  })
  return { registered }
}

const behind = { type: 'Text', text: /main is 1 behind origin\/main/ } as const
const more = (n: number) => ({ type: 'Text', text: `+${n} more band${n === 1 ? '' : 's'} (/bands)` }) as const

test('the hygiene band draws with room for it, and nothing is hidden', async ($, on) => {
  world(on, [HOLD])
  await $.session.start(START)
  const ui = await $.ui.mount({ ...BAND, surface: 'terminal' })
  expect(await ui.find(behind)).toBeDefined()
  expect(await ui.find({ type: 'Text', text: /more band/ })).toBeUndefined()
  await ui.unmount()
})

test('a hold above the cap hides the hygiene band, and one dim line says so', { options: { bandCap: 1 } }, async ($, on) => {
  world(on, [HOLD])
  await $.session.start(START)
  const ui = await $.ui.mount({ ...BAND, surface: 'terminal' })
  expect(await ui.find(behind)).toBeUndefined()
  expect(await ui.find(more(1))).toBeDefined()
  expect(await ui.find({ type: 'Text', text: 'beneath' })).toBeDefined()
  await ui.unmount()
})

test('an info band from another plugin gives way to the hygiene band', async ($, on) => {
  world(on, [HOLD, INFO])
  await $.session.start(START)
  const ui = await $.ui.mount({ ...BAND, surface: 'terminal' })
  expect(await ui.find(behind)).toBeDefined()
  expect(await ui.find(more(1))).toBeUndefined() // the info band's own plugin draws that line
  await ui.unmount()
})

for (const others of ['absent', 'throws'] as const) {
  test(`with the project plugin ${others} the band draws as before`, { options: { bandCap: 1 } }, async ($, on) => {
    world(on, others)
    await $.session.start(START)
    const ui = await $.ui.mount({ ...BAND, surface: 'terminal' })
    expect(await ui.find(behind)).toBeDefined()
    await ui.unmount()
  })
}

test('/bands lists every active band with its priority and whether it draws', { options: { bandCap: 1 } }, async ($, on) => {
  const w = world(on, [HOLD])
  await $.session.start(START)
  expect(w.registered).toContain('bands')
  const text = (await $.command.run({ command: 'bands', args: '' } as never)).text ?? ''
  expect(text).toContain('holds (project, hold): shown')
  expect(text).toContain('hygiene (git, normal): hidden')
})

test('the /bands command can be turned off', { options: { bandsCommand: false } }, async ($, on) => {
  const w = world(on)
  await $.session.start(START)
  expect(w.registered).not.toContain('bands')
})

const THEIRS: Record<string, Band> = {
  project: HOLD,
  repo: { id: 'ci', plugin: 'repo', priority: 'failure', since: 3 },
  devcontainer: { id: 'containers', plugin: 'devcontainer', priority: 'info', since: 4 },
}

for (const other of Object.keys(THEIRS)) {
  const { id, priority } = THEIRS[other] as Band
  test(`a band from ${other} counts toward the cap: at a cap of one the hygiene band gives way and says so`, { options: { bandCap: 1 } }, async ($, on) => {
    // the hygiene band is normal; a failure or hold outranks it, and an info band does not
    world(on, { [other]: [THEIRS[other] as Band] })
    await $.session.start(START)
    const ui = await $.ui.mount({ ...BAND, surface: 'terminal' })
    const outranks = priority !== 'info'
    expect(await ui.find(behind) === undefined).toBe(outranks)
    expect(await ui.find(more(1)) !== undefined).toBe(outranks)
    await ui.unmount()
    const text = (await $.command.run({ command: 'bands', args: '' } as never)).text ?? ''
    expect(text).toContain(`${id} (${other}, ${priority}): ${outranks ? 'shown' : 'hidden'}`)
  })
}

test('all three other plugins count: /bands shows the set the others draw', async ($, on) => {
  world(on, { project: [HOLD], repo: [THEIRS.repo as Band], devcontainer: [THEIRS.devcontainer as Band] })
  await $.session.start(START)
  const text = (await $.command.run({ command: 'bands', args: '' } as never)).text ?? ''
  expect(text).toContain('4 active bands, at most 2 shown')
  expect(text).toContain('holds (project, hold): shown')
  expect(text).toContain('ci (repo, failure): shown')
  expect(text).toContain('hygiene (git, normal): hidden')
  expect(text).toContain('containers (devcontainer, info): hidden')
})

test('the hygiene band owns the one more-line when it is the first hidden band', { options: { bandCap: 1 } }, async ($, on) => {
  world(on, { project: [HOLD], devcontainer: [THEIRS.devcontainer as Band] })
  await $.session.start(START)
  const ui = await $.ui.mount({ ...BAND, surface: 'terminal' })
  expect(await ui.find(behind)).toBeUndefined()
  expect(await ui.find(more(2))).toBeDefined()
  await ui.unmount()
})

test('a plugin that throws or is absent adds no band, and the others still count', { options: { bandCap: 1 } }, async ($, on) => {
  world(on, { project: 'throws', repo: [THEIRS.repo as Band] })
  await $.session.start(START)
  const text = (await $.command.run({ command: 'bands', args: '' } as never)).text ?? ''
  expect(text).toContain('2 active bands')
  expect(text).toContain('ci (repo, failure): shown')
})

test('with the project plugin leading, the hygiene band folds into its workspace line and /bands says so', async ($, on) => {
  world(on, { project: true as never })
  await $.session.start(START)
  const ui = await $.ui.mount({ ...BAND, surface: 'terminal' })
  expect(await ui.find(behind)).toBeUndefined()
  expect(await ui.find({ type: 'Text', text: /more band/ })).toBeUndefined()
  await ui.unmount()
  expect((await $.command.run({ command: 'bands', args: '' } as never)).text).toContain('hygiene (git, normal): in the workspace line')
})
