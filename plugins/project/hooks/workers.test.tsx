import { expect, mock, test } from 'claude-code/testing'

import { AGENTS, ALL_STATES, LOG, PROFILE_AND_CODEX, track } from './workers-fixtures'
import { segments } from './workers-draw'

type Handlers = Parameters<Parameters<typeof test>[1]>[1]

const ROOT = '/repo'
const START = { cwd: ROOT, surface: 'terminal', isInteractive: true } as const
const PANE = { plugin: 'project', component: 'Pane', requestId: 'workers', props: { bodyColumns: 120, bodyRows: 30, dock: 'right', view: {} } } as const
const PIPELINE_PANE = { plugin: 'project', component: 'Pane', requestId: 'pipeline', props: { bodyColumns: 100, bodyRows: 30, dock: 'right', view: {} } } as const

// A repository at ROOT as a plain object of files a test edits between steps.
function world(on: Handlers, files: Record<string, string>) {
  const w = { files, mtime: {} as Record<string, number>, opened: 0, closed: 0, placed: true, reads: [] as string[], agents: [] as unknown[] | null, clock: mock.clock(on) }
  const rel = (p: string) => p.replace(`${ROOT}/`, '')
  on('session.start', (_, e) => ({ cwd: e.cwd }))
  on('session.cwd', () => ({ value: ROOT }))
  on('command.register', (_, e) => ({ value: { command: e.name } }))
  on('ui.open', () => {
    w.opened += 1
    return { value: { isPlaced: w.placed } }
  })
  on('ui.close', () => undefined)
  on('process.run', (_, e) => ({ value: { exitCode: e.argv.includes('rev-parse') ? 0 : 1, stdout: `${ROOT}\n`, stderr: '' } }))
  on('fs.stat', (_, e) => {
    const p = rel(e.path)
    return p in w.files ? { value: { kind: 'file', size: w.files[p]?.length ?? 0, mtimeMs: w.mtime[p] ?? 1, isLink: false } } : { deny: 'ENOENT' }
  })
  on('fs.exists', (_, e) => ({ value: rel(e.path) in w.files }))
  on('fs.read', (_, e) => {
    w.reads.push(rel(e.path))
    return rel(e.path) in w.files ? { value: w.files[rel(e.path)] ?? '' } : { deny: 'ENOENT' }
  })
  on('fs.list', (_, e) => {
    const dir = rel(e.path).replace(/\/$/, '')
    const seen = new Map<string, 'file' | 'dir'>()
    for (const p of Object.keys(w.files)) {
      if (!p.startsWith(`${dir}/`)) continue
      const [head, ...rest] = p.slice(dir.length + 1).split('/')
      if (head) seen.set(head, rest.length > 0 ? 'dir' : 'file')
    }
    return { value: [...seen].map(([name, kind]) => ({ name, kind, size: 1, mtimeMs: 1, isLink: false })) }
  })
  on('agent.list', () => (w.agents === null ? { deny: 'nope' } : { value: w.agents as never }))
  on('tool.call', () => ({ result: {} }))
  return w
}

const statusFiles = (states: Record<string, string>): Record<string, string> =>
  Object.fromEntries(Object.entries(states).map(([name, text]) => [`.ai-gent/delegate/${name}/status.json`, text]))

// A parallel.yaml is part of the repository: without one the limit is unknown (decision 2), which has its own tests below.
const CAP3 = { '.ai-gent/parallel.yaml': 'max_parallel: 3\n' }
const FILES = { ...statusFiles(ALL_STATES), '.ai-gent/delegation.log': LOG, ...CAP3 }
const run = async ($: any, command = 'workers') => String((await $.command.run({ command, args: '' } as never)).text ?? '')

test('/workers lists every worker with its state, from the status files', async ($, on) => {
  world(on, FILES)
  await $.session.start(START)
  const out = await run($)
  expect(out).toContain('2 / 3 slots')
  for (const name of ['feat-core', 'feat-ui', 'feat-docs', 'feat-api', 'feat-db', 'feat-old']) expect(out).toContain(name)
  expect(out).toContain('Log: 1 run, 6m wall')
})

