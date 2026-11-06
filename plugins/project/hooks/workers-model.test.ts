import { expect, test } from 'claude-code/testing'

import type { WorkersSnapshot } from '../types'
import { AGENTS, ALL_STATES, LOG, NOW, PROFILE_AND_CODEX, statusJson, track } from './workers-fixtures'
import { HISTORY_KEY, agentRow, build, compactLine, duration, parseCap, parseLog, parseStatus, sortWorkers, text } from './workers-model'

const snapshot = (files: Record<string, string>, over: Partial<WorkersSnapshot> = {}): WorkersSnapshot => ({
  workers: Object.values(files).flatMap(t => parseStatus(t) ?? []),
  log: parseLog(LOG),
  cap: 3,
  skipped: 0,
  computedAt: NOW,
  ...over,
})
const ctx = { columns: 120, now: NOW, ascii: false, maxRows: 30, expanded: {} }

test('a status file parses to its fields; tokens are input plus output', () => {
  const w = parseStatus(ALL_STATES['feat-db'] as string)
  expect(w?.state).toBe('done')
  expect(w?.tokens).toBe(42_500)
  expect(w?.verdict).toEqual({ result: 'accepted', reason: null })
  expect(parseStatus(statusJson())?.tokens).toBeNull()
})

test('a file that is not well-formed is refused, never drawn', () => {
  expect(parseStatus('{')).toBeNull()
  expect(parseStatus('[]')).toBeNull()
  expect(parseStatus(statusJson({ track: '../x' }))).toBeNull()
  expect(parseStatus(statusJson({ state: 'paused' }))).toBeNull()
  expect(parseStatus(statusJson({ role: 'boss' }))).toBeNull()
  expect(parseStatus(statusJson({ tasks: ['rm -rf'] }))).toBeNull()
  expect(parseStatus(statusJson({ worktree: '/etc' }))).toBeNull()
  expect(parseStatus(statusJson({ worktree: '../up' }))).toBeNull()
  expect(parseStatus(statusJson({ agent: 'x\u001b[31m' }))).toBeNull()
  expect(parseStatus(statusJson({ started: -1 }))).toBeNull()
})

test('control characters never reach a line; a reason that is not a rule id is dropped', () => {
  const w = parseStatus(statusJson({ worktree: '.worktrees/a\u001b]0;x\u0007b', verdict: { result: 'rejected', reason: 'a diff --git\n+secret' } }))
  expect(w?.worktree).toBe('.worktrees/a]0;xb')
  expect(w?.verdict.reason).toBeNull()
})

test('fields outside the contract (a prompt, a brief) are not carried', () => {
  const w = parseStatus(statusJson({ prompt: 'SECRET-BRIEF', brief: 'SECRET-BRIEF' }))
  expect(JSON.stringify(w)).not.toContain('SECRET')
})

test('the log skips malformed lines; the cap reads max_parallel', () => {
  expect(parseLog(LOG).map(l => l.track)).toEqual(['feat-db', 'feat-old'])
  expect(parseLog('')).toEqual([])
  expect(parseCap('max_parallel: 2  # two\nx: 1')).toBe(2)
  expect(parseCap('max_parallel: 0')).toBe(3)
  expect(parseCap(null)).toBe(3)
})

test('durations', () => {
  expect([0, 45, 60, 720, 3900, 90_000].map(duration)).toEqual(['0s', '45s', '1m', '12m', '1h 05m', '1d 1h'])
})

test('rows sort running, stalled, queued, failed, done, taken back', () => {
  expect(sortWorkers(snapshot(ALL_STATES).workers).map(w => w.state)).toEqual(['running', 'stalled', 'queued', 'failed', 'done', 'taken-back'])
})

test('no workers: the pane says No workers and the pipeline line adds nothing', () => {
  expect(text(snapshot({}), ctx)).toBe('No workers')
  expect(text(null, ctx)).toBe('No workers')
  expect(compactLine(snapshot({}))).toBeNull()
  expect(compactLine(null)).toBeNull()
  expect(build(snapshot({}), ctx).isEmpty).toBe(true)
})

