import { expect, mock, test } from 'claude-code/testing'

import { CARDS_LEDGER, CARDS_LEDGER_PLAIN, CARDS_MEASURE } from './cards-fixtures'
import { appendLog, axis, headerText, hhmm, overlapsOf, parseMeasure, peak, slugOf, transitions } from './cards-model'
import { text as cardsText, view } from './cards-view'
import { parseLedger } from './token-model'

type Handlers = Parameters<Parameters<typeof test>[1]>[1]

const ROOT = '/repo'
const LEDGER = '.ai-gent/orchestrate-status.md'
const PANE = {
  plugin: 'agents',
  component: 'Pane',
  requestId: 'cards',
  props: { title: 'Cards', isFocused: false, bodyColumns: 100, placement: 'dock', scroll: { offset: 0, bodyRows: 40 }, view: {} },
} as const

// ----------------------------------------------------------------- the model

test('the slug is the cwd with every character outside A-Za-z0-9- turned into a dash, as cmd_budget does', () => {
  expect(slugOf('/home/lucas/codes/ai-gent')).toBe('-home-lucas-codes-ai-gent')
  expect(slugOf('/home/lucas/my proj/a.b_c')).toBe('-home-lucas-my-proj-a-b-c')
})

test('the header counts the states, running first, and calls an unknown state other', () => {
  expect(headerText(parseLedger(CARDS_LEDGER))).toBe('5 cards · 2 running · 1 review · 1 queued · 1 landed')
  expect(headerText(parseLedger(CARDS_LEDGER_PLAIN))).toBe('2 cards · 1 running · 1 other')
  expect(headerText(parseLedger('| Card | State |\n|---|---|\n| `a` | landed |\n'))).toBe('1 card · 1 landed')
})

test('a first read invents no transition; later reads log each change with the cards that are new', () => {
  const first = transitions(parseLedger(CARDS_LEDGER), {})
  expect(first.changes).toEqual([])
  expect(first.seen['004-us4']).toBe('queued')
  const moved = parseLedger(CARDS_LEDGER.replace('| queued |', '| running |').replace('`004-us5` | landed', '`004-us5` | review'))
  expect(transitions(moved, first.seen).changes).toEqual([
    { id: '004-us4', from: 'queued', to: 'running' },
    { id: '004-us5', from: 'landed', to: 'review' },
  ])
  const added = parseLedger(`${CARDS_LEDGER}| \`004-us6\` | queued | b | w | — / — |\n`)
  expect(transitions(added, first.seen).changes).toEqual([{ id: '004-us6', from: '—', to: 'queued' }])
})

test('the log keeps the newest 200 entries', () => {
  let log: { id: string; from: string; to: string; at: string }[] = []
  for (let i = 0; i < 205; i += 1) log = appendLog(log, [{ id: `c${i}`, from: 'queued', to: 'running' }], '15:02')
  expect(log).toHaveLength(200)
  expect(log[0]?.id).toBe('c5')
  expect(log.at(-1)?.id).toBe('c204')
})

test('hhmm is a local HH:MM', () => {
  const t = Date.UTC(2026, 9, 31, 14, 5)
  expect(hhmm(t)).toMatch(/^\d\d:\d\d$/)
  expect(hhmm(t + 3_600_000).slice(3)).toBe(hhmm(t).slice(3))
})

const ids = new Set(parseLedger(CARDS_LEDGER).map(c => c.id))

test('parseMeasure merges a card\'s records into one span, with the latest record\'s figures and reads the Z as UTC', () => {
  const spans = parseMeasure(JSON.stringify(CARDS_MEASURE))
  expect(spans?.map(s => s.card)).toEqual(['004-us1-core', '004-us2', '004-us5', 'old-card'])
  const first = spans?.[0]
  expect(first?.startMs).toBe(Date.UTC(2026, 9, 31, 14, 0))
  expect(first?.endMs).toBe(Date.UTC(2026, 9, 31, 14, 45))
  expect(first?.used).toBe(20_000)
  expect(parseMeasure('not json')).toBeNull()
  expect(parseMeasure('{"cards": 3}')).toBeNull()
})

