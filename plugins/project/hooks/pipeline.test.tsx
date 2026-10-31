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

// 013's pane tests run on its line list, which is the `compact` layout.
const COMPACT = { options: { pipelineLayout: 'compact' } } as const
const START = { cwd: ROOT, surface: 'terminal', isInteractive: true } as const
const PIPELINE: Record<string, string> = {
  [BRIEF]: '# Brief',
  [README]: README_TEXT,
  'specs/001-shell/spec.md': '# spec\n[NEEDS CLARIFICATION: which login?]',
}

test('the pane lists artifacts, features and open items on every surface', COMPACT, async ($, on) => {
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

test('an artifact older than its upstream is marked stale', COMPACT, async ($, on) => {
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

test('the pane follows a change within one poll, and an idle pane spawns nothing', COMPACT, async ($, on) => {
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

test('an Edit under specs/ shows at once, without waiting for the poll', COMPACT, async ($, on) => {
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

test('the /project:status button fills the prompt and never submits it', COMPACT, async ($, on) => {
  const w = world(on, { ...PIPELINE })
  await $.session.start(START)
  const ui = await $.ui.mount({ ...PANE, surface: 'terminal' })
  await ui.press({ key: 'button-status' })
  expect(w.filled).toEqual(['/project:status'])
  await ui.unmount()
})

test('the markers button expands the list', COMPACT, async ($, on) => {
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

// ------------------------------------------------------- the dashboard (029)

type Node = { type?: string; key?: string; props?: Record<string, unknown>; children?: unknown[] }

// Every string a drawn tree shows, in order, and every key it carries.
function walk(node: unknown, out: { texts: string[]; keys: string[]; types: string[] } = { texts: [], keys: [], types: [] }) {
  if (typeof node === 'string') out.texts.push(node)
  else if (node && typeof node === 'object') {
    const n = node as Node
    if (n.type) out.types.push(n.type)
    if (typeof n.props?.key === 'string') out.keys.push(n.props.key)
    if (typeof n.key === 'string') out.keys.push(n.key)
    for (const child of n.children ?? []) walk(child, out)
  }
  return out
}

test('the dashboard is the default: every region is there on every surface, in native components only', async ($, on) => {
  const w = world(on, { ...PIPELINE })
  w.dates[BRIEF] = '2026-10-01'
  await $.session.start(START)
  for (const surface of ['terminal', 'desktop', 'vscode'] as const) {
    const ui = await $.ui.mount({ ...PANE, surface })
    for (const key of ['header', 'summary', 'artifacts', 'progress', 'features', 'items', 'next', 'artifact:docs/product/brief.md', 'feature:002', 'marker:clarification']) {
      expect(await ui.find({ key })).toBeDefined()
    }
    expect(await ui.find({ type: 'Text', text: /PIPELINE/ })).toBeDefined()
    const { types, texts } = walk(await ui.drawn())
    expect(types.every(t => ['Box', 'Text', 'Button'].includes(t))).toBe(true)
    expect(texts.join('').includes('\u001b')).toBe(false)
    await ui.unmount()
  }
})

test('the header says LIVE while the poll checks, and the caption advances with each tick', async ($, on) => {
  const w = world(on, { ...PIPELINE })
  // The mock clock starts at 0, which the pane reads as never checked.
  await w.clock.advance(1_700_000_000_000)
  await $.session.start(START)
  const ui = await $.ui.mount({ ...PANE, surface: 'terminal' })
  expect(await ui.find({ type: 'Text', text: / LIVE / })).toBeDefined()
  const first = (await ui.find({ type: 'Text', text: /^Checked / }))?.text
  expect(first).toMatch(/^Checked \d\d:\d\d:\d\d · every 5 s$/)
  await w.clock.advance(65_000)
  const later = (await ui.find({ type: 'Text', text: /^Checked / }))?.text
  expect(later).not.toBe(first)
  expect(await ui.find({ type: 'Text', text: / STALE / })).toBeUndefined()
  await ui.unmount()
})

test('the dashboard shows the features as a matrix and an idle redraw reads nothing', async ($, on) => {
  const w = world(on, { ...PIPELINE })
  await $.session.start(START)
  const ui = await $.ui.mount({ ...PANE, surface: 'terminal' })
  expect((await ui.find({ key: 'feature:002' }))?.text).toMatch(/002.*search.*P1.*ready.*In progress.*Planned/)
  expect((await ui.find({ key: 'feature:003' }))?.text).toMatch(/003.*guides.*blocked/)
  const spawned = w.runs.length
  const listed = w.lists.length
  await ui.press({ key: 'button-markers' })
  expect(await ui.find({ type: 'Text', text: /specs\/001-shell\/spec.md:2 which login\?/ })).toBeDefined()
  expect(w.runs.length).toBe(spawned)
  expect(w.lists.length).toBe(listed)
  await ui.unmount()
})

test('the Next action block fills /project:status and says it is only a hint', async ($, on) => {
  const w = world(on, { ...PIPELINE })
  await $.session.start(START)
  const ui = await $.ui.mount({ ...PANE, surface: 'desktop' })
  expect(await ui.find({ type: 'Text', text: /Hint only · Full report:/ })).toBeDefined()
  expect(await ui.find({ type: 'Text', text: /Needs attention/ })).toBeDefined()
  await ui.press({ key: 'button-status' })
  expect(w.filled).toEqual(['/project:status'])
  await ui.unmount()
})

test('the width picks the mode: compact text under 40, condensed to 90, full above', async ($, on) => {
  world(on, { ...PIPELINE })
  await $.session.start(START)
  const at = async (bodyColumns: number) => {
    const ui = await $.ui.mount({ ...PANE, props: { ...PANE.props, bodyColumns }, surface: 'terminal' })
    const found = { dashboard: (await ui.find({ key: 'header' })) !== undefined, large: (await ui.find({ type: 'Text', text: /^Artifacts ready$/ })) !== undefined, line: (await ui.find({ type: 'Text', text: / ready · / })) !== undefined, compact: (await ui.find({ type: 'Text', text: /^Pipeline$/ })) !== undefined }
    await ui.unmount()
    return found
  }
  expect(await at(39)).toEqual({ dashboard: false, large: false, line: false, compact: true })
  expect(await at(40)).toEqual({ dashboard: true, large: false, line: true, compact: false })
  expect(await at(90)).toEqual({ dashboard: true, large: false, line: true, compact: false })
  expect(await at(91)).toEqual({ dashboard: true, large: true, line: false, compact: false })
})

test('the ASCII option leaves no non-ASCII glyph in what the pane draws', { options: { pipelineAscii: true } }, async ($, on) => {
  world(on, { ...PIPELINE })
  await $.session.start(START)
  const ui = await $.ui.mount({ ...PANE, surface: 'terminal' })
  const text = walk(await ui.drawn()).texts.join('\n')
  expect(text).toContain('PIPELINE')
  // The sentence of the next action comes from 013's recommend; its own text may hold non-ASCII only if a file does.
  expect(/^[\x20-\x7e\n]*$/.test(text)).toBe(true)
  await ui.unmount()
})

test('the row cap and the layout option reach the pane', { options: { pipelineMaxRows: 3 } }, async ($, on) => {
  world(on, { ...PIPELINE })
  await $.session.start(START)
  const ui = await $.ui.mount({ ...PANE, surface: 'terminal' })
  expect(await ui.find({ key: 'feature:003' })).toBeDefined()
  expect(await ui.find({ key: 'feature:004' })).toBeUndefined()
  expect(await ui.find({ type: 'Text', text: /\+6 more; filter to see them/ })).toBeDefined()
  await ui.unmount()
})

test('any layout value but compact reads as the dashboard', { options: { pipelineLayout: 'nonsense' } }, async ($, on) => {
  world(on, { ...PIPELINE })
  await $.session.start(START)
  const ui = await $.ui.mount({ ...PANE, surface: 'terminal' })
  expect(await ui.find({ key: 'header' })).toBeDefined()
  await ui.unmount()
})

test('headless /pipeline still answers 013s text', async ($, on) => {
  const w = world(on, { ...PIPELINE })
  w.placed = false
  await $.session.start({ ...START, isInteractive: false })
  const out = (await $.command.run({ command: 'pipeline-pane', args: '' } as never)).text ?? ''
  expect(out).toMatch(/^Pipeline\nArtifacts\n✓ brief/)
})

// ------------------------------------------- the refinement: interactions

test('the filter buttons write the atom, mark the active one and keep the legend', async ($, on) => {
  const w = world(on, { ...PIPELINE })
  await $.session.start(START)
  const ui = await $.ui.mount({ ...PANE, props: { ...PANE.props, bodyColumns: 60 }, surface: 'terminal' })
  expect(await ui.find({ key: 'feature:001' })).toBeDefined()
  const spawned = w.runs.length
  await ui.press({ key: 'filter:blocked' })
  expect(await ui.find({ key: 'feature:001' })).toBeUndefined()
  expect(await ui.find({ key: 'feature:003' })).toBeDefined()
  expect(await ui.find({ type: 'Text', text: /○ Planned/ })).toBeDefined()
  await ui.press({ key: 'filter:all' })
  expect(await ui.find({ key: 'feature:001' })).toBeDefined()
  expect(w.runs.length).toBe(spawned)
  await ui.unmount()
})

test('pressing a feature or an artifact opens its details, and again closes them', async ($, on) => {
  const w = world(on, { ...PIPELINE })
  w.dates[BRIEF] = '2026-10-01'
  await $.session.start(START)
  const ui = await $.ui.mount({ ...PANE, props: { ...PANE.props, bodyColumns: 44 }, surface: 'terminal' })
  expect(await ui.find({ type: 'Text', text: /Depends on/ })).toBeUndefined()
  await ui.press({ key: 'toggle:feature:003' })
  expect(await ui.find({ type: 'Text', text: /Depends on/ })).toBeDefined()
  expect(await ui.find({ type: 'Text', text: /002 search \(waits for\)/ })).toBeDefined()
  await ui.press({ key: 'toggle:feature:003' })
  expect(await ui.find({ type: 'Text', text: /Depends on/ })).toBeUndefined()
  await ui.press({ key: 'toggle:artifact:docs/product/brief.md' })
  expect(await ui.find({ type: 'Text', text: /^docs\/product\/brief.md$/ })).toBeDefined()
  await ui.unmount()
})

test('each kind of open item opens on its own button', async ($, on) => {
  world(on, { ...PIPELINE })
  await $.session.start(START)
  const ui = await $.ui.mount({ ...PANE, surface: 'terminal' })
  await ui.press({ key: 'toggle:marker:clarification' })
  expect(await ui.find({ type: 'Text', text: /which login\?/ })).toBeDefined()
  await ui.unmount()
})

test('the next action names the recommendation and fills its command, and without one offers only the report', async ($, on) => {
  const w = world(on, { [BRIEF]: '# Brief', [README]: README_TEXT })
  await $.session.start(START)
  const ui = await $.ui.mount({ ...PANE, surface: 'terminal' })
  expect(await ui.find({ type: 'Text', text: /Suggested/ })).toBeDefined()
  expect(await ui.find({ type: 'Text', text: /◆ Architecture/ })).toBeDefined()
  expect(await ui.find({ type: 'Text', text: /architecture is missing although a later stage already ran/ })).toBeDefined()
  await ui.press({ key: 'button-next' })
  expect(w.filled).toEqual(['/project:architecture'])
  await ui.press({ key: 'button-status' })
  expect(w.filled).toEqual(['/project:architecture', '/project:status'])
  await ui.unmount()
})

test('a recommendation that is not a command has no run button', async ($, on) => {
  world(on, { ...PIPELINE })
  await $.session.start(START)
  const ui = await $.ui.mount({ ...PANE, surface: 'terminal' })
  expect(await ui.find({ type: 'Text', text: /◆ Ask the user/ })).toBeDefined()
  expect(await ui.find({ key: 'button-next' })).toBeUndefined()
  await ui.unmount()
})