test('no status files: /workers says No workers', async ($, on) => {
  world(on, {})
  await $.session.start(START)
  expect(await run($)).toBe('No workers')
})

test('a malformed status file is counted, a file naming another track is not trusted', async ($, on) => {
  world(on, statusFiles({ 'feat-core': ALL_STATES['feat-core'] as string, broken: '{nope', 'feat-x': track('feat-y', 'running') }))
  await $.session.start(START)
  const out = await run($)
  expect(out).toContain('feat-core')
  expect(out).toContain('2 status files unreadable')
  expect(out).not.toContain('feat-y')
})

test('the cap comes from parallel.yaml', async ($, on) => {
  world(on, { ...FILES, '.ai-gent/parallel.yaml': 'max_parallel: 4\n' })
  await $.session.start(START)
  expect(await run($)).toContain('2 / 4 slots')
})

test('only status.json, the log and the cap line are read', async ($, on) => {
  const w = world(on, { ...FILES, '.ai-gent/delegate/feat-core/launch.json': 'SECRET', '.ai-gent/delegate/feat-core/last.txt': 'SECRET', '.ai-gent/delegate/feat-core/events.jsonl': 'SECRET' })
  await $.session.start(START)
  expect(await run($)).not.toContain('SECRET')
  expect(w.reads.filter(p => !p.endsWith('status.json') && p !== '.ai-gent/delegation.log' && p !== '.ai-gent/parallel.yaml')).toEqual([])
})

test('the pane draws on terminal and desktop, and a row opens on press', async ($, on) => {
  world(on, FILES)
  await $.session.start(START)
  await $.command.run({ command: 'workers-pane', args: '' } as never)
  for (const surface of ['terminal', 'desktop'] as const) {
    const ui = await $.ui.mount({ ...PANE, surface })
    expect(await ui.find({ type: 'Text', text: /2 \/ 3 slots/ })).toBeDefined()
    expect(await ui.find({ key: 'toggle:feat-core' })).toBeDefined()
    await ui.unmount()
  }
  const ui = await $.ui.mount({ ...PANE, surface: 'terminal' })
  await ui.press({ key: 'toggle:#history' })
  await ui.press({ key: 'toggle:feat-db' })
  expect(await ui.find({ type: 'Text', text: /Verdict: accepted/ })).toBeDefined()
  await ui.unmount()
})

// Spec 042: slots over the cap are [!] cells in the error tone, apart from the occupied ones.
const RUNNING = (n: number): Record<string, string> => statusFiles(Object.fromEntries(Array.from({ length: n }, (_, i) => [`w${i}`, track(`w${i}`, 'running')])))

const colorOf = async (ui: { find: (q: never) => Promise<unknown> }, text: RegExp) =>
  ((await ui.find({ type: 'Text', text } as never)) as { props?: { color?: string } } | undefined)?.props?.color

test('over the cap the pane draws the extra cells and the status in the error tone', async ($, on) => {
  world(on, { ...RUNNING(5), '.ai-gent/parallel.yaml': 'max_parallel: 3\n' })
  await $.session.start(START)
  await $.command.run({ command: 'workers-pane', args: '' } as never)
  const ui = await $.ui.mount({ ...PANE, surface: 'terminal' })
  expect(await colorOf(ui, /^\[!\] \[!\]$/)).toBe('error')
  expect(await colorOf(ui, /^2 over the limit$/)).toBe('error')
  expect(await colorOf(ui, /5 \/ 3 slots/)).toBe('error')
  expect(await colorOf(ui, /^\[■\] \[■\] \[■\] $/)).toBe('suggestion')
  await ui.unmount()
})

