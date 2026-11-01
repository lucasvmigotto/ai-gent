import { expect, mock, test } from 'claude-code/testing'

import { addToWave, errorPercent, landedNews, landingText, pctError, waveReady, waveSummary } from './landing-model'
import { PCT_FIXTURES, PCT_GAP_FIXTURES } from './pct-fixtures'
import { parseLedger } from './token-model'

type Handlers = Parameters<Parameters<typeof test>[1]>[1]

// ------------------------------------------------------- the percentage

test('the percentage equals pct_error over the shared fixture table, half to even', () => {
  for (const [p, u, want] of PCT_FIXTURES) expect(pctError(p, u), `${p}/${u}`).toBe(want)
  for (const [cp, cu, cwant] of PCT_GAP_FIXTURES) expect(pctError(cp, cu)).toBe(cwant)
  expect([pctError(200_000, 201_000), pctError(400_000, 406_000), pctError(400_000, 410_000)]).toEqual(['+0%', '+2%', '+2%'])
  expect(errorPercent(420_000, 391_000)).toBe(-7)
  expect(errorPercent(420_000, 550_000)).toBe(31)
})

test('no projection, no percentage', () => {
  expect([pctError(0, 5), errorPercent(0, 5)]).toEqual([null, null])
})

// -------------------------------------------------------- what is news

const cards = (rows: string): ReturnType<typeof parseLedger> => parseLedger(`| Card | State | Tokens |\n|---|---|---|\n${rows}\n`)
const row = (id: string, state: string, tokens: string) => `| \`${id}\` | ${state} | ${tokens} |`
const read = (rows: string[], seen = {}, toasted: string[] = [], first: string[] = [], hint = 25) => landedNews(cards(rows.join('\n')), seen, toasted, first, hint)

test('the first look records landed cards and never toasts them', () => {
  const out = read([row('a', 'landed', '~100k / 90k'), row('b', 'running', '~100k / —')])
  expect(out.news).toEqual([])
  expect(out.firstLook).toEqual(['a'])
  expect(out.seen).toEqual({ a: 'landed', b: 'running' })
  expect(out.toasted).toEqual([])
})

test('a transition into landed with both figures is news, once', () => {
  const first = read([row('a', 'review', '~420k / 391k')])
  const second = read([row('a', 'landed', '~420k / 391k')], first.seen, first.toasted, first.firstLook)
  expect(second.news).toMatchObject([{ id: 'a', projectedText: '~420k', usedText: '391k', percent: -7, hint: null }])
  const third = read([row('a', 'landed', '~420k / 391k')], second.seen, second.toasted, second.firstLook)
  expect(third.news).toEqual([])
})

test('a landed card with no used figure waits for it, even at the first look', () => {
  const first = read([row('a', 'landed', '~420k / —')])
  expect([first.news, first.firstLook]).toEqual([[], []])
  const later = read([row('a', 'landed', '~420k / 391k')], first.seen, first.toasted, first.firstLook)
  expect(later.news.map(n => n.id)).toEqual(['a'])
})

test('no projection or a zero projection gives a null percentage', () => {
  const seen = { z: 'running' }
  const out = read([row('a', 'landed', '— / 391k'), row('z', 'landed', '~0k / 391k')], seen)
  expect(out.news.map(n => [n.id, n.percent, n.hint])).toEqual([['a', null, null], ['z', null, null]])
})

test('a card that goes queued to landed between two reads is still news', () => {
  const first = read([row('a', 'queued', '—'), row('b', 'running', '—')])
  const out = read([row('a', 'landed', '~100k / 90k'), row('b', 'running', '—')], first.seen, first.toasted, first.firstLook)
  expect(out.news.map(n => n.id)).toEqual(['a'])
})

test('a fix round clears the card so its next landing toasts again, also for a first-look card', () => {
  const first = read([row('a', 'landed', '~100k / 90k'), row('b', 'running', '—')])
  const fix = read([row('a', 'running', '~100k / 90k'), row('b', 'running', '—')], first.seen, first.toasted, first.firstLook)
  expect(fix.firstLook).toEqual([])
  const again = read([row('a', 'landed', '~100k / 130k'), row('b', 'running', '—')], fix.seen, fix.toasted, fix.firstLook)
  expect(again.news.map(n => n.id)).toEqual(['a'])
  const fix2 = read([row('a', 'review', '~100k / 130k')], again.seen, again.toasted, again.firstLook)
  expect(fix2.toasted).toEqual([])
})