test('the peak and the overlaps come from the intervals alone', () => {
  const spans = (parseMeasure(JSON.stringify(CARDS_MEASURE)) ?? []).filter(s => ids.has(s.card))
  expect(peak(spans)).toBe(2)
  expect(overlapsOf(spans)).toEqual({ '004-us1-core': ['004-us2'], '004-us2': ['004-us1-core'], '004-us5': [] })
  const touching = [{ ...spans[0]!, startMs: 0, endMs: 10 }, { ...spans[0]!, card: 'x', startMs: 10, endMs: 20 }]
  expect(peak(touching)).toBe(1)
  expect(peak([])).toBe(0)
})

test('the axis scales to the columns and a short card keeps one cell', () => {
  const spans = (parseMeasure(JSON.stringify(CARDS_MEASURE)) ?? []).filter(s => ids.has(s.card))
  const a = axis(spans, 80)
  expect(a.startMs).toBe(Date.UTC(2026, 9, 31, 14, 0))
  expect(a.endMs).toBe(Date.UTC(2026, 9, 31, 15, 20))
  expect(a.bars).toEqual([
    { card: '004-us1-core', col: 0, width: 45 },
    { card: '004-us2', col: 10, width: 40 },
    { card: '004-us5', col: 60, width: 20 },
  ])
  const tiny = axis([{ ...spans[0]!, startMs: 0, endMs: 1 }, { ...spans[1]!, startMs: 0, endMs: 1_000_000 }], 10)
  expect(tiny.bars[0]).toEqual({ card: spans[0]!.card, col: 0, width: 1 })
  expect(axis([{ ...spans[0]!, startMs: 5, endMs: 5 }], 10).bars).toEqual([{ card: spans[0]!.card, col: 0, width: 1 }])
})

// ------------------------------------------------------------------ the view

const measured = (at = 1) => ({ status: 'done' as const, error: null, spans: parseMeasure(JSON.stringify(CARDS_MEASURE)) ?? [], at })
const snap = (extra = {}) => ({ cards: parseLedger(CARDS_LEDGER), log: [], since: '15:00', measure: null, ...extra })
const joined = (lines: { text: string }[]) => lines.map(l => l.text).join('\n')

test('wide rows carry state, branch, worktree and the Tokens cell as written; spans wait for Measure', () => {
  const out = joined(view(snap(), 100))
  expect(out).toContain('5 cards · 2 running · 1 review · 1 queued · 1 landed')
  expect(out).toMatch(/004-us1-core\s+running\s+feat\/us1\s+\.worktrees\/us1\s+~420k \/ 391k/)
  expect(out).toContain('press Measure')
  expect(out).toContain('since 15:00 (changes seen in this session)')
  expect(out).not.toContain('█')
})

test('a ledger with no Branch column has rows without one', () => {
  const out = joined(view(snap({ cards: parseLedger(CARDS_LEDGER_PLAIN) }), 100))
  expect(out).not.toContain('feat/')
  expect(out).toMatch(/^a\s+running\s+— \/ —$/m)
  expect(out).toMatch(/^b\s+weird/m)
})

test('measured, the header gives the peak and each card a bar, or "no run yet"', () => {
  const lines = view(snap({ measure: measured() }), 100)
  const out = joined(lines)
  expect(out).toContain('peak 2 in parallel')
  expect(out).toContain('█')
  expect(out).toMatch(/004-us4[^\n]*\n\s+no run yet/)
  expect(out).not.toContain('old-card')
  expect(out).toContain('script 20k') // the script's used beside the ledger's
  expect(lines.find(l => l.button)?.button?.id).toBe('measure')
})

test('a failed or running measurement says so and keeps the rows', () => {
  expect(joined(view(snap({ measure: { status: 'failed', error: 'timed out', spans: [], at: 1 } }), 100))).toContain('measure failed: timed out')
  expect(joined(view(snap({ measure: { status: 'running', error: null, spans: [], at: 1 } }), 100))).toContain('measuring')
})

