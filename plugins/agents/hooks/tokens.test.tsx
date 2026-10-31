import { expect, mock, test } from 'claude-code/testing'

import { etaNews, fmtEta, fmtRate, fmtTok, inFlight, news, parseBudget, parseLedger, parseTok, statusText } from './token-model'
import type { Budget } from './token-model'

type Handlers = Parameters<Parameters<typeof test>[1]>[1]

// ----------------------------------------------------------------- the model

const LEDGER = `# Orchestration ledger

| Card | Branch | State | Tokens |
|---|---|---|---|
| \`004-us1-core\` | feat/us1 | running | ~420k / 391k |
| \`004-us2\` | feat/us2 | review | ~300k / — |
| \`004-us3\` | feat/us3 | landed | ~100k / 90k |
| \`004-us4\` | feat/us4 | queued | — |
| not a card | x | running | ~1k / 1k |

Pending merges: none.
`

test('the ledger is read by its Card, State and Tokens columns', () => {
  const cards = parseLedger(LEDGER)
  expect(cards.map(c => [c.id, c.state, c.projectedText, c.usedText])).toEqual([
    ['004-us1-core', 'running', '~420k', '391k'],
    ['004-us2', 'review', '~300k', '—'],
    ['004-us3', 'landed', '~100k', '90k'],
    ['004-us4', 'queued', '—', '—'],
  ])
  expect(cards[0]).toMatchObject({ projected: 420_000, used: 391_000 })
  expect(cards[1]).toMatchObject({ projected: 300_000, used: null })
  expect(inFlight(cards).map(c => c.id)).toEqual(['004-us1-core', '004-us2', '004-us4'])
  // a table with no Card column is not the ledger
  expect(parseLedger('| Name | State |\n|---|---|\n| `x` | running |\n')).toEqual([])
})

test('figures read and print the way token_report.py does', () => {
  expect([parseTok('~420k'), parseTok('391k'), parseTok('1.24M'), parseTok('—'), parseTok('')]).toEqual([420_000, 391_000, 1_240_000, null, null])
  expect([fmtTok(391_000), fmtTok(1_240_000), fmtTok(3_100_000)]).toEqual(['391k', '1.24M', '3.10M'])
})

const BUDGET: Budget = {
  verdict: 'proceed',
  why: 'within the 2M left',
  fit: 3,
  caps: [
    { name: 'session', cap: 12_000_000, used: 8_900_000, left: 3_100_000 },
    { name: 'week', cap: 60_000_000, used: 38_000_000, left: 22_000_000 },
  ],
  cards: [],
  batches: [],
}

test('the status line is the cards, then what the caps leave, and gives way card by card', () => {
  const cards = inFlight(parseLedger(LEDGER))
  expect(statusText(cards, BUDGET, 200)).toBe('tokens · 004-us1-core ~420k/391k · 004-us2 ~300k/— · 004-us4 —/— · session left 3.10M · week left 22.00M')
  expect(statusText(cards, { ...BUDGET, verdict: 'shrink' }, 200)).toContain('⚠ shrink')
  const narrow = statusText(cards, { ...BUDGET, verdict: 'stop' }, 70) ?? ''
  expect(narrow).toContain('+2 more')
  expect(narrow).toContain('session left 3.10M')
  expect(narrow).toContain('⚠ stop')
  expect(statusText([], BUDGET, 200)).toBe('tokens · session left 3.10M · week left 22.00M')
  expect(statusText(cards, null, 200)).toBe('tokens · 004-us1-core ~420k/391k · 004-us2 ~300k/— · 004-us4 —/—')
  expect(statusText([], null, 200)).toBeNull()
})

test('only news is toasted: a verdict that turned bad, a card that passed its projection, each once', () => {
  const cards = parseLedger('| Card | State | Tokens |\n|---|---|---|\n| `a` | running | ~420k / 520k |\n| `b` | running | ~300k / — |\n| `c` | running | ~300k / 200k |\n')
  const first = news(cards, { ...BUDGET, verdict: 'shrink', why: 'only a fits' }, 'proceed', [])
  expect(first.verdict).toBe('budget: shrink — only a fits')
  expect(first.overruns).toEqual([{ id: 'a', text: 'a used 520k, projected ~420k (+24%)' }])
  const again = news(cards, { ...BUDGET, verdict: 'shrink', why: 'only a fits' }, 'shrink', ['a'])
  expect(again).toEqual({ verdict: null, overruns: [] })
  expect(news(cards, BUDGET, 'shrink', ['a']).verdict).toBeNull() // back to proceed is not news
  expect(parseBudget('not json')).toBeNull()
  expect(parseBudget('{"verdict":"stop","caps":[]}')?.verdict).toBe('stop')
})