test('a row that vanishes before its figure appears is forgotten without a toast', () => {
  const first = read([row('a', 'running', '—')])
  const mid = read([row('a', 'landed', '~100k / —')], first.seen)
  const gone = read([row('b', 'running', '—')], mid.seen, mid.toasted, mid.firstLook)
  expect(gone.news).toEqual([])
  expect(gone.seen).toEqual({ b: 'running' })
})

test('several landings in one read are all news, in ledger order', () => {
  const seen = { a: 'running', b: 'running', c: 'running', d: 'running' }
  const out = read(['a', 'b', 'c', 'd'].map(id => row(id, 'landed', '~100k / 90k')), seen)
  expect(out.news.map(n => n.id)).toEqual(['a', 'b', 'c', 'd'])
  expect(out.toasted).toEqual(['a', 'b', 'c', 'd'])
})

// ------------------------------------------------------ the wording

test('the toast says used against projected, with a hint only for a big miss either way', () => {
  const seen = { a: 'x', b: 'x', c: 'x', d: 'x', e: 'x' }
  const out = read([
    row('a', 'landed', '~420k / 391k'),
    row('b', 'landed', '~420k / 550k'),
    row('c', 'landed', '~420k / 240k'),
    row('d', 'landed', '~420k / 462k'),
    row('e', 'landed', '— / 391k'),
  ], seen)
  expect(out.news.map(landingText)).toEqual([
    'a landed: used 391k vs ~420k projected (-7%)',
    'b landed: used 550k vs ~420k projected (+31%) — wrong tags or effort? see token-economy',
    'c landed: used 240k vs ~420k projected (-43%) — projection too high; budget shrank for nothing',
    'd landed: used 462k vs ~420k projected (+10%)',
    'e landed: used 391k (no projection)',
  ])
})

test('the hint threshold is the percentage shown, and 0 turns the hint off', () => {
  const seen = { a: 'x' }
  expect(read([row('a', 'landed', '~400k / 500k')], seen).news[0]?.hint).toBe('over') // exactly +25%
  expect(read([row('a', 'landed', '~400k / 499k')], seen).news[0]?.hint).toBe('over') // +24.75% shows +25%
  expect(read([row('a', 'landed', '~400k / 497k')], seen).news[0]?.hint).toBeNull() // +24.25% shows +24%
  expect(read([row('a', 'landed', '~400k / 500k')], seen, [], [], 0).news[0]?.hint).toBeNull()
  expect(read([row('a', 'landed', '~400k / 500k')], seen, [], [], 50).news[0]?.hint).toBeNull()
})

test('a wave counts cards over and under, never a token sum', () => {
  let wave = { landed: 0, over: 0, under: 0 }
  const seen = { a: 'x', b: 'x', c: 'x', d: 'x' }
  wave = addToWave(wave, read([row('a', 'landed', '~100k / 130k'), row('b', 'landed', '~100k / 50k'), row('c', 'landed', '~100k / 60k'), row('d', 'landed', '— / 9k')], seen).news)
  expect(wave).toEqual({ landed: 4, over: 1, under: 2 })
  expect(waveSummary(wave)).toBe('wave landed: 4 cards, 1 over, 2 under projection · /tokens for the budget')
  expect(waveSummary({ landed: 1, over: 0, under: 1 })).toBe('wave landed: 1 card, 0 over, 1 under projection · /tokens for the budget')
})

test('the wave is due when nothing is in flight, a card landed, and no landed card still waits for its figure', () => {
  const wave = { landed: 1, over: 0, under: 0 }
  expect(waveReady(cards(row('a', 'landed', '~1k / 1k')), wave)).toBe(true)
  expect(waveReady(cards([row('a', 'landed', '~1k / 1k'), row('b', 'review', '~1k / —')].join('\n')), wave)).toBe(false)
  expect(waveReady(cards([row('a', 'landed', '~1k / 1k'), row('b', 'landed', '~1k / —')].join('\n')), wave)).toBe(false)
  expect(waveReady(cards([row('a', 'landed', '~1k / 1k'), row('b', 'landed', '—')].join('\n')), wave)).toBe(true)
  expect(waveReady(cards(row('a', 'landed', '~1k / 1k')), { landed: 0, over: 0, under: 0 })).toBe(false)
  expect(waveReady([], wave)).toBe(false)
})

// ----------------------------------------------------------- the flow

const ROOT = '/repo'
const START = { cwd: ROOT, surface: 'terminal', isInteractive: true } as const
const LEDGER_PATH = '.ai-gent/orchestrate-status.md'

