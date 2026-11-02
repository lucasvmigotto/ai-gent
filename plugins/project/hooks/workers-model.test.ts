import { expect, test } from 'claude-code/testing'

import type { WorkersSnapshot } from '../types'
import { ALL_STATES, LOG, NOW, statusJson, track } from './workers-fixtures'
import { build, compactLine, duration, parseCap, parseLog, parseStatus, sortWorkers, text } from './workers-model'

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
