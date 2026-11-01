import { expect, mock, test } from 'claude-code/testing'

type Handlers = Parameters<Parameters<typeof test>[1]>[1]

const ROOT = '/repo'
const CHECKPOINT = `${ROOT}/.ai-gent/checkpoint.md`
const SCRIPT = /\/agents\/scripts\/token_report\.py$/
const FILE_TIME = 2_000_000 // the checkpoint's mtimeMs
const START = { cwd: ROOT, surface: 'terminal', isInteractive: true } as const

const CAPS = (sessionLeft: number, weekLeft = 50) => ({
  verdict: 'proceed',
  caps: [
    { name: 'session_billable', cap: 4_000_000, used: 4_000_000 - (sessionLeft / 100) * 4_000_000, left: (sessionLeft / 100) * 4_000_000 },
    { name: 'week_billable', cap: 60_000_000, used: 0, left: (weekLeft / 100) * 60_000_000 },
  ],
  cards: [],
  batches: [],
})

type World = {
  percent: number | undefined
  usageFails: boolean
  usageCalls: number
  hasCheckpoint: boolean
  // a commit newer than the file, and how many came after it
  commits: number
  isDirty: boolean
  files: Set<string>
  budget: ReturnType<typeof CAPS>
  runs: string[][]
  toasts: string[]
  compacted: number
  clock: ReturnType<typeof mock.clock>
}

// Nothing sits beneath the plugin: these hooks stand for the engine, a repository
// at ROOT with a checkpoint, git, and token_report.py, whose answers a test edits.
function world(on: Handlers, init: Partial<World> = {}): World {
  const w: World = {
    percent: 82,
    usageFails: false,
    usageCalls: 0,
    hasCheckpoint: true,
    commits: 3,
    isDirty: false,
    files: new Set(['.ai-gent/budget.yaml', 'SCRIPT']),
    budget: CAPS(50),
    runs: [],
    toasts: [],
    compacted: 0,
    clock: mock.clock(on, { now: 1_800_000_000_000 }),
    ...init,
  }
  on('session.start', (_, e) => ({ cwd: e.cwd }))
  on('session.cwd', () => ({ value: ROOT }))
  on('turn.complete', (_, e) => ({ text: e.text }))
  on('session.compact', () => {
    w.compacted++
    return { messages: MESSAGES } as never
  })
  on('session.usage', () => {
    w.usageCalls++
    if (w.usageFails) return { deny: 'no usage' }
    return { value: { startedAt: 0, context: { window: 200_000, percent: w.percent }, rateLimits: [] } } as never
  })
  on('fs.stat', (_, e) =>
    e.path === CHECKPOINT && w.hasCheckpoint
      ? { value: { kind: 'file', size: 10, mtimeMs: FILE_TIME, isLink: false } }
      : { deny: 'ENOENT' },
  )
  on('fs.read', () => ({ value: '# Checkpoint\n\n## Holds\n\n- none\n' }))
  on('fs.exists', (_, e) => ({ value: SCRIPT.test(e.path) ? w.files.has('SCRIPT') : w.files.has(e.path.replace(`${ROOT}/`, '')) }))
  on('ui.toast', (_, e) => {
    w.toasts.push(e.text)
    return { value: undefined }
  })
  on('process.run', (_, e) => {
    const argv = [...e.argv]
    const out = (stdout: string) => ({ value: { exitCode: 0, stdout, stderr: '' } })
    if (argv[0] === 'python3') {
      w.runs.push(argv)
      return out(JSON.stringify(w.budget))
    }
    if (argv.includes('rev-parse')) return out(`${ROOT}\n`)
    // the last commit is newer than the file exactly when commits came after it
    if (argv.includes('log')) return out(w.commits > 0 ? '3000\n' : '1000\n')
    if (argv.includes('rev-list')) return out(`${w.commits}\n`)
    if (argv.includes('status')) return out(w.isDirty ? ' M plugins/x.ts\n' : '')
    return out('')
  })
  return w
}