test('at or under the cap the pane draws no [!] cell and no error-toned capacity', async ($, on) => {
  world(on, { ...RUNNING(3), '.ai-gent/parallel.yaml': 'max_parallel: 3\n' })
  await $.session.start(START)
  await $.command.run({ command: 'workers-pane', args: '' } as never)
  const ui = await $.ui.mount({ ...PANE, surface: 'terminal' })
  expect(await ui.find({ type: 'Text', text: /\[!\]/ })).toBeUndefined()
  expect(await colorOf(ui, /3 \/ 3 slots/)).toBe('suggestion')
  await ui.unmount()
})

test('a line cut at its marks is the same text again', () => {
  const line = { key: 'k', text: '[■] [!] [!]   2 over the limit', tone: 'info' as const, marks: [{ from: 4, to: 11, tone: 'bad' as const }, { from: 14, to: 30, tone: 'bad' as const }] }
  const parts = segments(line)
  expect(parts.map(([t]) => t).join('')).toBe(line.text)
  expect(parts.map(([, tone]) => tone)).toEqual(['info', 'bad', 'info', 'bad'])
  expect(segments({ key: 'k', text: 'plain', tone: 'dim' })).toEqual([['plain', 'dim']])
})

test('the pane polls while open and redraws when a status file changes, and an idle poll reads nothing again', async ($, on) => {
  const w = world(on, statusFiles({ 'feat-core': ALL_STATES['feat-core'] as string }))
  await $.session.start(START)
  await $.command.run({ command: 'workers-pane', args: '' } as never)
  let ui = await $.ui.mount({ ...PANE, surface: 'terminal' })
  expect(await ui.find({ type: 'Text', text: /1 running/ })).toBeDefined()
  await ui.unmount()
  w.files['.ai-gent/delegate/feat-core/status.json'] = track('feat-core', 'done')
  w.mtime['.ai-gent/delegate/feat-core/status.json'] = 2
  await w.clock.advance(5_000)
  ui = await $.ui.mount({ ...PANE, surface: 'terminal' })
  expect(await ui.find({ type: 'Text', text: /1 done/ })).toBeDefined()
  await ui.unmount()
  const reads = w.reads.length
  await w.clock.advance(30_000)
  expect(w.reads.length).toBe(reads)
})

test('/workers-pane prints the list where nothing can dock; it never opens by itself', async ($, on) => {
  const w = world(on, FILES)
  await $.session.start(START)
  expect(w.opened).toBe(0)
  w.placed = false
  const out = await run($, 'workers-pane')
  expect(out).toContain('no pane here — /workers prints the list')
  expect(out).toContain('feat-core')
})

test('workersPane off: the pane says so and /workers still prints', { options: { workersPane: false } }, async ($, on) => {
  const w = world(on, FILES)
  await $.session.start(START)
  expect(await run($, 'workers-pane')).toContain('pane is off')
  expect(w.opened).toBe(0)
  expect(await run($)).toContain('feat-core')
})

const README = 'specs/README.md'
const BASE = { 'docs/product/brief.md': '# Brief', [README]: '# Specs\n' }

test('the pipeline pane gets one compact workers line, and no worker rows', { options: { pipelineLayout: 'compact' } }, async ($, on) => {
  const w = world(on, { ...BASE, ...FILES })
  await $.session.start(START)
  const ui = await $.ui.mount({ ...PIPELINE_PANE, surface: 'terminal' })
  expect(await ui.find({ type: 'Text', text: /^Workers: 1 running · 1 queued · 1 stalled ⚠ · 1 failed ✕ · 1 done · 1 taken-back · slots 2\/3$/ })).toBeDefined()
  expect(await ui.find({ type: 'Text', text: /feat-core/ })).toBeUndefined()
  await ui.unmount()
})

test('with no workers the pipeline pane gets no row', { options: { pipelineLayout: 'compact' } }, async ($, on) => {
  world(on, { ...BASE })
  await $.session.start(START)
  const ui = await $.ui.mount({ ...PIPELINE_PANE, surface: 'terminal' })
  expect(await ui.find({ type: 'Text', text: /Workers/ })).toBeUndefined()
  await ui.unmount()
})

