import { expect, mock, test } from 'claude-code/testing'

import type { Band } from './band-priority'

type Handlers = Parameters<Parameters<typeof test>[1]>[1]

const ROOT = '/repo'
const START = { cwd: ROOT, surface: 'terminal', isInteractive: true } as const
const BAND = {
  plugin: 'devcontainer',
  component: 'AbovePrompt',
  props: { hasSurvey: false, isWorking: false, maxRows: 10, bodyColumns: 120, scroll: { offset: 0, bodyRows: 10 }, view: {} },
} as const

// Recorded outputs of leftovers.sh --json (a fake script; no engine runs here).
const LEFTOVERS =
  '{"checked":true,"project":"shop","findings":[{"kind":"dangling","count":2},{"kind":"volumes","count":1}]}\n'
const WITH_DISK =
  '{"checked":true,"project":"shop","findings":[{"kind":"disk","free_gb":14,"floor_gb":20,"store":"/s"},{"kind":"dangling","count":2},{"kind":"volumes","count":1},{"kind":"buildah","count":21}]}\n'
const CLEAN = '{"checked":true,"project":"shop","findings":[]}\n'
const TEXT = 'ai-gent containers (report only, nothing removed):\n2 dangling image(s) of shop: `wt.py finish`\n'

type World = {
  json: string | 'timeout'
  text: string
  scriptExists: boolean
  denyCalls: boolean
  jsonRuns: number
  textRuns: number
  other: string[][]
  registered: string[]
  clock: ReturnType<typeof mock.clock>
}

// Nothing sits beneath the plugin: these hooks stand for the engine, and a
// leftovers.sh whose answers a test edits between steps.
function world(on: Handlers, others: Band[] | 'absent' | Record<string, Band[] | 'throws'> = 'absent'): World {
  const w: World = {
    json: LEFTOVERS,
    text: TEXT,
    scriptExists: true,
    denyCalls: false,
    jsonRuns: 0,
    textRuns: 0,
    other: [],
    registered: [],
    clock: mock.clock(on, { now: 10 * 86_400_000 }),
  }
  on('session.start', (_, e) => ({ cwd: e.cwd }))
  on('command.register', (_, e) => {
    w.registered.push(e.name)
    return { value: { command: e.name } }
  })
  on('fs.exists', () => ({ value: w.scriptExists }))
  on('state.get', async (_, e, next) => {
    if (e.plugin === 'devcontainer') return next(e)
    // An array stands for git's atom (the others read empty); a record gives each plugin's, 'throws' being a denied read.
    const held = others === 'absent' ? undefined : Array.isArray(others) ? (e.plugin === 'git' ? others : []) : others[e.plugin]
    if (held === 'throws') return { deny: 'no such plugin' }
    return { value: held === undefined ? { value: undefined, version: 0 } : { value: held, version: 1 } }
  })
  on('process.run', (_, e) => {
    const argv = [...e.argv]
    if (!argv[0]?.endsWith('/hooks/leftovers.sh')) {
      w.other.push(argv)
      return { value: { exitCode: 1, stdout: '', stderr: 'unexpected' } }
    }
    if (argv[1] === '--json') {
      w.jsonRuns += 1
      if (w.json === 'timeout') return { deny: 'timed out' }
      return { value: { exitCode: 0, stdout: w.json, stderr: '' } }
    }
    w.textRuns += 1
    return { value: { exitCode: 0, stdout: w.text, stderr: '' } }
  })
  on('tool.call', () => (w.denyCalls ? { deny: 'no' } : { result: {} }))
  on('turn.complete', (_, e) => ({ text: e.text }))
  on('ui.render', ($, e) => {
    const { Text } = $.ui.resolve(e)
    return <Text dimColor>another band</Text>
  })
  return w
}

const started = async ($: Parameters<Parameters<typeof test>[1]>[0], w: World) => {
  await $.session.start(START)
  await w.clock.settle()
}
const bash = ($: Parameters<Parameters<typeof test>[1]>[0], command: string) => $.tool.call({ tool: 'Bash', command } as never)
const turn = ($: Parameters<Parameters<typeof test>[1]>[0]) => $.turn.complete({ text: 'done', reason: 'answer' } as never)
const band = (text: RegExp) => ({ type: 'Text', text }) as const
const run = ($: Parameters<Parameters<typeof test>[1]>[0], command = 'containers') => $.command.run({ command, args: '' } as never)

