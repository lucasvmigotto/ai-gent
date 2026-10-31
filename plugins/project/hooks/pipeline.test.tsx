import { expect, mock, test } from 'claude-code/testing'

import { EXPECTED, README_TEXT } from './pipeline-fixtures'
import { featureState, letter, parseReadme, recommend, scanMarkers, split } from './pipeline-model'
import { text as snapshotText, view } from './pipeline-view'
import type { ArtifactState, Feature, Snapshot } from '../types'
import { TEXT, VIEW_100, VIEW_39 } from './pipeline-golden'
import { snapshotOf } from './pipeline-view-fixtures'

type Handlers = Parameters<Parameters<typeof test>[1]>[1]

const ROOT = '/repo'
const BRIEF = 'docs/product/brief.md'
const ARCH = 'docs/product/architecture.md'
const README = 'specs/README.md'
const PANE = {
  plugin: 'project',
  component: 'Pane',
  requestId: 'pipeline',
  props: { title: 'Pipeline', isFocused: false, bodyColumns: 100, placement: 'dock', scroll: { offset: 0, bodyRows: 40 }, view: {} },
} as const

// ----------------------------------------------------------------- the model

test('the model reads the README as specsync.py and wt.py do', () => {
  const rows = parseReadme(README_TEXT)
  expect(Object.fromEntries(rows.map(r => [r.number, featureState(r.sides, r.notes)]))).toEqual(EXPECTED.states)
  const plan = split(rows)
  expect(plan.ready).toEqual(EXPECTED.ready)
  expect(plan.blocked).toEqual(EXPECTED.blocked)
  expect(plan.done).toEqual(EXPECTED.done)
})

test('a README row keeps its priority, dependencies and the status word before a note', () => {
  const rows = parseReadme(README_TEXT)
  const four = rows.find(r => r.number === '004')
  expect(four?.depends).toEqual(['001'])
  expect(four?.priority).toBe('P1')
  expect(four?.sides.frontend).toBe('Implemented (tests untested)')
  expect(rows.find(r => r.number === '003')?.depends).toEqual(['001', '002'])
  expect(rows.some(r => r.number === 'abc')).toBe(false)
})

test('markers count in prose only, not in code or as the placeholder', () => {
  const text = [
    'A [NEEDS CLARIFICATION: which login?] here and [ASSUMPTION: one tenant] there.',
    'Marker syntax is `[NEEDS CLARIFICATION: …]` and `[UPSTREAM GAP: x]`.',
    '```',
    '[UPSTREAM GAP: in a fence]',
    '```',
    '[UPSTREAM GAP: …]',
    '[UPSTREAM GAP: brief lacks a glossary]',
    '[ASSUMPTION: the `Feature` column names it]',
  ].join('\n')
  const found = scanMarkers('specs/001/spec.md', text)
  expect(found.map(m => [m.kind, m.line])).toEqual([['clarification', 1], ['assumption', 1], ['gap', 7], ['assumption', 8]])
  expect(found[3]?.text).toBe('the `Feature` column names it')
  expect(found[0]?.text).toBe('which login?')
})

function artifact(path: string, patch: Partial<ArtifactState> = {}): ArtifactState {
  return { path, label: path, stage: `/stage-of-${path}`, isPresent: true, commitDate: '2026-10-01', staleSince: null, isProblem: false, ...patch }
}

function feature(number: string, patch: Partial<Feature> = {}): Feature {
  return { number, name: `f${number}`, priority: 'P1', depends: [], sides: { frontend: 'Planned' }, state: 'todo', bucket: 'ready', waiting: [], files: { spec: true, plan: true, tasks: true }, ...patch }
}