test('under 60 columns the bars and the branch go and id, state and Tokens stay', () => {
  const out = joined(view(snap({ measure: measured() }), 50))
  expect(out).not.toContain('█')
  expect(out).not.toContain('feat/us1')
  expect(out).toMatch(/004-us1-core\s+running\s+~420k \/ 391k/)
})

test('the log lists the observed changes under the rows', () => {
  const out = joined(view(snap({ log: [{ id: '004-us2', from: 'queued', to: 'running', at: '15:02' }] }), 100))
  expect(out).toContain('15:02 004-us2 queued → running')
})

test('the text form is the header, a line per card, the log and the spans as a list', () => {
  const plain = cardsText(snap({ log: [{ id: '004-us2', from: 'queued', to: 'running', at: '15:02' }] }))
  expect(plain).toContain('5 cards · 2 running')
  expect(plain).toContain('004-us1-core')
  expect(plain).toContain('15:02 004-us2 queued → running')
  expect(plain).toContain('Spans: not measured')
  const m = cardsText(snap({ measure: measured() }))
  expect(m).toContain('peak 2 in parallel')
  expect(m).toMatch(/004-us1-core \d\d:\d\d–\d\d:\d\d/)
  expect(m).toContain('overlaps 004-us2')
  expect(cardsText(snap({ cards: [] }))).toContain('no cards')
})

// ----------------------------------------------------------------- the pane

type World = {
  files: Record<string, string>
  mtime: Record<string, number>
  runs: string[][]
  stats: number
  reads: number
  opened: number
  closed: string[]
  placed: boolean
  toasts: string[]
  registered: string[]
  measure: { exit: number; stdout: string; reject: boolean }
  clock: ReturnType<typeof mock.clock>
}

// Nothing sits beneath the plugin: these hooks stand for the engine, a
// repository at ROOT and token_report.py, edited by the test between steps.
function world(on: Handlers, files: Record<string, string>): World {
  const w: World = {
    files,
    mtime: {},
    runs: [],
    stats: 0,
    reads: 0,
    opened: 0,
    closed: [],
    placed: true,
    toasts: [],
    registered: [],
    measure: { exit: 0, stdout: JSON.stringify(CARDS_MEASURE), reject: false },
    clock: mock.clock(on, { now: 1_800_000_000_000 }),
  }
  const rel = (path: string) => path.replace(`${ROOT}/`, '')
  on('session.start', (_, e) => ({ cwd: e.cwd }))
  on('session.cwd', () => ({ value: ROOT }))
  on('turn.complete', (_, e) => ({ text: e.text }))
  on('command.register', (_, e) => {
    w.registered.push(e.name)
    return { value: { command: e.name } }
  })
  on('ui.open', () => {
    w.opened += 1
    return { value: { isPlaced: w.placed } }
  })
  on('ui.close', (_, e) => {
    w.closed.push(e.id)
    return undefined
  })
  on('ui.toast', (_, e) => {
    w.toasts.push(e.text)
    return { value: undefined }
  })
  on('ui.status', () => ({ value: undefined }))
  on('process.run', (_, e) => {
    w.runs.push([...e.argv])
    if (w.measure.reject) return { deny: 'timed out' }
    return { value: { exitCode: w.measure.exit, stdout: w.measure.stdout, stderr: '' } }
  })
  on('fs.stat', (_, e) => {
    w.stats += 1
    const path = rel(e.path)
    return path in w.files ? { value: { kind: 'file', size: w.files[path]?.length ?? 0, mtimeMs: w.mtime[path] ?? 1, isLink: false } } : { deny: 'ENOENT' }
  })
  on('fs.exists', (_, e) => ({ value: rel(e.path) in w.files }))
  on('fs.read', (_, e) => {
    w.reads += 1
    return rel(e.path) in w.files ? { value: w.files[rel(e.path)] ?? '' } : { deny: 'ENOENT' }
  })
  return w
}

