import { expect, test } from 'claude-code/testing'

import type { WorkersSnapshot } from '../types'
import { AGENTS, ALL_STATES, LOG, NOW, PROFILE_AND_CODEX, statusJson, track } from './workers-fixtures'
import { agentRow, build, compactLine, duration, parseCap, parseLog, parseStatus, sortWorkers, text } from './workers-model'

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
  expect(compactLine(snapshot({ a: track('a', 'running'), b: track('b', 'running'), c: track('c', 'done') }, { cap: 1 }))).toEqual({ text: 'Workers: 2 running · 1 done · slots 2/1', tone: 'plain' })
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

test('golden: wide', () => {
  expect(lines(snapshot(ALL_STATES), { limits: LIMITS })).toBe(`Workers  slots 2/3 · 1 running · 1 queued · 1 stalled ⚠ · 1 failed ✕ · 1 done · 1 taken-back
Usage  5h 41% · week 12%
  STATE        AGENT            ROLE       TASKS TRACK         ELAPSED LAST EVENT   VERDICT      WORKTREE
▸ ▶ running    codex            builder    T010,T011 feat-core 10m     10s ago      · pending    .worktrees/feat-core
▸ ⚠ stalled    opencode:flint-… builder    T020 feat-ui        10m     8m ago       · pending    .worktrees/feat-ui
▸ · queued     codex            researcher T040 feat-docs      —       —            · pending    .worktrees/feat-docs
▸ ✕ failed     claude-subagent  reviewer   T030 feat-api       5m      —            ✕ rejected   .worktrees/feat-api
▸ ✓ done       claude           builder    T001 feat-db        6m      —            ✓ accepted   .worktrees/feat-db
▸ ↩ taken-back codex            builder    T002 feat-old       3m      —            · pending    .worktrees/feat-old`)
})

test('golden: medium drops the verdict and the worktree', () => {
  expect(lines(snapshot(ALL_STATES), { columns: 80 })).toBe(`Workers  slots 2/3 · 1 running · 1 queued · 1 stalled ⚠ · 1 failed ✕ · 1 done ·…
  STATE        AGENT            ROLE       TASKS TRACK         ELAPSED LAST EVE…
▸ ▶ running    codex            builder    T010,T011 feat-core 10m     10s ago
▸ ⚠ stalled    opencode:flint-… builder    T020 feat-ui        10m     8m ago
▸ · queued     codex            researcher T040 feat-docs      —       —
▸ ✕ failed     claude-subagent  reviewer   T030 feat-api       5m      —
▸ ✓ done       claude           builder    T001 feat-db        6m      —
▸ ↩ taken-back codex            builder    T002 feat-old       3m      —`)
})

test('golden: narrow is one short row per worker', () => {
  expect(lines(snapshot(ALL_STATES), { columns: 40 })).toBe(`Workers  slots 2/3 · 1 running · 1 queu…
▸ ▶ codex feat-core 10m
▸ ⚠ opencode:flint-40b-free feat-ui 10m
▸ · codex feat-docs —
▸ ✕ claude-subagent feat-api 5m
▸ ✓ claude feat-db 6m
▸ ↩ codex feat-old 3m`)
})

test('golden: ascii fallback uses no non-ASCII glyph', () => {
  const out = lines(snapshot(ALL_STATES), { ascii: true })
  expect(out).toBe(`Workers  slots 2/3 | 1 running | 1 queued | 1 stalled ! | 1 failed x | 1 done | 1 taken-back
  STATE        AGENT            ROLE       TASKS TRACK         ELAPSED LAST EVENT   VERDICT      WORKTREE
> * running    codex            builder    T010,T011 feat-core 10m     10s ago      . pending    .worktrees/feat-core
> ! stalled    opencode:flin... builder    T020 feat-ui        10m     8m ago       . pending    .worktrees/feat-ui
> . queued     codex            researcher T040 feat-docs      -       -            . pending    .worktrees/feat-docs
> x failed     claude-subagent  reviewer   T030 feat-api       5m      -            x rejected   .worktrees/feat-api
> + done       claude           builder    T001 feat-db        6m      -            + accepted   .worktrees/feat-db
> < taken-back codex            builder    T002 feat-old       3m      -            . pending    .worktrees/feat-old`)
  expect(out).toMatch(/^[\x20-\x7e\n]*$/)
})

test('a row opens to its verdict, events, tokens and the log cost lines', () => {
  const out = lines(snapshot(ALL_STATES), { expanded: { 'feat-db': true, 'feat-api': true } })
  expect(out).toContain('    verdict: accepted · 12 events · 43k tokens')
  expect(out).toContain('    log: 1 run, 6m wall, 43k tokens')
  expect(out).toContain('    verdict: rejected (out-of-scope-path) · 12 events')
  expect(out).not.toContain('verdict: pending')
})

test('maxRows cuts the list and says how many more', () => {
  expect(lines(snapshot(ALL_STATES), { maxRows: 2 })).toContain('+4 more')
})

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

test('all three kinds count together: header, slots and the pipeline line', () => {
  const s = mixed()
  expect(compactLine(s)).toEqual({ text: 'Workers: 3 running · 1 queued · slots 3/3', tone: 'plain' })
  expect(lines(s).split('\n')[0]).toBe('Workers  slots 3/3 · 3 running · 1 queued')
})

test('golden: wide with the kind tag, no description', () => {
  expect(lines(mixed())).toBe(`Workers  slots 3/3 · 3 running · 1 queued
  STATE        KIND AGENT            ROLE       TASKS TRACK      ELAPSED LAST EVENT   VERDICT      WORKTREE
▸ ▶ running    prof claude-profile:… builder    T060 feat-prof   10m     5s ago       · pending    .worktrees/feat-prof
▸ ▶ running    ext  codex            builder    T061 feat-cx     10m     10s ago      · pending    .worktrees/feat-cx
▸ ▶ running    sub  claude-subagent  builder    sub-a1b2c3d4e5f6 10m     —            · pending    —
▸ · queued     sub  claude-subagent  researcher sub-e1           —       —            · pending    —`)
})

test('golden: the description appears only with the option', () => {
  const out = lines(mixed({}, true))
  expect(out).toContain('sub-e1: survey [31mthe repo'.replace('\u001b', ''))
  expect(lines(mixed({}, false))).not.toContain('survey')
})

test('golden: narrow and ascii keep a kind tag', () => {
  expect(lines(mixed(), { columns: 50 })).toBe(`Workers  slots 3/3 · 3 running · 1 queued
▸ ▶ prof claude-profile:work feat-prof 10m
▸ ▶ ext codex feat-cx 10m
▸ ▶ sub claude-subagent sub-a1b2c3d4e5f6 10m
▸ · sub claude-subagent sub-e1 —`)
  expect(lines(mixed(), { ascii: true })).toMatch(/^[\x20-\x7e\n]*$/)
})