test('Next follows project:status rules 1 to 3', () => {
  const marker = { kind: 'clarification' as const, file: 'specs/004-x/spec.md', line: 12, text: 'which login?' }
  // 1: an open clarification first, with file:line
  expect(recommend([artifact(BRIEF)], [feature('002')], [marker])).toMatchObject({ rule: 1, ref: 'specs/004-x/spec.md:12' })
  // 2: the earliest missing artifact a later stage depends on, then a stale one
  expect(recommend([artifact(BRIEF), artifact(ARCH, { isPresent: false, isProblem: true })], [feature('002')], [])).toMatchObject({ rule: 2, stage: `/stage-of-${ARCH}` })
  expect(recommend([artifact(BRIEF), artifact(ARCH, { staleSince: { path: BRIEF, date: '2026-10-09' } })], [], [])).toMatchObject({ rule: 2, ref: ARCH })
  expect(recommend([artifact(BRIEF, { isPresent: false })], [], [])).toMatchObject({ rule: 2, stage: '/project:init' })
  // 3: the highest-priority ready feature, at its first step
  const features = [feature('003', { priority: 'P2' }), feature('002', { sides: { frontend: 'Implemented', backend: 'Planned' } })]
  expect(recommend([artifact(BRIEF)], features, [])).toMatchObject({ rule: 3, stage: '/backend:build', ref: '002' })
  expect(recommend([artifact(BRIEF)], [feature('002', { files: { spec: true, plan: false, tasks: false } })], [])).toMatchObject({ rule: 3, stage: '/project:spec' })
  // nothing ready: the full report
  expect(recommend([artifact(BRIEF)], [feature('002', { bucket: 'done' })], [])).toMatchObject({ rule: null, stage: '/project:status' })
})

const SNAPSHOT: Snapshot = {
  artifacts: [artifact(BRIEF, { label: 'brief' }), artifact(ARCH, { label: 'architecture', isPresent: false, isProblem: true })],
  features: [feature('001', { sides: { frontend: 'Implemented', backend: 'N/A' }, bucket: 'done' }), feature('002', { depends: ['001'], waiting: ['001'], bucket: 'blocked', sides: { frontend: 'Planned' } })],
  hasReadme: true,
  markers: { counts: { gap: 0, clarification: 2, assumption: 5 }, items: [{ kind: 'clarification', file: 'specs/004/spec.md', line: 3, text: 'q?' }], isTruncated: false },
  next: { rule: 3, stage: '/frontend:build', reason: '002 f002: frontend is Planned', ref: '002' },
  hasGit: true,
  computedAt: 0,
}

test('the view is a table when wide and letters when narrow, and ends in a hint', () => {
  const wide = view(SNAPSHOT, 100, {}).map(l => l.text)
  expect(wide).toContain('✓ brief  docs/product/brief.md')
  expect(wide.some(t => t.startsWith('✗ architecture: missing'))).toBe(true)
  expect(wide.some(t => t.includes('002 f002') && t.includes('waits for 001'))).toBe(true)
  expect(wide).toContain('2 clarifications · 5 assumptions · 0 gaps')
  expect(view({ ...SNAPSHOT, markers: { ...SNAPSHOT.markers, counts: { gap: 1, clarification: 1, assumption: 1 } } }, 100, {}).map(l => l.text)).toContain('1 clarification · 1 assumption · 1 gap')
  expect(wide.at(-1)).toContain('Next (a hint): /frontend:build')

  const narrow = view(SNAPSHOT, 30, {}).map(l => l.text)
  expect(narrow).toContain('✓ 001 f001 I/n')
  expect(narrow.some(t => t.startsWith('⏸ 002 f002 P'))).toBe(true)
  expect(letter('In progress')).toBe('W')
})

test('the markers expand on the button, and the headless text carries them', () => {
  expect(view(SNAPSHOT, 100, {}).some(l => l.text.includes('specs/004/spec.md:3'))).toBe(false)
  expect(view(SNAPSHOT, 100, { markers: true }).some(l => l.text.includes('specs/004/spec.md:3 q?'))).toBe(true)
  expect(snapshotText(SNAPSHOT)).toContain('specs/004/spec.md:3 q?')
  const empty = { ...SNAPSHOT, artifacts: SNAPSHOT.artifacts.map(a => ({ ...a, isPresent: false })), features: [], hasReadme: false }
  expect(view(empty, 100, {}).map(l => l.text)).toContain('No pipeline artifacts yet. Start with /project:init.')
})

test('013 compatibility goldens: the compact view and the headless text are unchanged', () => {
  const snapshot = snapshotOf()
  const lines = (columns: number) => view(snapshot, columns, {}).map(l => `${l.tone}|${l.text}|${l.button?.label ?? ''}`)
  expect(lines(100)).toEqual(VIEW_100)
  expect(lines(39)).toEqual(VIEW_39)
  expect(snapshotText(snapshot)).toBe(TEXT)
})