const START = { cwd: ROOT, surface: 'terminal', isInteractive: true } as const
const open = (args = '') => ({ command: 'cards-pane', args }) as never
const edit = (w: World, text: string) => {
  w.files[LEDGER] = text
  w.mtime[LEDGER] = (w.mtime[LEDGER] ?? 1) + 1
}

test('/cards-pane opens the pane, which lists the rows and counts on terminal and desktop', async ($, on) => {
  const w = world(on, { [LEDGER]: CARDS_LEDGER })
  await $.session.start(START)
  expect(w.registered).toContain('cards-pane')
  expect(w.opened).toBe(0) // never by itself
  expect((await $.command.run(open())).text).toBe('Cards pane opened.')
  expect(w.opened).toBe(1)
  for (const surface of ['terminal', 'desktop'] as const) {
    const ui = await $.ui.mount({ ...PANE, surface })
    expect(await ui.find({ type: 'Text', text: /5 cards · 2 running · 1 review · 1 queued · 1 landed/ })).toBeDefined()
    expect(await ui.find({ type: 'Text', text: /004-us1-core\s+running\s+feat\/us1.*~420k \/ 391k/ })).toBeDefined()
    expect(await ui.find({ type: 'Text', text: /since \d\d:\d\d \(changes seen in this session\)/ })).toBeDefined()
    await ui.unmount()
  }
  expect(w.runs).toEqual([]) // opening never runs the script
})

test('with no ledger the command says so and opens nothing', async ($, on) => {
  const w = world(on, {})
  await $.session.start(START)
  const out = (await $.command.run(open())).text ?? ''
  expect(out).toContain('.ai-gent/orchestrate-status.md')
  expect(w.opened).toBe(0)
})

test('the pane follows a change within one poll, logs it, and an idle poll is one stat', { options: { tokenStatus: false } }, async ($, on) => {
  const w = world(on, { [LEDGER]: CARDS_LEDGER })
  await $.session.start(START)
  await $.command.run(open())
  const ui = await $.ui.mount({ ...PANE, surface: 'terminal' })
  expect(await ui.find({ type: 'Text', text: /changes seen/ })).toBeDefined()
  expect(await ui.find({ type: 'Text', text: /queued → running/ })).toBeUndefined()

  const reads = w.reads
  const stats = w.stats
  await w.clock.advance(5_000)
  expect(w.reads).toBe(reads) // nothing changed: no read
  expect(w.stats).toBe(stats + 1) // a poll is one stat
  expect(w.runs).toEqual([])

  edit(w, CARDS_LEDGER.replace('| queued | — | — |', '| running | feat/us4 | w4 |'))
  await w.clock.advance(5_000)
  expect(await ui.find({ type: 'Text', text: /\d\d:\d\d 004-us4 queued → running/ })).toBeDefined()
  expect(await ui.find({ type: 'Text', text: /1 review · 3 running|3 running · 1 review/ })).toBeDefined()
  await ui.unmount()
})

// The harness cannot raise `ui.close`, so a pane that cannot be placed stands for a closed one:
// the command then closes the pane's state and stops the poll.
test('a closed pane reads and polls nothing; reopening keeps the saved log without inventing the gap', { options: { tokenStatus: false } }, async ($, on) => {
  const w = world(on, { [LEDGER]: CARDS_LEDGER })
  await $.session.start(START)
  await $.command.run(open())
  edit(w, CARDS_LEDGER.replace('| queued | — | — |', '| running | feat/us4 | w4 |'))
  await w.clock.advance(5_000)
  w.placed = false
  await $.command.run(open())
  const stats = w.stats
  const reads = w.reads
  edit(w, CARDS_LEDGER.replace('| review |', '| landed |'))
  await w.clock.advance(60_000)
  expect([w.stats, w.reads]).toEqual([stats, reads])

  w.placed = true
  await $.command.run(open())
  const ui = await $.ui.mount({ ...PANE, surface: 'terminal' })
  expect(await ui.find({ type: 'Text', text: /004-us4 queued → running/ })).toBeDefined() // saved
  expect(await ui.find({ type: 'Text', text: /004-us3 review → landed/ })).toBeUndefined() // the gap is unknown
  await ui.unmount()
})

