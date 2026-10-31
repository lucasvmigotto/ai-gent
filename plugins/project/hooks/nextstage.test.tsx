import { expect, mock, test } from 'claude-code/testing'

import { HINT } from './nextstage-model'

type Handlers = Parameters<Parameters<typeof test>[1]>[1]
type Dollar = Parameters<Parameters<typeof test>[1]>[0]

const ROOT = '/repo'
const BRIEF = 'docs/product/brief.md'
const ARCH = 'docs/product/architecture.md'
const README = 'specs/README.md'
const START = { cwd: ROOT, surface: 'terminal', isInteractive: true } as const
const INDEX = (rows: string) => `# Features\n\n| # | Feature | Priority | Depends on | Frontend | Backend | QA |\n|---|---|---|---|---|---|---|\n${rows}\n`
const ROW_001 = '| 001 | foo | P1 | — | Planned | N/A | — |'
const NONE = 'nothing is ready to start; the full report covers QA, docs and CI'

type World = {
  files: Record<string, string>
  mtime: Record<string, number>
  runs: string[][]
  stats: number
  placed: boolean
  toasts: string[]
  // What the next Edit/Write does to the tree when the engine runs it.
  effect: (() => void) | null
  clock: ReturnType<typeof mock.clock>
  put: (path: string, text: string) => void
}

// The engine and a repository at ROOT, kept in a plain object a test edits.
function world(on: Handlers, files: Record<string, string>): World {
  let tick = 1
  const w: World = {
    files: {},
    mtime: {},
    runs: [],
    stats: 0,
    placed: true,
    toasts: [],
    effect: null,
    clock: mock.clock(on),
    put: (path, text) => {
      w.files[path] = text
      w.mtime[path] = ++tick
    },
  }
  for (const [path, text] of Object.entries(files)) w.put(path, text)
  const rel = (path: string) => path.replace(`${ROOT}/`, '')
  on('session.start', (_, e) => ({ cwd: e.cwd }))
  on('session.cwd', () => ({ value: ROOT }))
  on('turn.complete', (_, e) => ({ text: e.text }))
  on('command.register', () => ({ value: { command: 'pipeline-pane' } }))
  on('ui.open', () => ({ value: { isPlaced: w.placed } }))
  on('ui.close', () => undefined)
  on('ui.toast', (_, e) => {
    w.toasts.push(e.text)
    return { value: undefined }
  })
  on('process.run', (_, e) => {
    w.runs.push([...e.argv])
    return { value: { exitCode: 0, stdout: e.argv.includes('rev-parse') ? `${ROOT}\n` : '2026-10-01\n', stderr: '' } }
  })
  on('fs.stat', (_, e) => {
    const path = rel(e.path)
    if (!path.startsWith('.ai-gent/')) w.stats++
    return path in w.files ? { value: { kind: 'file', size: w.files[path]?.length ?? 0, mtimeMs: w.mtime[path] ?? 1, isLink: false } } : { deny: 'ENOENT' }
  })
  on('fs.exists', (_, e) => ({ value: rel(e.path) in w.files }))
  on('fs.read', (_, e) => (rel(e.path) in w.files ? { value: w.files[rel(e.path)] ?? '' } : { deny: 'ENOENT' }))
  on('fs.list', (_, e) => {
    const dir = rel(e.path).replace(/\/$/, '')
    const seen = new Map<string, 'file' | 'dir'>()
    for (const path of Object.keys(w.files)) {
      if (!path.startsWith(`${dir}/`)) continue
      const [head, ...rest] = path.slice(dir.length + 1).split('/')
      if (head) seen.set(head, rest.length > 0 ? 'dir' : 'file')
    }
    return { value: [...seen].map(([name, kind]) => ({ name, kind, size: 1, mtimeMs: w.mtime[`${dir}/${name}`] ?? 1, isLink: false })) }
  })
  on('tool.call', () => {
    w.effect?.()
    w.effect = null
    return { result: {} }
  })
  return w
}

const turn = ($: Dollar) => $.turn.complete({ text: 'done', reason: 'answer' } as never)
const write = (w: World, $: Dollar, path: string, text: string) => {
  w.effect = () => w.put(path, text)
  return $.tool.call({ tool: 'Write', file_path: `${ROOT}/${path}`, content: text } as never)
}
const edit = (w: World, $: Dollar, path: string, text: string) => {
  w.effect = () => w.put(path, text)
  return $.tool.call({ tool: 'Edit', file_path: `${ROOT}/${path}`, old_string: 'a', new_string: 'b' } as never)
}

for (const surface of ['terminal', 'desktop'] as const) {
  test(`story 1: a stage that writes the specs index toasts the next stage (${surface})`, async ($, on) => {
    const w = world(on, { [BRIEF]: '# Brief', [ARCH]: '# Arch' })
    await $.session.start({ ...START, surface })
    await write(w, $, README, INDEX(ROW_001))
    w.put('specs/001-foo/spec.md', '# spec')
    await turn($)
    expect(w.toasts).toEqual([`specs index written — next: /project:spec (001 foo needs plan) · ${HINT}`])
  })
}