const table = (...rows: string[]) => `# Orchestration ledger\n\n| Card | Branch | State | Tokens |\n|---|---|---|---|\n${rows.map(r => r.replace(/^\| (`[^`]+`) \|/, '| $1 | feat/x |')).join('\n')}\n`

type World = { files: Record<string, string>; toasts: string[]; notices: string[]; runs: string[][]; reads: number; clock: ReturnType<typeof mock.clock> }

function world(on: Handlers, ledger: string): World {
  const w: World = { files: { [LEDGER_PATH]: ledger }, toasts: [], notices: [], runs: [], reads: 0, clock: mock.clock(on, { now: 1_800_000_000_000 }) }
  on('session.start', (_, e) => ({ cwd: e.cwd }))
  on('session.cwd', () => ({ value: ROOT }))
  on('turn.complete', (_, e) => ({ text: e.text }))
  on('command.register', (_, e) => ({ value: { command: e.name } }))
  on('fs.exists', (_, e) => ({ value: e.path.replace(`${ROOT}/`, '') in w.files }))
  on('fs.read', (_, e) => {
    if (e.path.endsWith(LEDGER_PATH)) w.reads += 1
    return { value: w.files[e.path.replace(`${ROOT}/`, '')] ?? '' }
  })
  on('ui.status', () => ({ value: undefined }))
  on('ui.toast', (_, e) => {
    w.toasts.push(e.text)
    return { value: undefined }
  })
  on('ui.notify', (_, e) => {
    w.notices.push(e.text)
    return { value: { isSent: true } } as never
  })
  on('process.run', (_, e) => {
    w.runs.push([...e.argv])
    return { value: { exitCode: 1, stdout: '', stderr: '' } }
  })
  on('tool.call', () => ({ result: {} }))
  return w
}

const look = async ($: Parameters<Parameters<typeof test>[1]>[0], w: World) => {
  await w.clock.advance(130_000) // past the refresh throttle
  await $.turn.complete({ text: 'done', reason: 'answer' } as never)
  await w.clock.settle()
}
const landing = (w: World) => w.toasts.filter(t => t.includes(' landed: used '))

const BEFORE = table('| `004-us1-core` | review | ~420k / 391k |')
const AFTER = table('| `004-us1-core` | landed | ~420k / 391k |')

test('a card that lands with both figures toasts once, with the percentage', async ($, on) => {
  const w = world(on, BEFORE)
  await $.session.start(START)
  await w.clock.settle()
  w.files[LEDGER_PATH] = AFTER
  await $.turn.complete({ text: 'done', reason: 'answer' } as never)
  await w.clock.advance(200_000)
  await $.turn.complete({ text: 'done', reason: 'answer' } as never)
  await w.clock.settle()
  expect(landing(w).filter(t => t.startsWith('004-us1-core'))).toEqual(['004-us1-core landed: used 391k vs ~420k projected (-7%)'])
  await look($, w)
  await look($, w)
  expect(landing(w).filter(t => t.startsWith('004-us1-core'))).toHaveLength(1)
  expect(w.notices).toEqual([]) // toast only
  expect(w.runs).toEqual([]) // nothing spawned: the ledger is a file
})

test('a card landed at the session start is not replayed', async ($, on) => {
  const w = world(on, AFTER)
  await $.session.start(START)
  await w.clock.settle()
  await look($, w)
  expect(landing(w)).toEqual([])
})

test('the figure that arrives after the landing toasts then', async ($, on) => {
  const w = world(on, table('| `a` | running | ~420k / — |', '| `b` | running | ~100k / — |'))
  await $.session.start(START)
  await w.clock.settle()
  w.files[LEDGER_PATH] = table('| `a` | landed | ~420k / — |', '| `b` | running | ~100k / — |')
  await look($, w)
  expect(landing(w)).toEqual([])
  w.files[LEDGER_PATH] = table('| `a` | landed | ~420k / 391k |', '| `b` | running | ~100k / — |')
  await look($, w)
  expect(landing(w)).toEqual(['a landed: used 391k vs ~420k projected (-7%)'])
})

test('no projection reads as such', async ($, on) => {
  const w = world(on, table('| `a` | review | — |', '| `b` | running | — |'))
  await $.session.start(START)
  await w.clock.settle()
  w.files[LEDGER_PATH] = table('| `a` | landed | — / 391k |', '| `b` | running | — |')
  await look($, w)
  expect(landing(w)).toEqual(['a landed: used 391k (no projection)'])
})

test('the wave toast comes after the last card lands, with counts only', async ($, on) => {
  const w = world(on, table('| `a` | running | ~100k / — |', '| `b` | running | ~100k / — |', '| `c` | running | ~100k / — |'))
  await $.session.start(START)
  await w.clock.settle()
  w.files[LEDGER_PATH] = table('| `a` | landed | ~100k / 140k |', '| `b` | running | ~100k / — |', '| `c` | running | ~100k / — |')
  await look($, w)
  expect(w.toasts.some(t => t.startsWith('wave'))).toBe(false)
  w.files[LEDGER_PATH] = table('| `a` | landed | ~100k / 140k |', '| `b` | landed | ~100k / 60k |', '| `c` | landed | ~100k / 70k |')
  await look($, w)
  expect(w.toasts.filter(t => t.startsWith('wave'))).toEqual(['wave landed: 3 cards, 1 over, 2 under projection · /tokens for the budget'])
  await look($, w)
  expect(w.toasts.filter(t => t.startsWith('wave'))).toHaveLength(1)
})

test('at most three card toasts per read, then one +N more line', async ($, on) => {
  const ids = ['a', 'b', 'c', 'd', 'e']
  const w = world(on, table(...ids.map(id => `| \`${id}\` | running | ~100k / — |`)))
  await $.session.start(START)
  await w.clock.settle()
  w.files[LEDGER_PATH] = table(...ids.map(id => `| \`${id}\` | landed | ~100k / 90k |`))
  await look($, w)
  expect(w.toasts.filter(t => t.endsWith('(-10%)'))).toHaveLength(3)
  expect(w.toasts).toContain('+2 more landed — /tokens')
})

