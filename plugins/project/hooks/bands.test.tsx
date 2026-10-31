import { expect, test } from 'claude-code/testing'

import type { Band } from './band-priority'

type Handlers = Parameters<Parameters<typeof test>[1]>[1]

const ROOT = '/repo'
const FILE = `${ROOT}/.ai-gent/checkpoint.md`
const TEXT = '# Checkpoint\n\nUpdated 2026-10-28T19:00-03:00\n\n## Holds\n\n- main: hold until 026\n\n## State\n'
const START = { cwd: ROOT, surface: 'terminal', isInteractive: true } as const
const BAND = {
  plugin: 'project',
  component: 'AbovePrompt',
  props: { hasSurvey: false, isWorking: false, maxRows: 10, bodyColumns: 100, scroll: { offset: 0, bodyRows: 10 }, view: {} },
} as const

const GIT: Band = { id: 'hygiene', plugin: 'git', priority: 'normal', since: 1 }

function world(on: Handlers, others: Band[] | 'throws' | 'absent' = 'absent', hasFile = true) {
  const registered: string[] = []
  on('session.start', (_, e) => ({ cwd: e.cwd }))
  on('session.cwd', () => ({ value: ROOT }))
  on('command.register', (_, e) => {
    registered.push(e.name)
    return { value: { command: e.name } }
  })
  on('process.run', (_, e) =>
    e.argv.includes('rev-parse') ? { value: { exitCode: 0, stdout: `${ROOT}\n`, stderr: '' } } : { value: { exitCode: 0, stdout: '1000\n', stderr: '' } },
  )
  on('fs.stat', (_, e) => (e.path === FILE && hasFile ? { value: { kind: 'file', size: TEXT.length, mtimeMs: 2_000_000, isLink: false } } : { deny: 'ENOENT' }))
  on('fs.read', () => ({ value: TEXT }))
  on('state.get', async (_, e, next) => {
    if (e.plugin !== 'git') return next(e)
    if (others === 'throws') return { deny: 'no such plugin' }
    return { value: others === 'absent' ? { value: undefined, version: 0 } : { value: others, version: 1 } }
  })
  on('ui.render', ($, e) => {
    const { Text } = $.ui.resolve(e)
    return <Text>beneath</Text>
  })
  return { registered }
}

const holds = { type: 'Text', text: /hold until 026/ } as const

test('the holds band is a hold band: drawn at a cap of one beside another band', { options: { bandCap: 1 } }, async ($, on) => {
  world(on, [GIT])
  await $.session.start(START)
  const ui = await $.ui.mount({ ...BAND, surface: 'terminal' })
  expect(await ui.find(holds)).toBeDefined()
  // the hidden band's own plugin draws the "+N more" line, not this one
  expect(await ui.find({ type: 'Text', text: /more band/ })).toBeUndefined()
  await ui.unmount()
})

for (const others of ['absent', 'throws'] as const) {
  test(`with the git plugin ${others} the holds band draws as before`, async ($, on) => {
    world(on, others)
    await $.session.start(START)
    const ui = await $.ui.mount({ ...BAND, surface: 'terminal' })
    expect(await ui.find(holds)).toBeDefined()
    await ui.unmount()
  })
}

test('/bands lists the holds band and the other plugin\'s bands', async ($, on) => {
  const w = world(on, [GIT])
  await $.session.start(START)
  expect(w.registered).toContain('bands')
  const text = (await $.command.run({ command: 'bands', args: '' } as never)).text ?? ''
  expect(text).toContain('holds (project, hold): shown')
  expect(text).toContain('hygiene (git, normal): shown')
})

test('/bands says so when no band is active', async ($, on) => {
  const w = world(on, 'absent', false)
  await $.session.start(START)
  expect(w.registered).toContain('bands')
  expect((await $.command.run({ command: 'bands', args: '' } as never)).text).toBe('No active bands.')
})

test('/bands can be turned off', { options: { bandsCommand: false } }, async ($, on) => {
  const w = world(on)
  await $.session.start(START)
  expect(w.registered).not.toContain('bands')
})
