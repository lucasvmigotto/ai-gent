import { expect, test } from 'claude-code/testing'

import { gated, parse } from './checkpoint'

const ROOT = '/repo'
const FILE = `${ROOT}/.ai-gent/checkpoint.md`
const WITH_HOLDS = `# Checkpoint — repo

Updated 2026-10-28T19:00-03:00 · claude session x

## Rules in force

- 2026-10-28 "never mention the client on the page"

## Holds

- main: hold until the 026 content is ready
- no tag before the docs land,
  even a patch
- none

## State
`
const NO_HOLDS = WITH_HOLDS.replace(/## Holds[\s\S]*## State/, '## Holds\n\n- none\n\n## State')

const BAND = {
  plugin: 'project',
  component: 'AbovePrompt',
  props: {
    hasSurvey: false,
    isWorking: false,
    maxRows: 10,
    bodyColumns: 100,
    scroll: { offset: 0, bodyRows: 10 },
    view: {},
  },
} as const

// Nothing sits beneath the plugin: these hooks stand for the engine, the
// repository at ROOT, a commit older than the file and the person's answer.
function engine(on: Parameters<Parameters<typeof test>[1]>[1], text: string | null, answer = 'Stop') {
  const asked: string[] = []
  on('session.start', (_, e) => ({ cwd: e.cwd }))
  on('session.cwd', () => ({ value: ROOT }))
  on('process.run', (_, e) =>
    e.argv.includes('rev-parse')
      ? { value: { exitCode: 0, stdout: `${ROOT}\n`, stderr: '' } }
      : { value: { exitCode: 0, stdout: '1000\n', stderr: '' } },
  )
  on('fs.stat', (_, e) =>
    e.path === FILE && text !== null
      ? { value: { kind: 'file', size: text.length, mtimeMs: 2_000_000, isLink: false } }
      : { deny: 'ENOENT' },
  )
  on('fs.read', () => ({ value: text ?? '' }))
  on('tool.call', (_, e) => {
    if (e.tool !== 'AskUserQuestion') return { result: { stdout: 'ran' } }
    const question = e.questions[0]?.question ?? ''
    asked.push(question)
    return { result: { questions: e.questions, answers: { [question]: answer } } }
  })
  return asked
}

test('parse reads holds and rules, joins continuations, drops "none"', () => {
  const read = parse(WITH_HOLDS)
  expect(read.holds).toEqual([
    'main: hold until the 026 content is ready',
    'no tag before the docs land, even a patch',
  ])
  expect(read.rules).toEqual(['2026-10-28 "never mention the client on the page"'])
  expect(read.updated).toBe('2026-10-28T19:00-03:00')
  expect(parse(NO_HOLDS).holds).toEqual([])
  expect(parse('no sections at all').holds).toEqual([])
})

test('gated names pushes, merges, tags and releases, and nothing read-only', () => {
  expect(gated('git push origin feat/x')).toBe('push')
  expect(gated('git status && git push')).toBe('push')
  expect(gated('GIT_SSH_COMMAND=ssh git -C ../c4t push')).toBe('push')
  expect(gated('git tag 1.4.0')).toBe('tag')
  expect(gated('git tag -a 1.4.0 -m 1.4.0')).toBe('tag')
  expect(gated('git merge --no-ff feat/x')).toBe('merge')
  expect(gated('gh pr merge 12 --rebase')).toBe('merge')
  expect(gated('gh release create 1.4.0')).toBe('release')
  for (const safe of ['git tag', 'git tag -l', 'git tag --contains HEAD', 'git merge --abort', 'git log --grep push', 'gh pr list', 'echo git push']) {
    expect(gated(safe)).toBe(null)
  }
})

test('the band lists the holds, on every surface', async ($, on) => {
  engine(on, WITH_HOLDS)
  await $.session.start({ cwd: ROOT, surface: 'terminal', isInteractive: true })

  for (const surface of ['terminal', 'desktop'] as const) {
    const ui = await $.ui.mount({ ...BAND, surface })
    expect((await ui.find({ type: 'Text', text: /hold until the 026/ }))?.text).toContain('main: hold')
    expect(await ui.find({ type: 'Text', text: /even a patch/ })).toBeDefined()
    await ui.unmount()
  }
})

for (const [name, text] of [['without holds', NO_HOLDS], ['without a checkpoint', null]] as const) {
  test(`the band stays quiet ${name}`, async ($, on) => {
    engine(on, text)
    on('ui.render', ($, e) => {
      const { Text } = $.ui.resolve(e)
      return <Text>engine</Text>
    })
    await $.session.start({ cwd: ROOT, surface: 'terminal', isInteractive: true })

    const ui = await $.ui.mount({ ...BAND, surface: 'terminal' })
    expect((await ui.find({ type: 'Text' }))?.text).toBe('engine')
    await ui.unmount()
  })
}

test('a push under a hold asks, and Stop refuses it', async ($, on) => {
  const asked = engine(on, WITH_HOLDS, 'Stop')
  await $.session.start({ cwd: ROOT, surface: 'terminal', isInteractive: true })

  const ran = await $.tool.call({ tool: 'Bash', command: 'git push origin dev' })
  expect(asked.length).toBe(1)
  expect(asked[0]).toContain('main: hold until the 026 content is ready')
  expect(ran.deny).toContain('holds in force')
})

test('a push under a hold runs once the person says so', async ($, on) => {
  engine(on, WITH_HOLDS, 'Run the push')
  await $.session.start({ cwd: ROOT, surface: 'terminal', isInteractive: true })

  const ran = await $.tool.call({ tool: 'Bash', command: 'git push origin dev' })
  expect(ran.deny).toBeUndefined()
})

test('without holds, or for a read-only command, nothing is asked', async ($, on) => {
  const asked = engine(on, NO_HOLDS)
  await $.session.start({ cwd: ROOT, surface: 'terminal', isInteractive: true })

  expect((await $.tool.call({ tool: 'Bash', command: 'git push' })).deny).toBeUndefined()
  expect((await $.tool.call({ tool: 'Bash', command: 'git status' })).deny).toBeUndefined()
  expect(asked.length).toBe(0)
})

test('the gate can be turned off', { options: { holdsGate: false } }, async ($, on) => {
  const asked = engine(on, WITH_HOLDS)
  await $.session.start({ cwd: ROOT, surface: 'terminal', isInteractive: true })

  expect((await $.tool.call({ tool: 'Bash', command: 'git push' })).deny).toBeUndefined()
  expect(asked.length).toBe(0)
})
