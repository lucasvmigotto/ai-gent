import { expect, mock, test } from 'claude-code/testing'

import type { Band } from './band-priority'

type Handlers = Parameters<Parameters<typeof test>[1]>[1]

const ROOT = '/repo'
const HEAD = 'a'.repeat(40)
const START = { cwd: ROOT, surface: 'terminal', isInteractive: true } as const
const BAND = {
  plugin: 'git',
  component: 'AbovePrompt',
  props: { hasSurvey: false, isWorking: false, maxRows: 10, bodyColumns: 120, scroll: { offset: 0, bodyRows: 10 }, view: {} },
} as const

const target = (kind: string, label: string) => ({ kind, label, from: '1.0.0', to: '1.1.0', bump: 'minor' })
const plan = (over: Record<string, unknown> = {}) =>
  JSON.stringify({ version: '2.23.0', bump: 'minor', since: '2.22.0', targets: [target('plugin', 'git'), target('plugin', 'repo'), target('plugin', 'agents')], changelog: 'generated from commits', ...over })

const MINOR = plan()
const PATCH = plan({ version: '2.22.1', bump: 'patch', targets: [target('plugin', 'git')] })
const FIRST = plan({ version: '0.1.0', since: null, targets: [] })
const SOURCE = plan({ version: '1.4.0', since: '1.3.2', targets: [target('source', 'VERSION')] })

type World = {
  runs: string[][]
  branch: string
  head: string
  tags: string
  script: boolean
  release: { exit: number; stdout: string; stderr: string; throws: boolean }
  others: Band[]
  behind: number
  clock: ReturnType<typeof mock.clock>
}

const scripted = (w: World) => w.runs.filter(a => a[0] === 'python3')

