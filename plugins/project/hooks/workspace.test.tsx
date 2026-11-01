import { expect, test } from 'claude-code/testing'

import type { Band } from './band-priority'

type Handlers = Parameters<Parameters<typeof test>[1]>[1]

const ROOT = '/repo'
const FILE = `${ROOT}/.ai-gent/checkpoint.md`
const HOLD_TEXT = '# Checkpoint\n\nUpdated 2026-10-28T19:00-03:00\n\n## Holds\n\n- main: hold until 026\n\n## State\n'
const START = { cwd: ROOT, surface: 'terminal', isInteractive: true } as const
const PROPS = { hasSurvey: false, isWorking: false, maxRows: 10, bodyColumns: 100, scroll: { offset: 0, bodyRows: 10 }, view: {} }
const mount = ($: Parameters<Parameters<typeof test>[1]>[0], props: Partial<typeof PROPS> = {}, surface: 'terminal' | 'desktop' = 'terminal') =>
  $.ui.mount({ plugin: 'project', component: 'AbovePrompt', surface, props: { ...PROPS, ...props } } as never)

const CI_OK: Band = { id: 'ci', plugin: 'repo', priority: 'normal', since: 1, summary: { text: 'CI passing', tone: 'ok' } }
const CI_FAIL: Band = { id: 'ci', plugin: 'repo', priority: 'failure', since: 1 }
const RELEASE: Band = { id: 'release', plugin: 'git', priority: 'info', since: 2, summary: { text: 'Release 2.23.0', tone: 'info' } }
const BEHIND: Band = { id: 'hygiene', plugin: 'git', priority: 'normal', since: 3, summary: { text: 'main behind origin', tone: 'warn' } }
const LEFT: Band = { id: 'containers', plugin: 'devcontainer', priority: 'info', since: 4, summary: { text: '3 container leftovers', tone: 'warn' } }
const DISK: Band = { id: 'containers', plugin: 'devcontainer', priority: 'normal', since: 4 }

// `held` maps a plugin to the bands its atom holds ('throws': a denied read).
function world(on: Handlers, held: Record<string, Band[] | 'throws'> = {}, hasHold = false) {
  on('session.start', (_, e) => ({ cwd: e.cwd }))
  on('session.cwd', () => ({ value: ROOT }))
  on('command.register', (_, e) => ({ value: { command: e.name } }))
  on('process.run', (_, e) => (e.argv.includes('rev-parse') ? { value: { exitCode: 0, stdout: `${ROOT}\n`, stderr: '' } } : { value: { exitCode: 0, stdout: '1000\n', stderr: '' } }))
  on('fs.stat', (_, e) => (e.path === FILE && hasHold ? { value: { kind: 'file', size: 10, mtimeMs: 2_000_000, isLink: false } } : { deny: 'ENOENT' }))
  on('fs.read', () => ({ value: HOLD_TEXT }))
  on('state.get', async (_, e, next) => {
    if (e.plugin === 'project') return next(e)
    const bands = held[e.plugin]
    if (bands === 'throws') return { deny: 'no such plugin' }
    return { value: { value: bands, version: bands === undefined ? 0 : 1 } }
  })
  on('ui.render', ($, e) => {
    const { Text } = $.ui.resolve(e)
    return <Text>beneath</Text>
  })
}

const text = (pattern: RegExp) => ({ type: 'Text', text: pattern }) as const

test('several plugins fold into one Workspace line and one warnings line', async ($, on) => {
  world(on, { repo: [CI_OK], git: [RELEASE, BEHIND], devcontainer: [LEFT] })
  await $.session.start(START)
  const ui = await mount($)
  expect(await ui.find(text(/^Workspace: ✓ CI passing · Release 2\.23\.0$/))).toBeDefined()
  expect(await ui.find(text(/^⚠ main behind origin · 3 container leftovers$/))).toBeDefined()
  expect(await ui.find({ type: 'Button', props: { label: '▸ Details' } } as never)).toBeDefined()
  await ui.unmount()
})

test('Details opens every folded band and closes again', async ($, on) => {
  world(on, { repo: [CI_OK], git: [RELEASE] })
  await $.session.start(START)
  const ui = await mount($)
  expect(await ui.find(text(/repo: CI passing/))).toBeUndefined()
  await ui.press({ key: 'workspace-details' })
  expect(await ui.find(text(/repo: CI passing/))).toBeDefined()
  expect(await ui.find(text(/git: Release 2\.23\.0/))).toBeDefined()
  expect(await ui.find({ type: 'Button', props: { label: '▾ Details' } } as never)).toBeDefined()
  await ui.press({ key: 'workspace-details' })
  expect(await ui.find(text(/repo: CI passing/))).toBeUndefined()
  await ui.unmount()
})