// ------------------------------------------------- the burn rate (spec 018)

const WITH_ETA: Budget = {
  ...BUDGET,
  rate: { per_hour: 7_400_000, window_minutes: 30, messages: 112 },
  caps: [
    { name: 'session', cap: 12_000_000, used: 8_900_000, left: 3_100_000, eta_minutes: 130 },
    { name: 'week', cap: 60_000_000, used: 38_000_000, left: 22_000_000, eta_minutes: 900 },
  ],
}

test('an ETA reads ~25m, ~2h10m, ~15h, and >2d past 48 hours; a rate reads 7.40M/h', () => {
  expect([fmtEta(25), fmtEta(130), fmtEta(900), fmtEta(60), fmtEta(2880), fmtEta(2881)]).toEqual(['~25m', '~2h10m', '~15h', '~1h', '~48h', '>2d'])
  expect([fmtRate(7_400_000), fmtRate(400_000)]).toEqual(['7.40M/h', '400k/h'])
})

test('parseLedger reads the optional Branch and Worktree columns, and only when they exist', () => {
  const rich = parseLedger('| Card | State | Branch | Worktree | Tokens |\n|---|---|---|---|---|\n| `a` | running | feat/a | `.worktrees/a` | ~1k / — |\n| `b` | queued | — |  | — / — |\n')
  expect(rich[0]).toMatchObject({ id: 'a', branch: 'feat/a', worktree: '.worktrees/a', projectedText: '~1k' })
  expect(rich[1]?.branch).toBeUndefined()
  expect(rich[1]?.worktree).toBeUndefined()
  const plain = parseLedger('| Card | State | Tokens |\n|---|---|---|\n| `a` | running | ~1k / — |\n')
  expect('branch' in (plain[0] ?? {})).toBe(false)
})

test('the status line puts each cap\'s ETA after its figure, and only when the script gave one', () => {
  expect(statusText([], WITH_ETA, 200)).toBe('tokens · session left 3.10M (~2h10m) · week left 22.00M (~15h)')
  const partial = { ...WITH_ETA, caps: [{ ...WITH_ETA.caps[0]!, eta_minutes: null }, WITH_ETA.caps[1]!] }
  expect(statusText([], partial, 200)).toBe('tokens · session left 3.10M · week left 22.00M (~15h)')
  const long = { ...WITH_ETA, caps: [{ ...WITH_ETA.caps[0]!, eta_minutes: 5000 }] }
  expect(statusText([], long, 200)).toBe('tokens · session left 3.10M (>2d)')
  expect(statusText([], WITH_ETA, 200, false)).toBe('tokens · session left 3.10M · week left 22.00M') // tokenBurnRate off: 014's line
})

const withEta = (session: number | null, week: number | null = null, left = 3_100_000): Budget => ({
  ...WITH_ETA,
  caps: [
    { name: 'session', cap: 12_000_000, used: 0, left, eta_minutes: session },
    { name: 'week', cap: 60_000_000, used: 0, left: 22_000_000, eta_minutes: week },
  ],
})

test('an ETA toast fires once per crossing of the threshold, per cap', () => {
  let warned: string[] = []
  const step = (b: Budget | null, threshold = 30) => {
    const out = etaNews(b, warned, threshold)
    warned = out.warned
    return out.toasts
  }
  expect(step(withEta(60))).toEqual([])
  expect(step(withEta(25))).toEqual(['session cap lasts ~25m at 7.40M/h — pause the wave or finish the card in flight'])
  expect(step(withEta(25))).toEqual([]) // still under: no second toast
  expect(step(withEta(90))).toEqual([]) // recovered
  expect(step(withEta(20))).toHaveLength(1) // a new crossing
  expect(step(withEta(null))).toEqual([]) // null clears the flag
  expect(warned).toEqual([])
  expect(step(withEta(20, 10))).toHaveLength(2) // two caps, two toasts
  expect(step(withEta(20, 10))).toEqual([])
})