const turn = ($: Parameters<Parameters<typeof test>[1]>[0]) => $.turn.complete({ text: 'done', reason: 'answer' } as never)
const MESSAGES = [{ role: 'user', text: 'hi', toolUses: [] }]
const COMPACT = (trigger: 'manual' | 'auto' | 'plugin' | 'precompute') => ({ trigger, messages: MESSAGES }) as never

// ------------------------------------------------------------------ story 1

for (const surface of ['terminal', 'desktop'] as const) {
  test(`story 1: a full context toasts once, with the commits since the checkpoint (${surface})`, async ($, on) => {
    const w = world(on)
    await $.session.start({ ...START, surface })
    await turn($)
    expect(w.toasts).toEqual([
      'Context 82% full. The checkpoint is 3 commits older than HEAD — run /project:checkpoint before the conversation is compacted.',
    ])
    await turn($)
    await turn($)
    expect(w.toasts).toHaveLength(1)
  })
}

test('story 1: no toast below the threshold, and none without a figure', async ($, on) => {
  const w = world(on, { percent: 79 })
  await $.session.start(START)
  await turn($)
  w.percent = undefined
  await turn($)
  expect(w.toasts).toEqual([])
  w.percent = 80
  await turn($)
  expect(w.toasts).toHaveLength(1)
})

test('story 1: no checkpoint says so', async ($, on) => {
  const w = world(on, { hasCheckpoint: false })
  await $.session.start(START)
  await turn($)
  expect(w.toasts).toEqual(['Context 82% full. There is no checkpoint — run /project:checkpoint before the conversation is compacted.'])
})

test('story 1: a current checkpoint and a clean tree get the informational toast, once', async ($, on) => {
  const w = world(on, { commits: 0 })
  await $.session.start(START)
  await turn($)
  await turn($)
  expect(w.toasts).toEqual(['Context 82% full. The checkpoint is current; nothing more is needed.'])
})

test('story 1: a current checkpoint under a dirty tree is not current', async ($, on) => {
  const w = world(on, { commits: 0, isDirty: true })
  await $.session.start(START)
  await turn($)
  expect(w.toasts).toEqual([
    'Context 82% full. The working tree has uncommitted changes since the checkpoint — run /project:checkpoint before the conversation is compacted.',
  ])
})

test('story 1: when $.session.usage() fails nothing shows and nothing is retried in the turn', { options: { contextMeter: false, usageMeter: false } }, async ($, on) => {
  const w = world(on, { usageFails: true })
  await $.session.start(START)
  await turn($)
  expect(w.toasts).toEqual([])
  expect(w.usageCalls).toBe(1)
})

// ------------------------------------------------------------------ story 2

test('story 2: a manual or auto compaction toasts once and is passed on unchanged', async ($, on) => {
  const w = world(on, { percent: 10 })
  await $.session.start(START)
  const result = await $.session.compact(COMPACT('auto'))
  expect(result).toEqual({ messages: MESSAGES })
  expect(w.compacted).toBe(1)
  expect(w.toasts).toEqual([
    'The conversation is about to be compacted. The checkpoint is 3 commits older than HEAD — run /project:checkpoint first: a summary loses what it keeps.',
  ])
  await $.session.compact(COMPACT('manual'))
  expect(w.toasts).toHaveLength(1)
  expect(w.compacted).toBe(2)
})

test('story 2: precompute and plugin compactions toast nothing, and still pass', async ($, on) => {
  const w = world(on, { percent: 10 })
  await $.session.start(START)
  await $.session.compact(COMPACT('precompute'))
  await $.session.compact(COMPACT('plugin'))
  expect(w.toasts).toEqual([])
  expect(w.compacted).toBe(2)
})

test('story 2: a context toast already shown means no second toast for the compaction', async ($, on) => {
  const w = world(on)
  await $.session.start(START)
  await turn($)
  await $.session.compact(COMPACT('auto'))
  expect(w.toasts).toHaveLength(1)
  expect(w.compacted).toBe(1)
})

// ------------------------------------------------------------------ story 3