test('unreadable status files are counted', () => {
  expect(text(snapshot({}, { skipped: 2 }), ctx)).toBe('No workers\n2 status files unreadable')
})

test('compact line: counts, slots, a marker for stalled or failed', () => {
  expect(compactLine(snapshot({ a: track('a', 'running'), b: track('b', 'running'), c: track('c', 'done') }, { cap: 1 }))).toEqual({ text: 'Workers: 2 running · 1 done · slots 2/1', tone: 'bad' })
  expect(compactLine(snapshot({ a: track('a', 'running'), c: track('c', 'done') }, { cap: 3 }))?.text).toBe('Workers: 1 running · 1 done · slots 1/3')
  expect(compactLine(snapshot(ALL_STATES))).toEqual({ text: 'Workers: 1 running · 1 queued · 1 stalled ⚠ · 1 failed ✕ · 1 done · 1 taken-back · slots 2/3', tone: 'bad' })
  expect(compactLine(snapshot({ a: track('a', 'stalled') }), true)).toEqual({ text: 'Workers: 1 stalled ! | slots 1/3', tone: 'warn' })
})

test('the compact line never lists a worker', () => {
  const line = compactLine(snapshot(ALL_STATES))?.text ?? ''
  for (const name of ['feat-core', 'codex', 'T010', 'worktrees']) expect(line).not.toContain(name)
})

const lines = (s: WorkersSnapshot, over: Record<string, unknown> = {}) =>
  build(s, { ...ctx, ...over } as never).lines.map(l => l.text).join('\n')
const LIMITS = [{ kind: 'five_hour', percent: 41 }, { kind: 'seven_day', percent: 12 }, { kind: 'spend_limit', percent: 3 }]







test('every line fits the width', () => {
  for (const columns of [20, 30, 40, 60, 80, 100, 140]) for (const l of lines(snapshot(ALL_STATES), { columns, expanded: { 'feat-db': true } }).split('\n')) expect([...l].length).toBeLessThanOrEqual(columns)
})

test('the text never carries a prompt, brief or credential word from the files', () => {
  const out = text(snapshot({ a: statusJson({ prompt: 'hunter2', brief: 'hunter2', env: { TOKEN: 'hunter2' } }) }), ctx)
  expect(out).not.toContain('hunter2')
})

// Three kinds together: two session subagents, a profile track and a Codex track.
const SEEN = { first: 1_700_000_000, changed: 1_700_000_540 }
const mixed = (over: Partial<WorkersSnapshot> = {}, showDescription = false): WorkersSnapshot =>
  snapshot(PROFILE_AND_CODEX, { workers: [...Object.values(PROFILE_AND_CODEX).flatMap(t => parseStatus(t) ?? []), ...AGENTS.slice(0, 2).map(a => agentRow(a, SEEN, showDescription))], ...over })

test('kinds: a status file is a profile or an ordinary agent by its agent id', () => {
  expect(parseStatus(PROFILE_AND_CODEX['feat-prof'] as string)?.kind).toBe('profile')
  expect(parseStatus(PROFILE_AND_CODEX['feat-cx'] as string)?.kind).toBe('agent')
  expect(parseStatus(statusJson({ agent: 'claude-profile' }))?.kind).toBe('profile')
  expect(parseStatus(statusJson({ agent: 'claude-profiles' }))?.kind).toBe('agent')
  expect(parseStatus(statusJson({ agent: 'claude' }))?.kind).toBe('profile')
  expect(parseStatus(statusJson({ agent: 'claude:work' }))?.kind).toBe('profile')
  expect(parseStatus(statusJson({ agent: 'claude-subagent' }))?.kind).toBe('agent')
})

test('a subagent row: agent, role from the type, state from the status, no description', () => {
  const rows = AGENTS.map(a => agentRow(a, SEEN, false))
  expect(rows.map(r => [r.kind, r.agent, r.role, r.state])).toEqual([
    ['subagent', 'claude-subagent', 'builder', 'running'],
    ['subagent', 'claude-subagent', 'researcher', 'queued'],
    ['subagent', 'claude-subagent', 'reviewer', 'done'],
    ['subagent', 'claude-subagent', 'builder', 'failed'],
  ])
  expect(rows[0]?.track).toBe('sub-a1b2c3d4e5f6')
  expect(JSON.stringify(rows)).not.toContain('PROMPT-SECRET')
  expect(rows.every(r => r.note === null)).toBe(true)
})