test('the band says what leftovers.sh found, above whatever is beneath, on every surface', async ($, on) => {
  const w = world(on)
  await started($, w)
  for (const surface of ['terminal', 'desktop'] as const) {
    const ui = await $.ui.mount({ ...BAND, surface })
    expect(await ui.find(band(/containers: 2 dangling images · 1 volume of a gone worktree — \/containers/))).toBeDefined()
    expect(await ui.find(band(/another band/))).toBeDefined()
    await ui.unmount()
  }
  expect(w.jsonRuns).toBe(1)
  expect(w.other).toEqual([])
})

test('a disk finding leads, with its mark, in the script\'s order', async ($, on) => {
  const w = world(on)
  w.json = WITH_DISK
  await started($, w)
  const ui = await $.ui.mount({ ...BAND, surface: 'terminal' })
  expect(
    await ui.find(band(/containers: ⚠ disk 14 GB free, floor 20 · 2 dangling images · 1 volume of a gone worktree · 21 buildah \(any project\) — \/containers/)),
  ).toBeDefined()
  await ui.unmount()
})

test('clean, not checked and an unreadable answer draw nothing; /containers says which', async ($, on) => {
  const w = world(on)
  const none = async () => {
    const ui = await $.ui.mount({ ...BAND, surface: 'terminal' })
    expect(await ui.find(band(/containers/))).toBeUndefined()
    expect(await ui.find(band(/another band/))).toBeDefined()
    await ui.unmount()
  }
  w.json = CLEAN
  await started($, w)
  await none()
  expect((await run($)).text).toBe('No leftovers or disk problem reported for this project.')

  w.json = '{"checked":false,"reason":"no-engine"}\n'
  await run($)
  await bash($, 'podman build .')
  await w.clock.advance(5_000)
  await bash($, 'podman build .')
  await none()
  expect((await run($)).text).toContain('Nothing was checked (no-engine)')

  w.json = 'ai-gent containers (report only):\n2 dangling images\n'
  await w.clock.advance(5_000)
  await bash($, 'docker run x')
  await none()
  expect((await run($)).text).toBe('The report could not be read: leftovers.sh --json did not print JSON.')
})

test('an unknown kind is shown as its kind and count', async ($, on) => {
  const w = world(on)
  w.json = '{"checked":true,"findings":[{"kind":"networks","count":3}]}\n'
  await started($, w)
  const ui = await $.ui.mount({ ...BAND, surface: 'terminal' })
  expect(await ui.find(band(/containers: networks 3 — \/containers/))).toBeDefined()
  await ui.unmount()
})

test('the band yields to a survey and to a running turn, and shortens on a narrow terminal', async ($, on) => {
  const w = world(on)
  await started($, w)
  for (const props of [{ ...BAND.props, hasSurvey: true }, { ...BAND.props, isWorking: true }]) {
    const ui = await $.ui.mount({ ...BAND, props, surface: 'terminal' })
    expect(await ui.find(band(/containers/))).toBeUndefined()
    await ui.unmount()
  }
  const ui = await $.ui.mount({ ...BAND, props: { ...BAND.props, bodyColumns: 50 }, surface: 'terminal' })
  expect(await ui.find(band(/containers ⚠ 2 — \/containers/))).toBeDefined()
  await ui.unmount()
})

test('a build, a run or a worktree move reads again; other commands and quick repeats do not', async ($, on) => {
  const w = world(on)
  await started($, w)
  expect(w.jsonRuns).toBe(1)
  await w.clock.advance(5_000)
  await bash($, 'ls')
  await bash($, 'podman ps')
  expect(w.jsonRuns).toBe(1)

  w.json = '{"checked":true,"findings":[{"kind":"dangling","count":5}]}\n'
  await bash($, 'podman build -t x .')
  expect(w.jsonRuns).toBe(2)
  await bash($, 'docker rm abc')
  expect(w.jsonRuns).toBe(2) // within a second: joins the read that just ran
  const ui = await $.ui.mount({ ...BAND, surface: 'terminal' })
  expect(await ui.find(band(/containers: 5 dangling images/))).toBeDefined()
  await ui.unmount()

  await w.clock.advance(5_000)
  w.json = CLEAN
  await bash($, 'python3 plugins/agents/scripts/wt.py finish x')
  expect(w.jsonRuns).toBe(3)
  const gone = await $.ui.mount({ ...BAND, surface: 'terminal' })
  expect(await gone.find(band(/containers/))).toBeUndefined()
  await gone.unmount()
})