test('story 3: a cap under the share left toasts once; the next turns run nothing more inside the throttle', async ($, on) => {
  const w = world(on, { percent: 10, budget: CAPS(7) })
  await $.session.start(START)
  expect(w.runs).toEqual([]) // never on session.start
  await turn($)
  await w.clock.settle()
  expect(w.toasts).toEqual(['session_billable cap nearly spent (280k of 4M left). The checkpoint is 3 commits older than HEAD — run /project:checkpoint.'])
  expect(w.runs).toEqual([['python3', expect.stringMatching(SCRIPT), 'budget', '--json', '--projected=0']])
  await turn($)
  await w.clock.settle()
  expect(w.runs).toHaveLength(1)
  // past the throttle it looks again, and a cap already toasted stays quiet
  await w.clock.advance(6 * 60_000)
  await turn($)
  await w.clock.settle()
  expect(w.runs).toHaveLength(2)
  expect(w.toasts).toHaveLength(1)
})

test('story 3: a cap that falls low later toasts then, and several caps are named tightest first', async ($, on) => {
  const w = world(on, { percent: 10, budget: CAPS(50) })
  await $.session.start(START)
  await turn($)
  await w.clock.settle()
  expect(w.toasts).toEqual([])
  w.budget = CAPS(8, 5)
  await w.clock.advance(6 * 60_000)
  await turn($)
  await w.clock.settle()
  expect(w.toasts.map(t => t.split(' ')[0])).toEqual(['week_billable', 'session_billable'])
})

test('story 3: no budget.yaml, or no token_report.py beside the plugin, runs nothing and shows nothing', async ($, on) => {
  const noBudget = world(on, { percent: 10, budget: CAPS(5), files: new Set(['SCRIPT']) })
  await $.session.start(START)
  await turn($)
  await noBudget.clock.settle()
  expect(noBudget.runs).toEqual([])
  expect(noBudget.toasts).toEqual([])
})

test('story 3: without the script the cap check is silent and the context nudge still works', async ($, on) => {
  const w = world(on, { budget: CAPS(5), files: new Set(['.ai-gent/budget.yaml']) })
  await $.session.start(START)
  await turn($)
  await w.clock.settle()
  expect(w.runs).toEqual([])
  expect(w.toasts).toHaveLength(1)
  expect(w.toasts[0]).toContain('Context 82% full')
})

test('a full context and a low cap each toast once, with no second context toast', async ($, on) => {
  const w = world(on, { percent: 85, budget: CAPS(5) })
  await $.session.start(START)
  await turn($)
  await w.clock.settle()
  await turn($)
  await w.clock.settle()
  expect(w.toasts.filter(t => t.startsWith('Context'))).toHaveLength(1)
  expect(w.toasts.filter(t => t.includes('cap nearly spent'))).toHaveLength(1)
})

// ------------------------------------------------------------------ story 4

test('story 4: handoffNudge off reads and spawns nothing', { options: { handoffNudge: false, contextMeter: false, usageMeter: false } }, async ($, on) => {
  const w = world(on, { budget: CAPS(5) })
  await $.session.start(START)
  await turn($)
  await $.session.compact(COMPACT('auto'))
  await w.clock.settle()
  expect(w.toasts).toEqual([])
  expect(w.usageCalls).toBe(0)
  expect(w.runs).toEqual([])
  expect(w.compacted).toBe(1)
})

test('story 4: handoffNudgeContextPercent 60 fires at 65%', { options: { handoffNudgeContextPercent: 60 } }, async ($, on) => {
  const w = world(on, { percent: 65 })
  await $.session.start(START)
  await turn($)
  expect(w.toasts).toHaveLength(1)
  expect(w.toasts[0]).toContain('Context 65% full')
})

test('story 4: handoffNudgeCapPercent 25 fires at 20% left', { options: { handoffNudgeCapPercent: 25 } }, async ($, on) => {
  const w = world(on, { percent: 10, budget: CAPS(20) })
  await $.session.start(START)
  await turn($)
  await w.clock.settle()
  expect(w.toasts).toHaveLength(1)
  expect(w.toasts[0]).toContain('session_billable cap nearly spent')
})