// ----------------------------------------------------------------- the pane

type World = {
  files: Record<string, string>
  mtime: Record<string, number>
  dates: Record<string, string>
  runs: string[][]
  lists: string[]
  opened: number
  placed: boolean
  filled: string[]
  clock: ReturnType<typeof mock.clock>
}

// Nothing sits beneath the plugin: these hooks stand for the engine and for a
// repository at ROOT, kept in a plain object a test edits between steps.
function world(on: Handlers, files: Record<string, string>): World {
  const w: World = { files, mtime: {}, dates: {}, runs: [], lists: [], opened: 0, placed: true, filled: [], clock: mock.clock(on) }
  const rel = (path: string) => path.replace(`${ROOT}/`, '')
  const mtimeOf = (path: string) => w.mtime[path] ?? 1
  on('session.start', (_, e) => ({ cwd: e.cwd }))
  on('session.cwd', () => ({ value: ROOT }))
  on('command.register', () => ({ value: { command: 'pipeline-pane' } }))
  on('ui.open', () => {
    w.opened += 1
    return { value: { isPlaced: w.placed } }
  })
  on('ui.close', () => undefined)
  on('prompt.fill', (_, e) => {
    w.filled.push(e.text)
    return { isFilled: true }
  })
  on('process.run', (_, e) => {
    w.runs.push([...e.argv])
    if (e.argv.includes('rev-parse')) return { value: { exitCode: 0, stdout: `${ROOT}\n`, stderr: '' } }
    return { value: { exitCode: 0, stdout: `${w.dates[e.argv.at(-1) ?? ''] ?? ''}\n`, stderr: '' } }
  })
  on('fs.stat', (_, e) => {
    const path = rel(e.path)
    return path in w.files ? { value: { kind: 'file', size: w.files[path]?.length ?? 0, mtimeMs: mtimeOf(path), isLink: false } } : { deny: 'ENOENT' }
  })
  on('fs.exists', (_, e) => ({ value: rel(e.path) in w.files }))
  on('fs.read', (_, e) => (rel(e.path) in w.files ? { value: w.files[rel(e.path)] ?? '' } : { deny: 'ENOENT' }))
  on('fs.list', (_, e) => {
    const dir = rel(e.path).replace(/\/$/, '')
    w.lists.push(dir)
    const seen = new Map<string, 'file' | 'dir'>()
    for (const path of Object.keys(w.files)) {
      if (!path.startsWith(`${dir}/`)) continue
      const [head, ...rest] = path.slice(dir.length + 1).split('/')
      if (head) seen.set(head, rest.length > 0 ? 'dir' : 'file')
    }
    return { value: [...seen].map(([name, kind]) => ({ name, kind, size: w.files[`${dir}/${name}`]?.length ?? 0, mtimeMs: mtimeOf(`${dir}/${name}`), isLink: false })) }
  })
  on('tool.call', () => ({ result: {} }))
  return w
}

const START = { cwd: ROOT, surface: 'terminal', isInteractive: true } as const
const PIPELINE: Record<string, string> = {
  [BRIEF]: '# Brief',
  [README]: README_TEXT,
  'specs/001-shell/spec.md': '# spec\n[NEEDS CLARIFICATION: which login?]',
}

test('the pane lists artifacts, features and open items on every surface', async ($, on) => {
  const w = world(on, { ...PIPELINE })
  w.dates[BRIEF] = '2026-10-01'
  await $.session.start(START)

  for (const surface of ['terminal', 'desktop'] as const) {
    const ui = await $.ui.mount({ ...PANE, surface })
    expect(await ui.find({ type: 'Text', text: /✓ brief/ })).toBeDefined()
    expect(await ui.find({ type: 'Text', text: /✗ architecture: missing/ })).toBeDefined()
    expect(await ui.find({ type: 'Text', text: /001 shell|002 search/ })).toBeDefined()
    expect(await ui.find({ type: 'Text', text: /1 clarification ·/ })).toBeDefined()
    expect(await ui.find({ type: 'Text', text: /Next \(a hint\): ask the user/ })).toBeDefined()
    await ui.unmount()
  }
})