// Nothing sits beneath the plugin: these hooks stand for the engine and a
// repository at ROOT whose answers a test edits between steps.
function world(on: Handlers): World {
  const w: World = {
    runs: [],
    branch: 'dev',
    head: HEAD,
    tags: '2.21.0\n2.22.0',
    script: true,
    release: { exit: 0, stdout: MINOR, stderr: '', throws: false },
    others: [],
    behind: 0,
    clock: mock.clock(on, { now: 1_000_000 }),
  }
  const ok = (stdout: string) => ({ value: { exitCode: 0, stdout: `${stdout}\n`, stderr: '' } })
  on('session.start', (_, e) => ({ cwd: e.cwd }))
  on('command.register', (_, e) => ({ value: { command: e.name } }))
  on('fs.stat', () => ({ deny: 'ENOENT' }))
  on('fs.exists', () => ({ value: w.script }))
  on('state.get', async (_, e, next) => {
    if (e.plugin !== 'project') return next(e)
    return { value: { value: w.others, version: 1 } }
  })
  on('process.run', (_, e) => {
    const argv = [...e.argv]
    w.runs.push(argv)
    const line = argv.join(' ')
    if (line.includes('--show-toplevel')) return ok(ROOT)
    if (line.includes('--abbrev-ref')) return ok(w.branch)
    if (line === 'git rev-parse HEAD') return ok(w.head)
    if (argv[1] === 'tag') return ok(w.tags)
    if (argv[1] === 'for-each-ref') return ok('refs/heads/dev\nrefs/remotes/origin/dev')
    if (argv[1] === 'rev-list') return ok(`0\t${w.behind}`)
    if (line.includes('--git-common-dir')) return ok(`${ROOT}/.git`)
    if (argv[0] === 'git') return { value: { exitCode: 1, stdout: '', stderr: 'unexpected' } }
    if (argv[0] === 'python3') {
      if (w.release.throws) return { deny: 'ENOENT' }
      return { value: { exitCode: w.release.exit, stdout: w.release.stdout, stderr: w.release.stderr } }
    }
    return { value: { exitCode: 1, stdout: '', stderr: 'unexpected' } }
  })
  on('tool.call', () => ({ result: {} }))
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

const turn = ($: Parameters<Parameters<typeof test>[1]>[0]) => $.turn.complete({ text: 'done', reason: 'answer' } as never)

const text = async ($: Parameters<Parameters<typeof test>[1]>[0], props: Partial<typeof BAND.props> = {}, surface: 'terminal' | 'desktop' = 'terminal') => {
  const ui = await $.ui.mount({ ...BAND, props: { ...BAND.props, ...props }, surface })
  const found = await ui.find({ type: 'Text', text: /next|release preview/ })
  await ui.unmount()
  return found
}

const NEXT = /next release 2\.23\.0 \(minor, since 2\.22\.0\) · 3 plugins bump — \/release-preview for the plan/

// ---------------------------------------------------------------- story 1

test('a releasable plan shows the version, bump, base tag and plugin count, on every surface', async ($, on) => {
  const w = world(on)
  await started($, w)
  for (const surface of ['terminal', 'desktop'] as const) {
    const ui = await $.ui.mount({ ...BAND, surface })
    expect(await ui.find({ type: 'Text', text: NEXT })).toBeDefined()
    expect(await ui.find({ type: 'Text', text: /another band/ })).toBeDefined()
    await ui.unmount()
  }
  expect(scripted(w)).toEqual([['python3', `${ROOT}/scripts/release.py`, '--dry-run', '--json']])
})

test('nothing releasable (exit 3) draws no band', async ($, on) => {
  const w = world(on)
  w.release = { exit: 3, stdout: '', stderr: 'no release needed', throws: false }
  await started($, w)
  expect(await text($)).toBeUndefined()
  expect(scripted(w).length).toBe(1)
})

test('a failed run is one dim line with the first stderr line, and is not retried for the same HEAD', async ($, on) => {
  const w = world(on)
  w.release = { exit: 1, stdout: '', stderr: 'release.py: error: boom\nmore', throws: false }
  await started($, w)
  expect(await text($)).toBeDefined()
  const ui = await $.ui.mount({ ...BAND, surface: 'terminal' })
  expect(await ui.find({ type: 'Text', text: 'release preview failed (exit 1): release.py: error: boom' })).toBeDefined()
  await ui.unmount()
  await turn($)
  await w.clock.settle()
  expect(scripted(w).length).toBe(1)
})

test('exit 2 from an old script says how to refresh it; a start failure and stray stdout fail too', async ($, on) => {
  const w = world(on)
  w.release = { exit: 2, stdout: '', stderr: 'release.py: error: unrecognized arguments: --json', throws: false }
  await started($, w)
  let ui = await $.ui.mount({ ...BAND, surface: 'terminal' })
  expect(await ui.find({ type: 'Text', text: /exit 2.*unrecognized arguments: --json — re-run devsecops:pipeline/ })).toBeDefined()
  await ui.unmount()

  w.head = 'b'.repeat(40)
  w.release = { exit: 0, stdout: 'release 2.23.0 (minor)', stderr: '', throws: false }
  await $.tool.call({ tool: 'Bash', command: 'git commit -m x' } as never)
  await w.clock.settle()
  ui = await $.ui.mount({ ...BAND, surface: 'terminal' })
  expect(await ui.find({ type: 'Text', text: /printed no plan/ })).toBeDefined()
  await ui.unmount()

  w.head = 'c'.repeat(40)
  w.release.throws = true
  await $.tool.call({ tool: 'Bash', command: 'git commit -m y' } as never)
  await w.clock.settle()
  ui = await $.ui.mount({ ...BAND, surface: 'terminal' })
  expect(await ui.find({ type: 'Text', text: /could not be started/ })).toBeDefined()
  await ui.unmount()
})

test('no scripts/release.py runs nothing and shows nothing', async ($, on) => {
  const w = world(on)
  w.script = false
  await started($, w)
  expect(scripted(w)).toEqual([])
  expect(await text($)).toBeUndefined()
})

test('a feature branch is not previewed, at session start or after a turn', async ($, on) => {
  const w = world(on)
  w.branch = 'feat/x'
  await started($, w)
  await turn($)
  await w.clock.settle()
  expect(scripted(w)).toEqual([])
  expect(await text($)).toBeUndefined()
})

test('a plan with no plugin target omits the count; the first release says so; narrow is short', async ($, on) => {
  const w = world(on)
  w.release.stdout = SOURCE
  await started($, w)
  let ui = await $.ui.mount({ ...BAND, surface: 'terminal' })
  expect(await ui.find({ type: 'Text', text: 'next release 1.4.0 (minor, since 1.3.2) — /release-preview for the plan' })).toBeDefined()
  await ui.unmount()

  w.head = 'b'.repeat(40)
  w.release.stdout = FIRST
  await $.tool.call({ tool: 'Bash', command: 'git commit -m x' } as never)
  await w.clock.settle()
  ui = await $.ui.mount({ ...BAND, surface: 'terminal' })
  expect(await ui.find({ type: 'Text', text: 'next release 0.1.0 (minor, first release) — /release-preview for the plan' })).toBeDefined()
  await ui.unmount()

  w.head = 'c'.repeat(40)
  w.release.stdout = PATCH
  await $.tool.call({ tool: 'Bash', command: 'git commit -m y' } as never)
  await w.clock.settle()
  ui = await $.ui.mount({ ...BAND, props: { ...BAND.props, bodyColumns: 40 }, surface: 'terminal' })
  expect(await ui.find({ type: 'Text', text: 'next 2.22.1 (patch)' })).toBeDefined()
  await ui.unmount()
})

test('it yields to a survey and to a running turn', async ($, on) => {
  const w = world(on)
  await started($, w)
  expect(await text($, { hasSurvey: true })).toBeUndefined()
  expect(await text($, { isWorking: true })).toBeUndefined()
})

// ---------------------------------------------------------------- story 2

test('the script runs once per HEAD and tag list, however many turns complete or time passes', async ($, on) => {
  const w = world(on)
  await started($, w)
  for (let i = 0; i < 3; i++) await turn($)
  await w.clock.advance(10 * 60_000)
  await w.clock.settle()
  expect(scripted(w).length).toBe(1)
})

test('a command that moved refs reads HEAD and the tags again, and runs only if either changed', async ($, on) => {
  const w = world(on)
  await started($, w)
  await $.tool.call({ tool: 'Bash', command: 'git status' } as never)
  await $.tool.call({ tool: 'Bash', command: 'git commit --amend --no-edit' } as never)
  await w.clock.settle()
  expect(scripted(w).length).toBe(1)

  w.tags = '2.21.0\n2.22.0\n2.23.0'
  await $.tool.call({ tool: 'Bash', command: 'git tag 2.23.0' } as never)
  await w.clock.settle()
  expect(scripted(w).length).toBe(2)

  w.head = 'b'.repeat(40)
  w.release.exit = 3
  await $.tool.call({ tool: 'Bash', command: 'git pull --ff-only' } as never)
  await w.clock.settle()
  expect(scripted(w).length).toBe(3)
  expect(await text($)).toBeUndefined()
})

test('leaving the chain clears the band, and coming back computes again', async ($, on) => {
  const w = world(on)
  await started($, w)
  w.branch = 'feat/x'
  await $.tool.call({ tool: 'Bash', command: 'git switch feat/x' } as never)
  await w.clock.settle()
  expect(await text($)).toBeUndefined()
  w.branch = 'main'
  await $.tool.call({ tool: 'Bash', command: 'git switch main' } as never)
  await w.clock.settle()
  expect(scripted(w).length).toBe(2)
  expect(await text($)).toBeDefined()
})

test('nothing but a dry run ever runs, and git is only read', async ($, on) => {
  const w = world(on)
  await started($, w)
  for (const argv of scripted(w)) expect(argv.slice(2)).toEqual(['--dry-run', '--json'])
  const READ_ONLY = new Set(['rev-parse', 'tag', 'for-each-ref', 'rev-list', 'log'])
  for (const argv of w.runs.filter(a => a[0] === 'git')) expect(READ_ONLY.has(argv[1] ?? ''), argv.join(' ')).toBe(true)
  expect(w.runs.filter(a => a[1] === 'tag').every(a => a.includes('--list'))).toBe(true)
})

// ---------------------------------------------------------------- story 3

test('Hide keeps the band away until HEAD changes', async ($, on) => {
  const w = world(on)
  await started($, w)
  const ui = await $.ui.mount({ ...BAND, surface: 'terminal' })
  await ui.press({ key: 'hide' } as never)
  await ui.unmount()
  expect(await text($)).toBeUndefined()
  await turn($)
  expect(await text($)).toBeUndefined()

  w.head = 'b'.repeat(40)
  await $.tool.call({ tool: 'Bash', command: 'git commit -m x' } as never)
  await w.clock.settle()
  expect(await text($)).toBeDefined()
})

test('releaseBand off runs nothing for it, and /release-preview still works', { options: { releaseBand: false } }, async ($, on) => {
  const w = world(on)
  await started($, w)
  await $.tool.call({ tool: 'Bash', command: 'git commit -m x' } as never)
  await w.clock.settle()
  expect(scripted(w)).toEqual([])
  expect(await text($)).toBeUndefined()
  w.release.stdout = 'release 2.23.0 (minor)'
  expect((await $.command.run({ command: 'release-preview', args: '' } as never)).text).toBe('release 2.23.0 (minor)')
})

test('the local branch being behind origin is said, never fixed', async ($, on) => {
  const w = world(on)
  w.behind = 2
  await started($, w)
  const ui = await $.ui.mount({ ...BAND, surface: 'terminal' })
  expect(await ui.find({ type: 'Text', text: /3 plugins bump \(local branch is behind origin\) — \/release-preview/ })).toBeDefined()
  await ui.unmount()
  for (const argv of w.runs.filter(a => a[0] === 'git')) expect(['fetch', 'pull', 'merge']).not.toContain(argv[1])
})

// ---------------------------------------------------------- band priority

test('it yields to higher bands: two holds hide it, and the more line says so', async ($, on) => {
  const w = world(on)
  w.others = [
    { id: 'holds', plugin: 'project', priority: 'hold', since: 1 },
    { id: 'ci', plugin: 'repo', priority: 'failure', since: 1 },
  ]
  await started($, w)
  expect(await text($)).toBeUndefined()
  const ui = await $.ui.mount({ ...BAND, surface: 'terminal' })
  expect(await ui.find({ type: 'Text', text: '+1 more band (/bands)' })).toBeDefined()
  await ui.unmount()
})

test('it draws when the bands above it leave room, and /bands lists it as info', async ($, on) => {
  const w = world(on)
  w.others = [{ id: 'holds', plugin: 'project', priority: 'hold', since: 1 }]
  await started($, w)
  expect(await text($)).toBeDefined()
  const listing = (await $.command.run({ command: 'bands', args: '' } as never)).text
  expect(listing).toContain('release (git, info): shown')
})