test('a failed call or a deny reads nothing again', async ($, on) => {
  const w = world(on)
  await started($, w)
  await w.clock.advance(5_000)
  w.denyCalls = true
  await bash($, 'podman build .')
  expect(w.jsonRuns).toBe(1)
})

test('a turn reads again at most every containersRefreshSeconds', async ($, on) => {
  const w = world(on)
  await started($, w)
  await turn($)
  expect(w.jsonRuns).toBe(1)
  await w.clock.advance(119_000)
  await turn($)
  expect(w.jsonRuns).toBe(1)
  await w.clock.advance(2_000)
  await turn($)
  expect(w.jsonRuns).toBe(2)
})

test('containersRefreshSeconds sets the throttle', { options: { containersRefreshSeconds: 10 } }, async ($, on) => {
  const w = world(on)
  await started($, w)
  await w.clock.advance(11_000)
  await turn($)
  expect(w.jsonRuns).toBe(2)
})

test('a timeout keeps the last text, and /containers says it timed out', async ($, on) => {
  const w = world(on)
  await started($, w)
  w.json = 'timeout'
  await w.clock.advance(200_000)
  await turn($)
  const ui = await $.ui.mount({ ...BAND, surface: 'terminal' })
  expect(await ui.find(band(/containers: 2 dangling images/))).toBeDefined()
  await ui.unmount()
  expect((await run($)).text).toBe('The report timed out: leftovers.sh did not answer in 10 seconds.')
})

test('a missing script draws nothing and /containers says so', async ($, on) => {
  const w = world(on)
  w.scriptExists = false
  await started($, w)
  const ui = await $.ui.mount({ ...BAND, surface: 'terminal' })
  expect(await ui.find(band(/containers/))).toBeUndefined()
  await ui.unmount()
  expect(w.jsonRuns).toBe(0)
  expect((await run($)).text).toContain('hooks/leftovers.sh is missing')
})

test('/containers prints each finding, the script\'s text and the pointer, starting nothing else', async ($, on) => {
  const w = world(on)
  await started($, w)
  expect(w.registered).toContain('containers')
  const before = { json: w.jsonRuns, text: w.textRuns }
  const text = (await run($)).text ?? ''
  expect(text).toContain('- 2 dangling images')
  expect(text).toContain('- 1 volume of a gone worktree')
  expect(text).toContain('wt.py finish')
  expect(text).toContain('wt.py resources')
  expect(text).not.toMatch(/prune/i)
  expect(w.jsonRuns).toBe(before.json + 1)
  expect(w.textRuns).toBe(before.text + 1)
  expect(w.other).toEqual([])
})

test('the band records its priority, and gives way to a crowded prompt area', { options: { bandCap: 1 } }, async ($, on) => {
  const w = world(on, [{ id: 'hygiene', plugin: 'git', priority: 'normal', since: 1 }])
  await started($, w)
  const ui = await $.ui.mount({ ...BAND, surface: 'terminal' })
  expect(await ui.find(band(/containers:/))).toBeUndefined()
  expect(await ui.find({ type: 'Text', text: '+1 more band (/bands)' })).toBeDefined()
  await ui.unmount()
})

const HOLDS: Record<string, Band> = {
  project: { id: 'holds', plugin: 'project', priority: 'hold', since: 1 },
  git: { id: 'hygiene', plugin: 'git', priority: 'hold', since: 1 },
  repo: { id: 'ci', plugin: 'repo', priority: 'hold', since: 1 },
}

for (const other of Object.keys(HOLDS)) {
  test(`a band from ${other} counts toward the cap: at a cap of one the containers band gives way and says so`, { options: { bandCap: 1 } }, async ($, on) => {
    const w = world(on, { [other]: [HOLDS[other] as Band] })
    await started($, w)
    const ui = await $.ui.mount({ ...BAND, surface: 'terminal' })
    expect(await ui.find(band(/containers:/))).toBeUndefined()
    expect(await ui.find({ type: 'Text', text: '+1 more band (/bands)' })).toBeDefined()
    await ui.unmount()
  })
}