test('a failed CI, a low disk and a hold draw on their own, never behind the line', async ($, on) => {
  world(on, { repo: [CI_FAIL], devcontainer: [DISK], git: [RELEASE, BEHIND] }, true)
  await $.session.start(START)
  const ui = await mount($)
  // the line has the two git summaries only
  expect(await ui.find(text(/^Workspace: Release 2\.23\.0$/))).toBeDefined()
  expect(await ui.find(text(/hold until 026/))).toBeDefined()
  expect(await ui.find(text(/failed|CI/i))).toBeUndefined()
  // /bands: the failure and the disk are not "in the workspace line"
  const listing = (await $.command.run({ command: 'bands', args: '' } as never)).text ?? ''
  expect(listing).toContain('ci (repo, failure): shown')
  expect(listing).toContain('release (git, info): in the workspace line')
  expect(listing).toContain('hygiene (git, normal): in the workspace line')
  expect(listing).toContain('containers (devcontainer, normal): hidden')
  await ui.unmount()
})

test('a healthy CI with nothing else is one compact line', async ($, on) => {
  world(on, { repo: [CI_OK] })
  await $.session.start(START)
  const ui = await mount($)
  expect(await ui.find(text(/^Workspace: ✓ CI passing$/))).toBeDefined()
  await ui.unmount()
})

test('no bands: no workspace line', async ($, on) => {
  world(on)
  await $.session.start(START)
  const ui = await mount($)
  expect(await ui.find(text(/Workspace|Details/))).toBeUndefined()
  await ui.unmount()
})

test('a survey or a running turn hides the line; the band beneath stays', async ($, on) => {
  world(on, { repo: [CI_OK] })
  await $.session.start(START)
  for (const props of [{ hasSurvey: true }, { isWorking: true }]) {
    const ui = await mount($, props)
    expect(await ui.find(text(/Workspace/))).toBeUndefined()
    expect(await ui.find(text(/beneath/))).toBeDefined()
    await ui.unmount()
  }
})

test('workspaceLine off: every band is left to draw alone, and /bands says so', { options: { workspaceLine: false } }, async ($, on) => {
  world(on, { repo: [CI_OK] })
  await $.session.start(START)
  const ui = await mount($)
  expect(await ui.find(text(/Workspace/))).toBeUndefined()
  expect((await $.command.run({ command: 'bands', args: '' } as never)).text).toContain('ci (repo, normal): shown')
  await ui.unmount()
})

test('a plugin that throws adds nothing; the rest still fold', async ($, on) => {
  world(on, { repo: 'throws', git: [RELEASE] })
  await $.session.start(START)
  const ui = await mount($)
  expect(await ui.find(text(/^Workspace: Release 2\.23\.0$/))).toBeDefined()
  await ui.unmount()
})

for (const [columns, label] of [[100, 'wide'], [60, 'medium'], [30, 'narrow']] as const) {
  test(`${label}: the lines fit ${columns} columns`, async ($, on) => {
    world(on, { repo: [CI_OK], git: [RELEASE, BEHIND], devcontainer: [LEFT] })
    await $.session.start(START)
    const ui = await mount($, { bodyColumns: columns })
    const rows = (await ui.findAll({ type: 'Text' } as never)) as { text?: string }[]
    for (const row of rows) {
      const t = row.text ?? ''
      if (t !== 'beneath' && t.trim() !== '') expect([...t].length + [...t].filter(c => c === '⚠').length).toBeLessThanOrEqual(Math.max(columns, 12))
    }
    await ui.unmount()
  })
}

test('desktop draws the same line', async ($, on) => {
  world(on, { repo: [CI_OK] })
  await $.session.start({ ...START, surface: 'desktop' })
  const ui = await mount($, {}, 'desktop')
  expect(await ui.find(text(/^Workspace: ✓ CI passing$/))).toBeDefined()
  await ui.unmount()
})

test('headless: no render, and the leader is declared only where a session started', async ($, on) => {
  world(on, { repo: [CI_OK] })
  await $.session.start({ cwd: ROOT, surface: null, isInteractive: false } as never)
  expect((await $.command.run({ command: 'bands', args: '' } as never)).text).toContain('ci (repo, normal)')
})

test('chromeAscii draws the line with ASCII only', { options: { chromeAscii: true } }, async ($, on) => {
  world(on, { repo: [CI_OK], git: [BEHIND] })
  await $.session.start(START)
  const ui = await mount($)
  expect(await ui.find(text(/^Workspace: ok CI passing$/))).toBeDefined()
  expect(await ui.find(text(/^! main behind origin$/))).toBeDefined()
  expect(await ui.find({ type: 'Button', props: { label: '> Details' } } as never)).toBeDefined()
  await ui.unmount()
})