test('workersLine off hides the compact line', { options: { pipelineLayout: 'compact', workersLine: false } }, async ($, on) => {
  const w = world(on, { ...BASE, ...FILES })
  await $.session.start(START)
  const ui = await $.ui.mount({ ...PIPELINE_PANE, surface: 'terminal' })
  expect(await ui.find({ type: 'Text', text: /Workers:/ })).toBeUndefined()
  await ui.unmount()
})

test('all three kinds are listed and totalled; only the two delegated tracks fill the meter, the subagent stands beside it', async ($, on) => {
  const w = world(on, { ...BASE, ...CAP3, ...statusFiles(PROFILE_AND_CODEX) })
  w.agents = AGENTS.slice(0, 2)
  await $.session.start(START)
  const out = await run($)
  expect(out).toContain('2 / 3 slots')
  expect(out).toContain('+1 subagent')
  for (const needle of ['prof', 'ext', 'sub', 'feat-prof', 'feat-cx', 'sub-e1']) expect(out).toContain(needle)
  expect(out).not.toContain('PROMPT-SECRET')
  expect(out).not.toContain('survey')
})

test('the compact pipeline line totals every kind but counts slots for delegated tracks only', { options: { pipelineLayout: 'compact' } }, async ($, on) => {
  const w = world(on, { ...BASE, ...CAP3, ...statusFiles(PROFILE_AND_CODEX) })
  w.agents = AGENTS.slice(0, 2)
  await $.session.start(START)
  const ui = await $.ui.mount({ ...PIPELINE_PANE, surface: 'terminal' })
  expect(await ui.find({ type: 'Text', text: /^Workers: 3 running · 1 queued · slots 2\/3 · \+1 subagent$/ })).toBeDefined()
  await ui.unmount()
})

test('subagents alone make a list; the description shows only with workersShowDescription', { options: { workersShowDescription: true } }, async ($, on) => {
  const w = world(on, {})
  w.agents = AGENTS.slice(0, 2)
  await $.session.start(START)
  const out = await run($)
  expect(out).toContain('survey')
  expect(out).not.toMatch(/[\u0000-\u0009\u000b-\u001f]/)
  expect(out).toContain('PROMPT-SECRET')
})

test('an agent list that fails leaves the status-file rows', async ($, on) => {
  const w = world(on, statusFiles({ 'feat-core': ALL_STATES['feat-core'] as string }))
  w.agents = null
  await $.session.start(START)
  expect(await run($)).toContain('feat-core')
})

test('an attention row opens that worker, and the open rows survive a refresh', async ($, on) => {
  const w = world(on, FILES)
  await $.session.start(START)
  await $.command.run({ command: 'workers-pane', args: '' } as never)
  let ui = await $.ui.mount({ ...PANE, surface: 'terminal' })
  expect(await ui.find({ type: 'Text', text: /ATTENTION · 2 issues/ })).toBeDefined()
  expect(await ui.find({ type: 'Text', text: /Verdict: rejected/ })).toBeUndefined()
  await ui.press({ key: 'attention:feat-api' })
  expect(await ui.find({ type: 'Text', text: /Verdict: rejected \(out-of-scope-path\)/ })).toBeDefined()
  await ui.press({ key: 'toggle:#history' })
  await ui.unmount()
  w.files['.ai-gent/delegate/feat-core/status.json'] = track('feat-core', 'running', { updated: 1_700_000_599 })
  w.mtime['.ai-gent/delegate/feat-core/status.json'] = 2
  await w.clock.advance(5_000)
  ui = await $.ui.mount({ ...PANE, surface: 'terminal' })
  expect(await ui.find({ type: 'Text', text: /Verdict: rejected \(out-of-scope-path\)/ })).toBeDefined()
  expect(await ui.find({ key: 'toggle:feat-db' })).toBeDefined()
  await ui.press({ key: 'toggle:feat-api' })
  expect(await ui.find({ type: 'Text', text: /Verdict: rejected/ })).toBeUndefined()
  await ui.unmount()
})