test('a plugin that is absent or whose read throws adds no band, and the others still count', { options: { bandCap: 1 } }, async ($, on) => {
  const w = world(on, { project: 'throws', git: [HOLDS.git as Band] })
  await started($, w)
  const ui = await $.ui.mount({ ...BAND, surface: 'terminal' })
  expect(await ui.find(band(/containers:/))).toBeUndefined()
  expect(await ui.find({ type: 'Text', text: '+1 more band (/bands)' })).toBeDefined()
  await ui.unmount()
})

test('with every other plugin absent the containers band draws at a cap of one', { options: { bandCap: 1 } }, async ($, on) => {
  const w = world(on, {})
  await started($, w)
  const ui = await $.ui.mount({ ...BAND, surface: 'terminal' })
  expect(await ui.find(band(/containers:/))).toBeDefined()
  expect(await ui.find({ type: 'Text', text: /more band/ })).toBeUndefined()
  await ui.unmount()
})

test('exactly one plugin draws the more-line: this one when its band is the first hidden, the other otherwise', { options: { bandCap: 1 } }, async ($, on) => {
  const first = world(on, { project: [HOLDS.project as Band], repo: [{ id: 'ci', plugin: 'repo', priority: 'info', since: 1e15 }] })
  await started($, first)
  const ui = await $.ui.mount({ ...BAND, surface: 'terminal' })
  expect(await ui.find({ type: 'Text', text: '+2 more bands (/bands)' })).toBeDefined()
  await ui.unmount()
})

test('a hidden band behind another plugin\'s older hidden one leaves the line to that plugin', { options: { bandCap: 1 } }, async ($, on) => {
  const w = world(on, { project: [HOLDS.project as Band], git: [{ id: 'hygiene', plugin: 'git', priority: 'normal', since: 5 }] })
  await started($, w)
  const ui = await $.ui.mount({ ...BAND, surface: 'terminal' })
  expect(await ui.find(band(/containers:/))).toBeUndefined()
  expect(await ui.find({ type: 'Text', text: /more band/ })).toBeUndefined()
  await ui.unmount()
})

test('a disk finding outranks leftovers for the room', { options: { bandCap: 1 } }, async ($, on) => {
  const w = world(on, [{ id: 'hygiene', plugin: 'git', priority: 'normal', since: 1e12 }])
  w.json = WITH_DISK
  await started($, w)
  const ui = await $.ui.mount({ ...BAND, surface: 'terminal' })
  expect(await ui.find(band(/containers: ⚠ disk/))).toBeDefined()
  await ui.unmount()
})

test('no /bands command is registered here', async ($, on) => {
  const w = world(on)
  await started($, w)
  expect(w.registered).not.toContain('bands')
})

test('containersBand off reads and draws nothing, but the command stays', { options: { containersBand: false } }, async ($, on) => {
  const w = world(on)
  await started($, w)
  await bash($, 'podman build .')
  await turn($)
  expect(w.jsonRuns).toBe(0)
  const ui = await $.ui.mount({ ...BAND, surface: 'terminal' })
  expect(await ui.find(band(/containers/))).toBeUndefined()
  await ui.unmount()
  expect(w.registered).toContain('containers')
  expect((await run($)).text).toContain('- 2 dangling images')
})

test('containersCommand off keeps the band and registers no command', { options: { containersCommand: false } }, async ($, on) => {
  const w = world(on)
  await started($, w)
  expect(w.registered).not.toContain('containers')
  const ui = await $.ui.mount({ ...BAND, surface: 'terminal' })
  expect(await ui.find(band(/containers: 2 dangling images/))).toBeDefined()
  await ui.unmount()
})

test('guardUx off does not turn the band off', { options: { guardUx: false } }, async ($, on) => {
  const w = world(on)
  await started($, w)
  const ui = await $.ui.mount({ ...BAND, surface: 'terminal' })
  expect(await ui.find(band(/containers: 2 dangling images/))).toBeDefined()
  await ui.unmount()
})

test('both off registers nothing and reads nothing', { options: { containersBand: false, containersCommand: false } }, async ($, on) => {
  const w = world(on)
  await started($, w)
  expect(w.jsonRuns).toBe(0)
  expect(w.registered).toEqual([])
})