test('no ETA toast for a cap with nothing left, with the threshold at 0, or without a budget', () => {
  expect(etaNews(withEta(0, null, 0), [], 30).toasts).toEqual([])
  expect(etaNews(withEta(5), [], 0)).toEqual({ toasts: [], warned: [] })
  expect(etaNews(null, ['session'], 30)).toEqual({ toasts: [], warned: [] })
})

// ----------------------------------------------------------------- the flow

const ROOT = '/repo'
const START = { cwd: ROOT, surface: 'terminal', isInteractive: true } as const

type World = {
  files: Record<string, string>
  runs: string[][]
  budget: { exit: number; json: Budget; text: string }
  cardsExit: number
  statuses: (string | undefined)[]
  toasts: string[]
  notices: string[]
  registered: string[]
  clock: ReturnType<typeof mock.clock>
}

// Nothing sits beneath the plugin: these hooks stand for the engine, a repository
// at ROOT and token_report.py, whose answers a test edits between steps.
function world(on: Handlers, files: Record<string, string>): World {
  const w: World = {
    files,
    runs: [],
    budget: { exit: 0, json: BUDGET, text: 'wave: 1 card(s)\nverdict: proceed - within the 2M left' },
    cardsExit: 0,
    statuses: [],
    toasts: [],
    notices: [],
    registered: [],
    clock: mock.clock(on, { now: 1_800_000_000_000 }),
  }
  on('session.start', (_, e) => ({ cwd: e.cwd }))
  on('session.cwd', () => ({ value: ROOT }))
  on('turn.complete', (_, e) => ({ text: e.text }))
  on('command.register', (_, e) => {
    w.registered.push(e.name)
    return { value: { command: e.name } }
  })
  on('fs.exists', (_, e) => ({ value: e.path.replace(`${ROOT}/`, '') in w.files }))
  on('fs.read', (_, e) => ({ value: w.files[e.path.replace(`${ROOT}/`, '')] ?? '' }))
  on('ui.status', (_, e) => {
    w.statuses.push(e.text)
    return { value: undefined }
  })
  on('ui.toast', (_, e) => {
    w.toasts.push(e.text)
    return { value: undefined }
  })
  on('ui.notify', (_, e) => {
    w.notices.push(e.text)
    return { value: { isSent: true } } as never
  })
  on('process.run', (_, e) => {
    const argv = [...e.argv]
    w.runs.push(argv)
    const isJson = argv.includes('--json')
    // --cards names cards the script may not know (no cards.md): it exits 1 then
    if (argv.some(a => a.startsWith('--cards=')) && w.cardsExit !== 0) return { value: { exitCode: w.cardsExit, stdout: '', stderr: 'budget: no such card' } }
    if (w.budget.exit !== 0) return { value: { exitCode: w.budget.exit, stdout: '', stderr: 'boom' } }
    return { value: { exitCode: 0, stdout: isJson ? JSON.stringify(w.budget.json) : w.budget.text, stderr: '' } }
  })
  return w
}

const FILES = { '.ai-gent/orchestrate-status.md': LEDGER, '.ai-gent/budget.yaml': 'session_billable: 12M\nweek_billable: 60M\n' }
const last = (w: World) => w.statuses.at(-1)

test('the status line follows the cards in flight and what the caps leave', async ($, on) => {
  const w = world(on, { ...FILES })
  await $.session.start(START)
  expect(w.runs.length).toBe(0) // detached: the budget does not hold the session's start
  await w.clock.settle()
  expect(w.registered).toContain('tokens')
  expect(last(w)).toBe('tokens · 004-us1-core ~420k/391k · 004-us2 ~300k/— · 004-us4 —/— · session left 3.10M · week left 22.00M')
  expect(w.runs[0]).toEqual(['python3', expect.stringMatching(/token_report\.py$/), 'budget', '--json', '--cards=004-us1-core,004-us2,004-us4'])
})