test('the pane in a narrow body fits and keeps every worker reachable', async ($, on) => {
  world(on, FILES)
  await $.session.start(START)
  await $.command.run({ command: 'workers-pane', args: '' } as never)
  const ui = await $.ui.mount({ ...PANE, props: { ...PANE.props, bodyColumns: 36 }, surface: 'terminal' })
  expect(await ui.find({ type: 'Text', text: /0\/3 slots|2\/3 slots/ })).toBeDefined()
  for (const t of ['feat-core', 'feat-docs', 'feat-api']) expect(await ui.find({ key: `toggle:${t}` })).toBeDefined()
  await ui.unmount()
})

// Decisions: the meter counts delegated tracks only, and an unreadable or invalid cap is unknown, never a guessed 3.
const CAP_LINE = (out: string) => out.split('\n').filter(l => /CAPACITY|slots/.test(l)).join('\n')

test('subagents beside two delegated tracks leave the meter at 2 / 3, with no mark', async ($, on) => {
  const w = world(on, { ...statusFiles(PROFILE_AND_CODEX), '.ai-gent/parallel.yaml': 'max_parallel: 3\n' })
  w.agents = [0, 1, 2].map(i => ({ id: `s${i}`, description: 'd', type: 'Explore', status: 'running' }))
  await $.session.start(START)
  const out = await run($)
  expect(out).toContain('2 / 3 slots')
  expect(out).toContain('+3 subagents')
  expect(out).not.toContain('[!]')
  expect(out).not.toContain('over the limit')
  await $.command.run({ command: 'workers-pane', args: '' } as never)
  const ui = await $.ui.mount({ ...PANE, surface: 'terminal' })
  expect(await ui.find({ type: 'Text', text: /\[!\]/ })).toBeUndefined()
  expect(await colorOf(ui, /2 \/ 3 slots/)).toBe('suggestion')
  await ui.unmount()
})

const UNKNOWN_CAPS: [string, Record<string, string>][] = [
  ['a missing parallel.yaml', {}],
  ['max_parallel: 0', { '.ai-gent/parallel.yaml': 'max_parallel: 0\n' }],
  ['a non-numeric value', { '.ai-gent/parallel.yaml': 'max_parallel: many\n' }],
  ['a value of 99+', { '.ai-gent/parallel.yaml': 'max_parallel: 100\n' }],
  ['an oversized file', { '.ai-gent/parallel.yaml': `max_parallel: 2\n${'#'.repeat(70_000)}` }],
]
for (const [name, extra] of UNKNOWN_CAPS) {
  test(`${name}: the pane says the limit is unknown, with no mark and no error tone`, async ($, on) => {
    world(on, { ...RUNNING(5), ...extra })
    await $.session.start(START)
    const out = await run($)
    expect(CAP_LINE(out)).toContain('5 in use (limit unknown)')
    expect(out).not.toContain('[!]')
    expect(out).not.toContain('over the limit')
    expect(out).not.toMatch(/\d+ \/ \d+ slots/)
    await $.command.run({ command: 'workers-pane', args: '' } as never)
    const ui = await $.ui.mount({ ...PANE, surface: 'terminal' })
    expect(await ui.find({ type: 'Text', text: /\[!\]/ })).toBeUndefined()
    expect(await colorOf(ui, /limit unknown/)).toBeUndefined()
    await ui.unmount()
  })
}

test('a readable parallel.yaml without the key uses the default cap', async ($, on) => {
  world(on, { ...RUNNING(2), '.ai-gent/parallel.yaml': 'other: 1\n' })
  await $.session.start(START)
  expect(await run($)).toContain('2 / 3 slots')
})