test('Measure runs the read-only script once with --project=<slug>, and draws the overlap', async ($, on) => {
  const w = world(on, { [LEDGER]: CARDS_LEDGER })
  await $.session.start(START)
  await $.command.run(open())
  const ui = await $.ui.mount({ ...PANE, surface: 'terminal' })
  expect(await ui.find({ type: 'Text', text: /press Measure/ })).toBeDefined()
  await ui.press({ key: 'button-measure' })
  expect(w.runs).toEqual([['python3', expect.stringMatching(/scripts\/token_report\.py$/), 'tasks', '--dry-run', '--json', '--project=-repo']])
  expect(await ui.find({ type: 'Text', text: /peak 2 in parallel/ })).toBeDefined()
  expect(await ui.find({ type: 'Text', text: /no run yet/ })).toBeDefined()
  await ui.unmount()
})

test('a failing or timed out script is reported and the rows and log stay', async ($, on) => {
  const w = world(on, { [LEDGER]: CARDS_LEDGER })
  await $.session.start(START)
  await $.command.run(open())
  const ui = await $.ui.mount({ ...PANE, surface: 'terminal' })
  w.measure.exit = 1
  w.measure.stdout = ''
  await ui.press({ key: 'button-measure' })
  expect(await ui.find({ type: 'Text', text: /measure failed/ })).toBeDefined()
  expect(await ui.find({ type: 'Text', text: /004-us1-core\s+running/ })).toBeDefined()
  w.measure.reject = true
  await ui.press({ key: 'button-measure' })
  expect(await ui.find({ type: 'Text', text: /measure failed/ })).toBeDefined()
  await ui.unmount()
})

test('/cards-pane --measure opens the pane and measures at once', async ($, on) => {
  const w = world(on, { [LEDGER]: CARDS_LEDGER })
  await $.session.start(START)
  await $.command.run(open('--measure'))
  await w.clock.settle()
  expect(w.runs).toHaveLength(1)
  const ui = await $.ui.mount({ ...PANE, surface: 'terminal' })
  expect(await ui.find({ type: 'Text', text: /peak 2 in parallel/ })).toBeDefined()
  await ui.unmount()
})

test('where no pane can open the command prints the text form, measured on request', async ($, on) => {
  const w = world(on, { [LEDGER]: CARDS_LEDGER })
  w.placed = false
  await $.session.start({ ...START, surface: 'mobile' })
  const plain = (await $.command.run(open())).text ?? ''
  expect(plain).toContain('5 cards · 2 running')
  expect(plain).toContain('Spans: not measured')
  expect(w.runs).toEqual([])
  const m = (await $.command.run(open('--measure'))).text ?? ''
  expect(m).toContain('peak 2 in parallel')
  expect(w.runs).toHaveLength(1)
  await w.clock.advance(60_000) // not left polling
  expect(w.runs).toHaveLength(1)
})

test('cardsPane off registers nothing', { options: { cardsPane: false } }, async ($, on) => {
  const w = world(on, { [LEDGER]: CARDS_LEDGER })
  await $.session.start(START)
  expect(w.registered).not.toContain('cards-pane')
  expect(w.registered).toContain('tokens') // the pane off leaves the token status
})

test('tokenStatus off does not turn the pane off', { options: { tokenStatus: false } }, async ($, on) => {
  const w = world(on, { [LEDGER]: CARDS_LEDGER })
  await $.session.start(START)
  expect(w.registered).toContain('cards-pane')
  expect(w.registered).not.toContain('tokens')
})

test('the token status still registers beside the pane', async ($, on) => {
  const w = world(on, {})
  await $.session.start(START)
  expect(w.registered).toEqual(expect.arrayContaining(['tokens', 'cards-pane']))
})