test('with a budget but no ledger only the budget shows; with neither, no status', async ($, on) => {
  const w = world(on, { '.ai-gent/budget.yaml': 'session_billable: 12M\n' })
  await $.session.start(START)
  await w.clock.settle()
  expect(last(w)).toBe('tokens · session left 3.10M · week left 22.00M')
  expect(w.runs[0]).toContain('--projected=0')
})

test('with neither a ledger nor a budget there is no status and nothing is run', async ($, on) => {
  const w = world(on, {})
  await $.session.start(START)
  await w.clock.settle()
  expect(last(w)).toBeUndefined()
  expect(w.runs).toEqual([])
})

test('a ledger with no budget file shows the cards and runs nothing', async ($, on) => {
  const w = world(on, { '.ai-gent/orchestrate-status.md': LEDGER })
  await $.session.start(START)
  await w.clock.settle()
  expect(last(w)).toBe('tokens · 004-us1-core ~420k/391k · 004-us2 ~300k/— · 004-us4 —/—')
  expect(w.runs).toEqual([])
})

test('a verdict that turns to shrink or stop is toasted once, and a stop also notifies', async ($, on) => {
  const w = world(on, { ...FILES })
  await $.session.start(START)
  await w.clock.settle()
  expect(w.toasts.filter(t => t.startsWith('budget:'))).toEqual([])

  w.budget.json = { ...BUDGET, verdict: 'shrink', why: 'only 004-us1-core fits; run token_report.py budget for the batches' }
  await $.command.run({ command: 'tokens', args: '' } as never)
  await $.command.run({ command: 'tokens', args: '' } as never)
  expect(w.toasts.filter(t => t.startsWith('budget:'))).toEqual(['budget: shrink — only 004-us1-core fits; run token_report.py budget for the batches'])
  expect(last(w)).toContain('⚠ shrink')
  expect(w.notices).toEqual([])

  w.budget.json = { ...BUDGET, verdict: 'stop', why: 'nothing fits' }
  await $.command.run({ command: 'tokens', args: '' } as never)
  expect(w.toasts.filter(t => t.startsWith('budget:')).at(-1)).toBe('budget: stop — nothing fits')
  expect(w.notices).toEqual(['budget: stop — nothing fits'])

  w.budget.json = BUDGET
  await $.command.run({ command: 'tokens', args: '' } as never)
  expect(last(w)).not.toContain('⚠')
  w.budget.json = { ...BUDGET, verdict: 'shrink', why: 'again' }
  await $.command.run({ command: 'tokens', args: '' } as never)
  expect(w.toasts.at(-1)).toBe('budget: shrink — again')
})

test('a card that passes its projection is toasted once; unknown figures are not compared', async ($, on) => {
  const ledger = LEDGER.replace('~420k / 391k', '~420k / 520k')
  const w = world(on, { ...FILES, '.ai-gent/orchestrate-status.md': ledger })
  await $.session.start(START)
  await w.clock.settle()
  await $.command.run({ command: 'tokens', args: '' } as never)
  expect(w.toasts.filter(t => t.includes('projected'))).toEqual(['004-us1-core used 520k, projected ~420k (+24%)'])
})

test('cards the script does not know fall back to the caps alone', async ($, on) => {
  const w = world(on, { ...FILES })
  w.cardsExit = 1
  await $.session.start(START)
  await w.clock.settle()
  expect(w.runs.map(a => a.at(-1))).toEqual(['--cards=004-us1-core,004-us2,004-us4', '--projected=0'])
  expect(last(w)).toContain('session left 3.10M')
})

test('/tokens prints the script\'s own text, and the mod only ever asks for a budget', async ($, on) => {
  const w = world(on, { ...FILES })
  await $.session.start(START)
  await w.clock.settle()
  expect((await $.command.run({ command: 'tokens', args: '' } as never)).text).toBe('wave: 1 card(s)\nverdict: proceed - within the 2M left')
  w.budget.exit = 2
  expect((await $.command.run({ command: 'tokens', args: '' } as never)).text).toContain('could not run')
  for (const argv of w.runs) {
    expect(argv[2], argv.join(' ')).toBe('budget') // never `tasks`, which writes
    expect(argv[0]).toBe('python3')
  }
})