test('every agent status maps to a state', () => {
  const states = (['pending', 'running', 'waiting', 'idle', 'completed', 'failed', 'killed'] as const).map(status => agentRow({ ...AGENTS[0], status } as never, SEEN, false).state)
  expect(states).toEqual(['queued', 'running', 'running', 'queued', 'done', 'failed', 'taken-back'])
})

test('the description is carried only when asked, without control characters, and cut', () => {
  const on = agentRow(AGENTS[1] as never, SEEN, true)
  expect(on.note).toBe('survey [31mthe repo'.replace('\u001b', ''))
  expect(on.note).not.toMatch(/[\u0000-\u001f]/)
  expect(agentRow({ ...AGENTS[0], description: 'y'.repeat(500) } as never, SEEN, true).note?.length).toBe(60)
  expect(agentRow({ ...AGENTS[0], id: 'a\u001b]0;x\u0007/../b', type: 'T\u001b[1m' } as never, SEEN, false).track).toBe('sub-a0x..b')
})



test('golden: the description appears only with the option', () => {
  const out = lines(mixed({}, true))
  expect(out).toContain('sub-e1: survey [31mthe repo'.replace('\u001b', ''))
  expect(lines(mixed({}, false))).not.toContain('survey')
})



// The redesigned pane (hierarchy: overview, capacity, attention, workers, history).
const CAP_ROWS = (s: WorkersSnapshot, over: Record<string, unknown> = {}) => lines(s, over).split('\n').filter(l => /CAPACITY|^\[|^\S.*slots/.test(l) || /available|full|over the limit/.test(l))

test('golden: wide has overview, capacity, attention, a column header and a collapsed history', () => {
  expect(lines(snapshot(ALL_STATES), { limits: LIMITS })).toBe(`WORKERS                                             1 failed · 1 stalled
1 running · 1 queued · 1 stalled ⚠ · 1 failed ✕ · 1 done · 1 taken-back
Usage  5h 41% · week 12%
CAPACITY                                                     2 / 3 slots
[■] [■] [ ]                                                  1 available

ATTENTION · 2 issues
▸ ⚠ feat-ui stalled · T020 · 10m
▸ ✕ feat-api failed · T030 · 5m · verdict rejected

    WORKER / TRACK  KIND  STATE       TASKS                   TIME
▸ ▶ feat-core       ext   Running     T010, T011              10m
▸ ⚠ feat-ui         ext   Stalled     T020                    10m
▸ ○ feat-docs       ext   Queued      T040                    —
▸ ✕ feat-api        ext   Failed      T030                    5m
▸ History · 1 done · 1 taken back`)
})

test('golden: medium is two lines per worker, state flush right', () => {
  expect(lines(snapshot(ALL_STATES), { columns: 70 })).toContain(`WORKERS
▸ ▶ feat-core                                                  Running
    T010, T011 · 10m · builder · ext · verdict pending
▸ ⚠ feat-ui                                                    Stalled`)
})

test('golden: narrow keeps state visible and a compact capacity', () => {
  const out = lines(snapshot(ALL_STATES), { columns: 40 })
  expect(out).toContain('CAPACITY         2/3 slots · 1 available')
  expect(out).not.toContain('[■]')
  expect(out).toContain('▸ ✕ feat-api                      Failed\n    T030 · 5m · ext')
})

test('golden: ascii uses only ASCII', () => {
  const out = lines(snapshot(ALL_STATES), { ascii: true })
  expect(out).toMatch(/^[\x20-\x7e\n]*$/)
  expect(out).toContain('[#] [#] [ ]')
  expect(out).toContain('> o feat-docs')
})

test('capacity: 0/3, 2/3, 3/3, over, unknown, wide cap and a narrow pane', () => {
  const none = snapshot({ a: track('a', 'queued') })
  expect(CAP_ROWS(none).join('\n')).toContain('0 / 3 slots')
  expect(lines(none)).toContain('[ ] [ ] [ ]')
  expect(lines(none)).toContain('3 available')
  expect(lines(snapshot({ a: track('a', 'running'), b: track('b', 'stalled') }))).toContain('[■] [■] [ ]')
  const full = lines(snapshot({ a: track('a', 'running'), b: track('b', 'running'), c: track('c', 'running') }))
  expect(full).toContain('3 / 3 slots')
  expect(full).toContain('full')
  const over = lines(snapshot({ a: track('a', 'running'), b: track('b', 'running') }, { cap: 1 }))
  expect(over).toContain('2 / 1 slots')
  expect(over).toContain('1 over the limit')
  const unknown = lines(snapshot({ a: track('a', 'running') }, { cap: 0 }))
  expect(unknown).toContain('1 in use (limit unknown)')
  expect(unknown).not.toContain('[')
  expect(lines(snapshot({ a: track('a', 'running') }, { cap: 20 }))).toContain('1/20 slots · 19 available')
  expect(lines(none, { columns: 40 })).toContain('0/3 slots · 3 available')
})

// Spec 042: the slots beyond the cap are [!] cells in the error tone, and the text is what /workers prints.
const running = (n: number): Record<string, string> => Object.fromEntries(Array.from({ length: n }, (_, i) => [`w${i}`, track(`w${i}`, 'running')]))
// The CAPACITY line and the meter under it, when there is one (build renumbers the keys, so by text).
const capLines = (n: number, cap: number, over: Record<string, unknown> = {}) => {
  const all = build(snapshot(running(n), { cap }), { ...ctx, ...over } as never).lines
  const at = all.findIndex(l => l.text.startsWith('CAPACITY'))
  return all.slice(at, at + (all[at + 1]?.text.startsWith('[') ? 2 : 1))
}
const slice = (l: { text: string; marks?: { from: number; to: number }[] }) => (l.marks ?? []).map(m => l.text.slice(m.from, m.to))

test('over the limit: the meter grows to the used slots, the extra cells are [!] in the error tone', () => {
  const [figure, meter] = capLines(6, 3)
  expect(figure?.tone).toBe('bad')
  expect(figure?.text).toContain('6 / 3 slots')
  expect(meter?.text).toMatch(/^\[■\] \[■\] \[■\] \[!\] \[!\] \[!\] +3 over the limit$/)
  expect(meter?.tone).toBe('info')
  expect(slice(meter as never)).toEqual(['[!] [!] [!]', '3 over the limit'])
  expect(meter?.marks?.every(m => m.tone === 'bad')).toBe(true)
})

test('over the limit: one cell over, and the figure is the same as before', () => {
  const [, meter] = capLines(4, 3)
  expect(meter?.text).toMatch(/^\[■\] \[■\] \[■\] \[!\] +1 over the limit$/)
  expect(slice(meter as never)).toEqual(['[!]', '1 over the limit'])
})

test('over the limit in ASCII: [#] and [!], and the text stays ASCII', () => {
  const lines2 = capLines(5, 3, { ascii: true })
  const meter = lines2[1]
  expect(meter?.text).toMatch(/^\[#\] \[#\] \[#\] \[!\] \[!\] +2 over the limit$/)
  expect(lines2.map(l => l.text).join('\n')).toMatch(/^[\x20-\x7e\n]*$/)
})

test('not over the limit: no marks, no [!], the same tones as before', () => {
  for (const [n, tone] of [[2, 'info'], [3, 'info']] as const) {
    const [figure, meter] = capLines(n, 3)
    expect(figure?.tone).toBe(tone)
    expect(meter?.marks).toBeUndefined()
    expect(meter?.text).not.toContain('[!]')
  }
})

test('over the limit falls back to one error-toned line: narrow pane, wide cap, too many slots, no room', () => {
  for (const [n, cap, over] of [[5, 3, { columns: 40 }], [20, 14, {}], [14, 3, {}], [12, 3, { columns: 60 }]] as const) {
    const out = capLines(n, cap, over)
    expect(out).toHaveLength(1)
    expect(out[0]?.tone).toBe('bad')
    expect(out[0]?.marks).toBeUndefined()
    expect(out[0]?.text).toContain('over the limit')
  }
  // The same slots on a pane that has room keep the meter.
  expect(capLines(12, 3, { columns: 72 })).toHaveLength(2)
})

test('the marks always cut the line the model printed', () => {
  for (const columns of [60, 62, 72, 100]) {
    for (const n of [4, 6, 8]) {
      for (const l of capLines(n, 3, { columns })) {
        for (const m of l.marks ?? []) {
          expect(m.from).toBeGreaterThanOrEqual(0)
          expect(m.to).toBeLessThanOrEqual(l.text.length)
          expect(l.text.slice(m.from, m.to).trim()).not.toBe('')
        }
      }
    }
  }
})

test('the pipeline line is error-toned over the limit, below failed and above stalled, and its text is unchanged', () => {
  const over = compactLine(snapshot(running(4), { cap: 3 }))
  expect(over).toEqual({ text: 'Workers: 4 running · slots 4/3', tone: 'bad' })
  expect(compactLine(snapshot(running(3), { cap: 3 }))?.tone).toBe('plain')
  expect(compactLine(snapshot({ ...running(4), s: track('s', 'stalled') }, { cap: 3 }))?.tone).toBe('bad')
  expect(compactLine(snapshot(running(2), { cap: 0 }))?.tone).toBe('plain')
})

test('a queued worker with free capacity is never called blocked or waiting for a slot', () => {
  const out = text(snapshot({ a: track('a', 'queued') }), ctx)
  expect(out).toContain('3 available')
  expect(out).toContain('Queued')
  expect(out).not.toMatch(/block|waiting for|no slot|wait/i)
})

test('attention lists failed and stalled only, stalled first, and is absent otherwise', () => {
  const out = lines(snapshot(ALL_STATES))
  expect(out).toContain('ATTENTION · 2 issues')
  expect(out.indexOf('⚠ feat-ui stalled')).toBeLessThan(out.indexOf('✕ feat-api failed'))
  expect(lines(snapshot({ a: track('a', 'running'), b: track('b', 'done') }))).not.toContain('ATTENTION')
  expect(lines(snapshot({ a: track('a', 'failed') }))).toContain('ATTENTION · 1 issue\n')
  const view = build(snapshot(ALL_STATES), ctx)
  const row = view.lines.find(l => l.buttonKey === 'attention:feat-api')
  expect(row?.toggle).toBe('feat-api')
})

test('a failure without a verdict reason or events invents none', () => {
  const out = lines(snapshot({ a: track('a', 'failed', { events: 0 }) }), { expanded: { a: true } })
  expect(out).toContain('Verdict: pending')
  expect(out).toContain('Last event: 0 events')
  expect(out).not.toMatch(/reason|because|retry|fix/i)
})

test('history is collapsed with its count; opening it lists done and taken-back rows', () => {
  const closed = lines(snapshot(ALL_STATES))
  expect(closed).toContain('▸ History · 1 done · 1 taken back')
  expect(closed).not.toContain('feat-db')
  expect(build(snapshot(ALL_STATES), ctx).lines.find(l => l.toggle === HISTORY_KEY)).toBeDefined()
  const open = lines(snapshot(ALL_STATES), { expanded: { [HISTORY_KEY]: true } })
  expect(open).toContain('▾ History')
  expect(open).toContain('✓ feat-db')
  expect(open).toContain('↩ feat-old')
})

test('only done workers: no active rows, history still counted', () => {
  const out = lines(snapshot({ a: track('a', 'done') }))
  expect(out).toContain('No active workers')
  expect(out).toContain('History · 1 done')
})

test('execution state and verdict stay independent', () => {
  const out = lines(snapshot({ a: track('a', 'done'), b: track('b', 'failed', { verdict: { result: 'accepted' } }), c: track('c', 'running', { verdict: { result: 'rejected', reason: 'x-rule' } }) }), { expanded: { a: true, b: true, c: true, [HISTORY_KEY]: true } })
  expect(out).toMatch(/State: Done[^\n]*\n\s+Verdict: pending/)
  expect(out).toMatch(/State: Failed[^\n]*\n\s+Verdict: accepted/)
  expect(out).toContain('Verdict: accepted')
  expect(out).toContain('Verdict: rejected (x-rule)')
})

test('a row opens to its details, shown only when available', () => {
  const out = lines(snapshot(ALL_STATES), { expanded: { 'feat-db': true, [HISTORY_KEY]: true } })
  expect(out).toContain('    Track: feat-db')
  expect(out).toContain('    Tokens: 43k')
  expect(out).toContain('    Log: 1 run, 6m wall, 43k tokens')
  expect(out).toContain('    Worktree: .worktrees/feat-db')
  const sub = lines(mixed(), { expanded: { 'sub-e1': true } })
  expect(sub).toContain('    Track: sub-e1')
  expect(sub).not.toMatch(/Worktree|Tokens|Tasks:|Last event/)
})

test('long values wrap in details and truncate with an ellipsis in rows, never past the width', () => {
  const long = 'a'.repeat(99)
  const s = snapshot({ x: track(long, 'failed', { worktree: `.worktrees/${'b'.repeat(100)}`, tasks: Array.from({ length: 30 }, (_, i) => `T${100 + i}`) }) })
  for (const columns of [20, 40, 60, 80, 100, 140]) {
    const out = lines(s, { columns, expanded: { [long]: true } })
    for (const l of out.split('\n')) expect([...l].length).toBeLessThanOrEqual(columns)
    expect(out).toContain('…')
    expect(out.replace(/\n\s+/g, '')).toContain('b'.repeat(100))
  }
})

test('a large list is cut at maxRows and says how many more', () => {
  const files = Object.fromEntries(Array.from({ length: 40 }, (_, i) => [`w${i}`, track(`w${i}`, 'running')]))
  const out = lines(snapshot(files), { maxRows: 10 })
  expect(out.match(/▶ w\d+/g)?.length).toBe(10)
  expect(out).toContain('+30 more')
})

test('counts in the header are the snapshot counts', () => {
  const view = build(snapshot(ALL_STATES), ctx)
  expect(view.counts).toEqual({ running: 1, queued: 1, stalled: 1, failed: 1, done: 1, 'taken-back': 1 })
  expect(view.slots).toEqual({ used: 2, cap: 3 })
  expect(lines(snapshot(ALL_STATES)).split('\n')[1]).toBe('1 running · 1 queued · 1 stalled ⚠ · 1 failed ✕ · 1 done · 1 taken-back')
})

test('all three kinds count together and show: header, capacity, kind column and pipeline line', () => {
  const s = mixed()
  expect(compactLine(s)).toEqual({ text: 'Workers: 3 running · 1 queued · slots 3/3', tone: 'plain' })
  const out = lines(s)
  expect(out.split('\n')[1]).toBe('3 running · 1 queued')
  expect(out).toContain('3 / 3 slots')
  expect(out).toMatch(/feat-prof\s+prof\s+Running/)
  expect(out).toMatch(/feat-cx\s+ext\s+Running/)
  expect(out).toMatch(/sub-a1b2c3d4e5f6\s+sub\s+Running/)
  expect(out).toMatch(/sub-e1\s+sub\s+Queued/)
  expect(out).not.toContain('ATTENTION')
})

test('the description appears only with the option', () => {
  expect(lines(mixed({}, true))).toContain('sub-e1: survey [31mthe repo'.replace('\u001b', ''))
  expect(lines(mixed({}, false))).not.toContain('survey')
})

test('text() opens every row and the history, trimmed', () => {
  const out = text(snapshot(ALL_STATES), { columns: 80, now: NOW, ascii: false, maxRows: 30 })
  for (const name of ['feat-core', 'feat-ui', 'feat-docs', 'feat-api', 'feat-db', 'feat-old']) expect(out).toContain(`Track: ${name}`)
  expect(out.split('\n').every(l => l === l.trimEnd())).toBe(true)
})