test('an artifact older than its upstream is marked stale', async ($, on) => {
  const w = world(on, { ...PIPELINE, [ARCH]: '# Architecture' })
  w.dates[BRIEF] = '2026-10-10'
  w.dates[ARCH] = '2026-10-01'
  await $.session.start(START)
  const ui = await $.ui.mount({ ...PANE, surface: 'terminal' })
  expect(await ui.find({ type: 'Text', text: /~ architecture: older than docs\/product\/brief.md \(2026-10-10\)/ })).toBeDefined()
  await ui.unmount()
})

test('with no pipeline the pane says so and does not open unasked', async ($, on) => {
  const w = world(on, {})
  await $.session.start(START)
  expect(w.opened).toBe(0)
  const ui = await $.ui.mount({ ...PANE, surface: 'terminal' })
  expect(ui).toBeDefined()
  await ui.unmount()
})

test('the pane follows a change within one poll, and an idle pane spawns nothing', async ($, on) => {
  const w = world(on, { ...PIPELINE })
  const { clock } = w
  await $.session.start(START)
  const ui = await $.ui.mount({ ...PANE, surface: 'terminal' })
  expect(await ui.find({ type: 'Text', text: /✗ architecture: missing/ })).toBeDefined()

  const spawned = w.runs.length
  await clock.advance(600_000)
  expect(w.runs.length).toBe(spawned)

  w.files[ARCH] = '# Architecture'
  w.mtime[ARCH] = 99
  w.dates[ARCH] = '2026-10-02'
  await clock.advance(5_000)
  expect(await ui.find({ type: 'Text', text: /✓ architecture/ })).toBeDefined()
  await ui.unmount()
})

test('an Edit under specs/ shows at once, without waiting for the poll', async ($, on) => {
  const w = world(on, { ...PIPELINE })
  await $.session.start(START)
  w.files['specs/001-shell/spec.md'] = '# spec\nno marker now'
  w.mtime['specs/001-shell/spec.md'] = 50
  await $.tool.call({ tool: 'Edit', file_path: `${ROOT}/specs/001-shell/spec.md`, old_string: 'a', new_string: 'b' } as never)
  const ui = await $.ui.mount({ ...PANE, surface: 'terminal' })
  expect(await ui.find({ type: 'Text', text: /0 clarifications ·/ })).toBeDefined()
  await ui.unmount()
})

test('/pipeline opens the pane, or prints the snapshot where nothing can draw it', async ($, on) => {
  const w = world(on, { ...PIPELINE })
  await $.session.start({ ...START, surface: 'mobile' })
  expect(w.opened).toBe(0)

  expect((await $.command.run({ command: 'pipeline-pane', args: '' } as never)).text).toBe('Pipeline pane opened.')
  w.placed = false
  const text = (await $.command.run({ command: 'pipeline-pane', args: '' } as never)).text ?? ''
  expect(text).toContain('Pipeline')
  expect(text).toContain('✓ brief')
})

test('the /project:status button fills the prompt and never submits it', async ($, on) => {
  const w = world(on, { ...PIPELINE })
  await $.session.start(START)
  const ui = await $.ui.mount({ ...PANE, surface: 'terminal' })
  await ui.press({ key: 'button-status' })
  expect(w.filled).toEqual(['/project:status'])
  await ui.unmount()
})

test('the markers button expands the list', async ($, on) => {
  world(on, { ...PIPELINE })
  await $.session.start(START)
  const ui = await $.ui.mount({ ...PANE, surface: 'terminal' })
  expect(await ui.find({ type: 'Text', text: /specs\/001-shell\/spec.md:2/ })).toBeUndefined()
  await ui.press({ key: 'button-markers' })
  expect(await ui.find({ type: 'Text', text: /specs\/001-shell\/spec.md:2 which login\?/ })).toBeDefined()
  await ui.unmount()
})

test('pipelinePane off registers nothing', { options: { pipelinePane: false } }, async ($, on) => {
  const w = world(on, { ...PIPELINE })
  await $.session.start(START)
  expect(w.opened).toBe(0)
})