test('the hint reaches the toast and cardCostHintPercent moves it', { options: { cardCostHintPercent: 50 } }, async ($, on) => {
  const w = world(on, table('| `a` | running | ~100k / — |', '| `b` | running | ~100k / — |'))
  await $.session.start(START)
  await w.clock.settle()
  w.files[LEDGER_PATH] = table('| `a` | landed | ~100k / 140k |', '| `b` | landed | ~100k / 30k |')
  await look($, w)
  expect(w.toasts).toContain('a landed: used 140k vs ~100k projected (+40%)')
  expect(w.toasts).toContain('b landed: used 30k vs ~100k projected (-70%) — projection too high; budget shrank for nothing')
})

test('a Bash call that ran token_report.py tasks reads the ledger at once, bypassing the throttle', async ($, on) => {
  const w = world(on, table('| `a` | landed | ~100k / — |', '| `b` | running | ~100k / — |'))
  await $.session.start(START)
  await w.clock.settle()
  w.files[LEDGER_PATH] = table('| `a` | landed | ~100k / 90k |', '| `b` | running | ~100k / — |')
  const before = w.reads
  await $.tool.call({ tool: 'Bash', command: 'ls' } as never)
  await w.clock.settle()
  expect(w.reads).toBe(before) // another command reads nothing
  await $.tool.call({ tool: 'Bash', command: 'python3 plugins/agents/scripts/token_report.py tasks --feature 004' } as never)
  await w.clock.settle()
  expect(landing(w)).toEqual(['a landed: used 90k vs ~100k projected (-10%)'])
  expect(w.runs).toEqual([])
})

test('cardCostToast off shows no landing toast', { options: { cardCostToast: false } }, async ($, on) => {
  const w = world(on, BEFORE)
  await $.session.start(START)
  await w.clock.settle()
  w.files[LEDGER_PATH] = AFTER
  await look($, w)
  await $.tool.call({ tool: 'Bash', command: 'token_report.py tasks' } as never)
  await w.clock.settle()
  expect(landing(w)).toEqual([])
  expect(w.toasts.some(t => t.startsWith('wave'))).toBe(false)
})

test('tokenStatus off turns the landing toast off too', { options: { tokenStatus: false } }, async ($, on) => {
  const w = world(on, BEFORE)
  await $.session.start(START)
  await w.clock.settle()
  w.files[LEDGER_PATH] = AFTER
  await $.tool.call({ tool: 'Bash', command: 'token_report.py tasks' } as never)
  await w.clock.settle()
  expect(w.toasts).toEqual([])
})

test('a resumed session does not replay the landings of the one before', async ($, on) => {
  const w = world(on, BEFORE)
  await $.session.start(START)
  await w.clock.settle()
  w.files[LEDGER_PATH] = AFTER
  await look($, w)
  expect(landing(w)).toHaveLength(1)
  await $.session.start(START)
  await w.clock.settle()
  expect(landing(w)).toHaveLength(1)
})