test('story 1: writing only the brief truthfully says /project:status', async ($, on) => {
  const w = world(on, {})
  await $.session.start(START)
  expect(w.toasts).toEqual([])
  await write(w, $, BRIEF, '# Brief')
  await turn($)
  expect(w.toasts).toEqual([`brief written — next: /project:status (${NONE}) · ${HINT}`])
})

test('story 1: the toast quotes what the pane shows for the same files', async ($, on) => {
  const w = world(on, { [BRIEF]: '# Brief', [ARCH]: '# Arch' })
  await $.session.start(START)
  await write(w, $, README, INDEX(ROW_001))
  await turn($)
  w.placed = false
  const pane = (await $.command.run({ command: 'pipeline-pane', args: '' } as never)).text ?? ''
  expect(w.toasts).toHaveLength(1)
  expect(pane).toContain('Next (a hint): /project:spec')
  expect(w.toasts[0]).toContain('next: /project:spec (001 foo needs specify)')
})

test('story 1: the same recommendation is not told again, nor an edit that changes nothing', async ($, on) => {
  const w = world(on, {})
  await $.session.start(START)
  await write(w, $, BRIEF, '# Brief')
  await turn($)
  await edit(w, $, BRIEF, '# Brief, edited')
  await turn($)
  await write(w, $, 'docs/product/ux-vision.md', '# ux')
  await turn($)
  expect(w.toasts).toHaveLength(1)
})

test('story 1: an edit that adds a [NEEDS CLARIFICATION] toasts ask the user with file:line', async ($, on) => {
  const w = world(on, { [BRIEF]: '# Brief', [ARCH]: '# Arch', 'specs/001-foo/spec.md': '# spec\n' })
  await $.session.start(START)
  await edit(w, $, 'specs/001-foo/spec.md', '# spec\n[NEEDS CLARIFICATION: which login?]\n')
  await turn($)
  expect(w.toasts).toEqual([`ask the user (an open [NEEDS CLARIFICATION] blocks plan: which login?) — specs/001-foo/spec.md:2 · ${HINT}`])
})

test('story 1: nothing toasts at session start, and the first change is judged against the seed', async ($, on) => {
  const w = world(on, { [BRIEF]: '# Brief' })
  await $.session.start(START)
  await w.clock.advance(1_000)
  expect(w.toasts).toEqual([])
  await edit(w, $, BRIEF, '# Brief again')
  await turn($)
  expect(w.toasts).toEqual([])
})

test('story 1: a write by another tool is caught by the turn-end check', async ($, on) => {
  const w = world(on, {})
  await $.session.start(START)
  await w.clock.advance(10)
  w.put(BRIEF, '# Brief')
  await turn($)
  expect(w.toasts).toEqual([`brief written — next: /project:status (${NONE}) · ${HINT}`])
  await turn($)
  expect(w.toasts).toHaveLength(1)
})

test('story 2: ten writes in one turn give one toast, for the final recommendation', async ($, on) => {
  const w = world(on, { [BRIEF]: '# Brief', [ARCH]: '# Arch', [README]: INDEX(ROW_001) })
  await $.session.start(START)
  for (const name of ['spec', 'plan', 'tasks', 'research', 'quickstart', 'data-model', 'a', 'b', 'c', 'd']) {
    await write(w, $, `specs/001-foo/${name}.md`, `# ${name}`)
  }
  await turn($)
  expect(w.toasts).toHaveLength(1)
  expect(w.toasts[0]).toContain('001 foo: frontend is Planned')
})

test('story 2: a recommendation shown once is not repeated when it comes back', async ($, on) => {
  const w = world(on, {})
  await $.session.start(START)
  await write(w, $, BRIEF, '# Brief')
  await turn($)
  await write(w, $, 'specs/001-foo/spec.md', '# spec\n[NEEDS CLARIFICATION: q?]')
  await turn($)
  expect(w.toasts).toHaveLength(2)
  await edit(w, $, 'specs/001-foo/spec.md', '# spec\n')
  await turn($)
  expect(w.toasts).toHaveLength(2)
  await edit(w, $, 'specs/001-foo/spec.md', '# spec\n[NEEDS CLARIFICATION: q?]')
  await turn($)
  expect(w.toasts).toHaveLength(2)
})

test('story 2: a write outside the watched paths computes nothing', async ($, on) => {
  const w = world(on, {})
  await $.session.start(START)
  await w.clock.advance(0)
  const before = w.stats
  await write(w, $, 'src/app.ts', 'x')
  expect(w.toasts).toEqual([])
  expect(w.stats).toBe(before)
})

test('story 3: nextStageToast off computes and toasts nothing', { options: { nextStageToast: false, pipelinePane: false } }, async ($, on) => {
  const w = world(on, {})
  await $.session.start(START)
  await write(w, $, BRIEF, '# Brief')
  await turn($)
  expect(w.toasts).toEqual([])
  expect(w.stats).toBe(0)
})

test('story 3: the toast works with the pane off', { options: { pipelinePane: false } }, async ($, on) => {
  const w = world(on, {})
  await $.session.start(START)
  await write(w, $, BRIEF, '# Brief')
  await turn($)
  expect(w.toasts).toHaveLength(1)
})