test('/tokens without a budget file says how to set one up and runs nothing', async ($, on) => {
  const w = world(on, {})
  expect((await $.command.run({ command: 'tokens', args: '' } as never)).text).toContain('No .ai-gent/budget.yaml')
  expect(w.runs).toEqual([])
})

test('after a turn the budget is read again only once per refresh interval', async ($, on) => {
  const w = world(on, { ...FILES })
  await $.session.start(START)
  await w.clock.settle()
  const reads = () => w.runs.length
  const base = reads()
  await $.turn.complete({ text: 'done', reason: 'answer' } as never)
  await w.clock.advance(30_000)
  expect(reads()).toBe(base)
  await w.clock.advance(100_000)
  await $.turn.complete({ text: 'done', reason: 'answer' } as never)
  await w.clock.settle()
  expect(reads()).toBeGreaterThan(base)
})

test('tokenStatus off registers nothing of the token status (the pane is its own option)', { options: { tokenStatus: false, cardsPane: false } }, async ($, on) => {
  const w = world(on, { ...FILES })
  await $.session.start(START)
  await w.clock.settle()
  expect(w.registered).toEqual([])
  expect(w.runs).toEqual([])
})

// ------------------------------------------------- the burn rate in the flow

test('the status shows each cap\'s ETA and the mod spawns nothing more for it', async ($, on) => {
  const w = world(on, { ...FILES })
  w.budget.json = WITH_ETA
  await $.session.start(START)
  await w.clock.settle()
  expect(last(w)).toBe('tokens · 004-us1-core ~420k/391k · 004-us2 ~300k/— · 004-us4 —/— · session left 3.10M (~2h10m) · week left 22.00M (~15h)')
  expect(w.runs).toHaveLength(1)
})

test('tokenBurnRate off keeps 014\'s status and toasts no ETA', { options: { tokenBurnRate: false } }, async ($, on) => {
  const w = world(on, { ...FILES })
  w.budget.json = withEta(10)
  await $.session.start(START)
  await w.clock.settle()
  expect(last(w)).not.toContain('(~')
  expect(w.toasts).toEqual([])
})

test('the ETA toast fires at the first crossing, not on the repeat, and again after a recovery', async ($, on) => {
  const w = world(on, { ...FILES })
  await $.session.start(START)
  await w.clock.settle()
  const etaToasts = () => w.toasts.filter(t => t.includes('cap lasts'))
  const look = async (b: Budget) => {
    w.budget.json = b
    await $.command.run({ command: 'tokens', args: '' } as never)
  }
  await look(withEta(60))
  expect(etaToasts()).toEqual([])
  await look(withEta(25))
  expect(etaToasts()).toEqual(['session cap lasts ~25m at 7.40M/h — pause the wave or finish the card in flight'])
  await look(withEta(25))
  expect(etaToasts()).toHaveLength(1)
  await look(withEta(90))
  await look(withEta(20))
  expect(etaToasts()).toHaveLength(2)
  expect(w.notices).toEqual([]) // toast only
})

test('a cap with nothing left has no ETA toast', async ($, on) => {
  const w = world(on, { ...FILES })
  w.budget.json = withEta(5, null, 0)
  await $.session.start(START)
  await w.clock.settle()
  expect(w.toasts.filter(t => t.includes('cap lasts'))).toEqual([])
})

test('tokenEtaWarnMinutes sets the threshold', { options: { tokenEtaWarnMinutes: 10 } }, async ($, on) => {
  const w = world(on, { ...FILES })
  w.budget.json = withEta(25)
  await $.session.start(START)
  await w.clock.settle()
  expect(w.toasts).toEqual([]) // 25 is above a threshold of 10
  expect(last(w)).toContain('(~25m)')
})

test('tokenEtaWarnMinutes 0 keeps the status and toasts nothing', { options: { tokenEtaWarnMinutes: 0 } }, async ($, on) => {
  const w = world(on, { ...FILES })
  w.budget.json = withEta(5)
  await $.session.start(START)
  await w.clock.settle()
  expect(w.toasts).toEqual([])
  expect(last(w)).toContain('(~5m)')
})
