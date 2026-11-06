// The workers pane's model (spec 041, FR-016 to FR-018), pure: status files and the
// delegation log in, lines and the compact pipeline line out. Counts and states only:
// the status file has no prompt, brief or credential field and nothing here asks for one.
// Every string read from a file is validated or stripped of control characters, because
// a file is data and a terminal escape in a track name must never reach the screen.

import type { AgentInfo } from 'claude-code'

import type { WorkerKind, WorkerLogLine, WorkerState, WorkerStatus, WorkersSnapshot } from '../types'

export type Tone = 'plain' | 'dim' | 'bold' | 'good' | 'warn' | 'bad' | 'info'

export const STATES: readonly WorkerState[] = ['queued', 'running', 'stalled', 'done', 'failed', 'taken-back']
const ROLES = ['builder', 'reviewer', 'researcher'] as const
const RESULTS = ['pending', 'accepted', 'rejected'] as const

export const DEFAULT_CAP = 3
export const MAX_STATUS_BYTES = 65_536
export const MAX_LOG_LINES = 2000
export const NARROW = 60
export const WIDE = 100

const TRACK = /^[a-z0-9][a-z0-9._-]{0,99}$/
const AGENT = /^[a-z0-9][a-z0-9._-]*(:[A-Za-z0-9._/-]+)?$/
const TASK = /^T\d+[a-z]?$/
const RULE = /^[a-z0-9][a-z0-9._-]{0,63}$/

const isObject = (v: unknown): v is Record<string, unknown> => typeof v === 'object' && v !== null && !Array.isArray(v)
const isCount = (v: unknown): v is number => typeof v === 'number' && Number.isFinite(v) && v >= 0
const isState = (v: unknown): v is WorkerState => typeof v === 'string' && (STATES as readonly string[]).includes(v)

// Control characters out, length cut: for the one free-text field (the worktree path).
function clean(value: string, max: number): string {
  // eslint-disable-next-line no-control-regex
  return value.replace(/[\u0000-\u001f\u007f-\u009f]/g, '').slice(0, max)
}

function tokensOf(value: unknown): number | null {
  if (!isObject(value)) return null
  const input = isCount(value.input_tokens) ? value.input_tokens : 0
  const output = isCount(value.output_tokens) ? value.output_tokens : 0
  return input + output
}

// Kinds 2 and 3 are status-file tracks: another Claude profile (`claude` or `claude-profile[:name]`) or any other
// code agent. Kind 1, a subagent of this session, comes from `$.agent.list()` (agentRow below).
export const kindOfAgent = (agent: string): WorkerKind => (/^claude(-profile)?(:|$)/.test(agent) ? 'profile' : 'agent')

const KIND_TAG: Record<WorkerKind, string> = { subagent: 'sub', profile: 'prof', agent: 'ext' }

// One status.json, or null when it is not well-formed (the pane counts those, never draws them).
export function parseStatus(text: string): WorkerStatus | null {
  let raw: unknown
  try {
    raw = JSON.parse(text)
  } catch {
    return null
  }
  if (!isObject(raw)) return null
  const { track, agent, role, tasks, state, started, updated, events, worktree, verdict, tokens } = raw
  if (typeof track !== 'string' || !TRACK.test(track)) return null
  if (typeof agent !== 'string' || agent.length > 80 || !AGENT.test(agent)) return null
  if (typeof role !== 'string' || !(ROLES as readonly string[]).includes(role)) return null
  if (!isState(state) || !isCount(started) || !isCount(updated)) return null
  if (!Array.isArray(tasks) || tasks.length > 40 || !tasks.every(t => typeof t === 'string' && TASK.test(t))) return null
  if (typeof worktree !== 'string' || worktree === '' || worktree.startsWith('/') || worktree.split(/[\\/]/).includes('..')) return null
  let result: (typeof RESULTS)[number] = 'pending'
  let reason: string | null = null
  if (isObject(verdict)) {
    if (typeof verdict.result === 'string' && (RESULTS as readonly string[]).includes(verdict.result)) result = verdict.result as typeof result
    if (typeof verdict.reason === 'string' && RULE.test(verdict.reason)) reason = verdict.reason
  }
  return {
    track,
    agent,
    role: role as WorkerStatus['role'],
    tasks: tasks as string[],
    state,
    started,
    updated,
    events: isCount(events) ? Math.floor(events) : 0,
    worktree: clean(worktree, 120),
    verdict: { result, reason },
    tokens: tokensOf(tokens),
    kind: kindOfAgent(agent),
    note: null,
  }
}

// $.agent.list() status to a worker state: an idle agent waits for a message and is not working (queued,
// no slot); one waiting on a tool or a child is mid-task (running).
const AGENT_STATE: Record<AgentInfo['status'], WorkerState> = { pending: 'queued', running: 'running', waiting: 'running', idle: 'queued', completed: 'done', failed: 'failed', killed: 'taken-back' }

// Control characters out, anything outside a safe id alphabet dropped.
const slug = (value: string, max: number): string => clean(value, 200).replace(/[^A-Za-z0-9._-]/g, '').slice(0, max)

function roleOfType(type: string): WorkerStatus['role'] {
  const t = type.toLowerCase()
  if (t.includes('review') || t.includes('audit')) return 'reviewer'
  if (t === 'explore' || t === 'plan' || t.includes('research') || t.includes('search') || t.includes('survey')) return 'researcher'
  return 'builder'
}

// One session subagent (kind 1) as a worker row. `first` and `changed` are epoch seconds the caller saw it
// (the API gives no clock). The description is the Agent call's free text, which may be a prompt: it is
// carried only when `showDescription` is set, cut and stripped of control characters (FR-017).
export function agentRow(info: AgentInfo, seen: { first: number; changed: number }, showDescription: boolean): WorkerStatus {
  const id = slug(String(info.id), 40) || 'agent'
  const short = id.length > 12 ? id.slice(0, 12) : id
  const note = showDescription ? clean(String(info.description ?? ''), 60).trim() : ''
  return {
    track: `sub-${short}`,
    agent: 'claude-subagent',
    role: roleOfType(slug(String(info.type), 40)),
    tasks: [],
    state: AGENT_STATE[info.status] ?? 'running',
    started: seen.first,
    updated: seen.changed,
    events: 0,
    worktree: '',
    verdict: { result: 'pending', reason: null },
    tokens: null,
    kind: 'subagent',
    note: note === '' ? null : note,
  }
}

// The well-formed lines of the delegation log; a malformed line is skipped. Only the last MAX_LOG_LINES count.
export function parseLog(text: string): WorkerLogLine[] {
  const out: WorkerLogLine[] = []
  for (const line of text.split('\n').slice(-MAX_LOG_LINES)) {
    if (line.trim() === '') continue
    let raw: unknown
    try {
      raw = JSON.parse(line)
    } catch {
      continue
    }
    if (!isObject(raw)) continue
    const { ts, track, agent, tasks, wall_secs: wall, tokens, result } = raw
    if (!isCount(ts) || typeof track !== 'string' || !TRACK.test(track) || typeof agent !== 'string' || !AGENT.test(agent) || !isCount(wall) || !isState(result)) continue
    out.push({ ts, track, agent, tasks: Array.isArray(tasks) ? tasks.filter((t): t is string => typeof t === 'string' && TASK.test(t)) : [], wallSecs: wall, tokens: tokensOf(tokens), result })
  }
  return out
}

// `max_parallel: 3` from .ai-gent/parallel.yaml (the key wt.py reads), else the default.
export function parseCap(text: string | null): number {
  const m = text === null ? null : /^\s*max_parallel\s*:\s*([0-9]+)\s*(#.*)?$/m.exec(text)
  const n = m === null ? Number.NaN : Number.parseInt(m[1] as string, 10)
  return Number.isFinite(n) && n >= 1 && n <= 99 ? n : DEFAULT_CAP
}

const glyphs = (ascii: boolean) => ({
  sep: ascii ? ' | ' : ' · ',
  warn: ascii ? '!' : '⚠',
  fail: ascii ? 'x' : '✕',
  ellipsis: ascii ? '...' : '…',
  open: ascii ? 'v' : '▾',
  closed: ascii ? '>' : '▸',
  none: ascii ? '-' : '—',
})

const STATE_GLYPH: Record<WorkerState, [string, string]> = {
  queued: ['○', 'o'],
  running: ['▶', '*'],
  stalled: ['⚠', '!'],
  done: ['✓', '+'],
  failed: ['✕', 'x'],
  'taken-back': ['↩', '<'],
}
const STATE_TONE: Record<WorkerState, Tone> = { queued: 'dim', running: 'info', stalled: 'warn', done: 'good', failed: 'bad', 'taken-back': 'dim' }
const VERDICT_GLYPH: Record<string, [string, string]> = { pending: ['·', '.'], accepted: ['✓', '+'], rejected: ['✕', 'x'] }

export type Counts = Record<WorkerState, number>

export function countsOf(workers: readonly WorkerStatus[]): Counts {
  const counts = { queued: 0, running: 0, stalled: 0, done: 0, failed: 0, 'taken-back': 0 }
  for (const w of workers) counts[w.state] += 1
  return counts
}

// A worker holds a worktree slot while it runs or has stalled (not yet stopped).
export const slotsUsed = (counts: Counts): number => counts.running + counts.stalled

const COUNT_ORDER: readonly WorkerState[] = ['running', 'queued', 'stalled', 'failed', 'done', 'taken-back']

function countText(counts: Counts, ascii: boolean): string {
  const g = glyphs(ascii)
  const parts: string[] = []
  for (const s of COUNT_ORDER) {
    if (counts[s] === 0) continue
    const mark = s === 'stalled' ? ` ${g.warn}` : s === 'failed' ? ` ${g.fail}` : ''
    parts.push(`${counts[s]} ${s}${mark}`)
  }
  return parts.join(g.sep)
}

// The one line the pipeline pane adds (FR-018): `Workers: 2 running · 1 done · slots 1/3`, marked
// when a worker stalled or failed. Null with no workers, so the pane gains no row.
export function compactLine(snapshot: WorkersSnapshot | null, ascii = false): { text: string; tone: Tone } | null {
  if (snapshot === null || snapshot.workers.length === 0) return null
  const counts = countsOf(snapshot.workers)
  const over = Number.isInteger(snapshot.cap) && snapshot.cap >= 1 && slotsUsed(counts) > snapshot.cap
  const tone: Tone = counts.failed > 0 || over ? 'bad' : counts.stalled > 0 ? 'warn' : 'plain'
  return { text: `Workers: ${countText(counts, ascii)}${glyphs(ascii).sep}slots ${slotsUsed(counts)}/${snapshot.cap}`, tone }
}

// 45s, 12m, 1h 05m, 2d 3h.
export function duration(secs: number): string {
  const s = Math.max(0, Math.floor(secs))
  if (s < 60) return `${s}s`
  if (s < 3600) return `${Math.floor(s / 60)}m`
  if (s < 86_400) return `${Math.floor(s / 3600)}h ${String(Math.floor((s % 3600) / 60)).padStart(2, '0')}m`
  return `${Math.floor(s / 86_400)}d ${Math.floor((s % 86_400) / 3600)}h`
}

const compactNumber = (n: number): string => (n >= 1_000_000 ? `${(n / 1_000_000).toFixed(2)}M` : n >= 1000 ? `${Math.round(n / 1000)}k` : String(n))

// `marks` are ranges of `text` drawn in their own tone (the slots over the limit); the text itself is what
// `/workers` prints, so colour is never the only carrier of the meaning.
export type Mark = { from: number; to: number; tone: Tone }
export type Line = { key: string; text: string; tone: Tone; toggle?: string; buttonKey?: string; marks?: Mark[] }
export type Limit = { kind: string; percent: number }
export type Context = {
  columns: number
  // Milliseconds, the poll's last check: nothing here reads the clock.
  now: number
  ascii: boolean
  maxRows: number
  expanded: Readonly<Record<string, boolean>>
  limits?: readonly Limit[]
}
export type WorkersView = { isEmpty: boolean; counts: Counts; slots: { used: number; cap: number }; lines: Line[] }

// The expansion key of the History / Done disclosure. It cannot be a track name (TRACK has no '#').
export const HISTORY_KEY = '#history'

const pad = (s: string, n: number) => (s.length >= n ? s : s + ' '.repeat(n - s.length))
const fit = (s: string, n: number, ellipsis: string) => (s.length > n ? `${s.slice(0, Math.max(1, n - ellipsis.length))}${ellipsis}` : s)
// Left text, then right text flush to the width; the left is cut when both do not fit.
function spread(left: string, right: string, width: number, ellipsis: string): string {
  if (right === '') return fit(left, width, ellipsis)
  const room = width - right.length - 1
  if (room < 4) return fit(`${left} ${right}`, width, ellipsis)
  return `${pad(fit(left, room, ellipsis), room)} ${right}`
}
const cap1 = (s: string) => s.charAt(0).toUpperCase() + s.slice(1)

const RANK: Record<WorkerState, number> = { running: 0, stalled: 1, queued: 2, failed: 3, done: 4, 'taken-back': 5 }

export function sortWorkers(workers: readonly WorkerStatus[]): WorkerStatus[] {
  return [...workers].sort((a, b) => RANK[a.state] - RANK[b.state] || b.updated - a.updated || a.track.localeCompare(b.track))
}

function meters(limits: readonly Limit[] | undefined): string | null {
  const label: Record<string, string> = { five_hour: '5h', seven_day: 'week' }
  const parts = (limits ?? []).filter(l => label[l.kind] !== undefined).map(l => `${label[l.kind]} ${Math.round(l.percent)}%`)
  return parts.length > 0 ? parts.join(' · ') : null
}

// Cost lines for one track, from the delegation log.
function costLine(track: string, log: readonly WorkerLogLine[]): string | null {
  const runs = log.filter(l => l.track === track)
  if (runs.length === 0) return null
  const wall = runs.reduce((a, l) => a + l.wallSecs, 0)
  const tokens = runs.reduce((a, l) => a + (l.tokens ?? 0), 0)
  const reported = runs.some(l => l.tokens !== null)
  return `${runs.length} ${runs.length === 1 ? 'run' : 'runs'}, ${duration(wall)} wall${reported ? `, ${compactNumber(tokens)} tokens` : ''}`
}

const KIND_LABEL: Record<WorkerKind, string> = { subagent: 'sub (session subagent)', profile: 'prof (Claude profile)', agent: 'ext (code agent)' }
const needsAttention = (w: WorkerStatus) => w.state === 'stalled' || w.state === 'failed'
const isHistory = (w: WorkerStatus) => w.state === 'done' || w.state === 'taken-back'

// The slot meter: one cell per slot of a small positive cap, wider caps and narrow panes as text. The cap is the
// worker-slot pool (running plus stalled workers hold one); this is occupancy, never task progress, and a queued
// worker is not said to wait for a slot because nothing in the status says why it waits.
function capacity(used: number, cap: number, columns: number, ascii: boolean, ellipsis: string): Line[] {
  const known = Number.isInteger(cap) && cap >= 1
  if (!known) {
    return [{ key: 'cap', text: fit(`CAPACITY  ${used} in use (limit unknown)`, columns, ellipsis), tone: 'dim' }]
  }
  const over = used > cap
  const free = Math.max(0, cap - used)
  const tone: Tone = over ? 'bad' : used > 0 ? 'info' : 'dim'
  const figure = `${used} / ${cap} slots`
  const status = over ? `${used - cap} over the limit` : free === 0 ? 'full' : `${free} available`
  const compact = (): Line[] => [{ key: 'cap', text: spread('CAPACITY', `${used}/${cap} slots · ${status}`, columns, ellipsis), tone }]
  if (cap > 12 || columns < NARROW) return compact()
  // Past the cap the meter keeps growing: the slots beyond it are [!] cells in the error tone. Used cells = used when
  // over, so the meter must fit beside the status; otherwise the compact text, wholly in the error tone.
  const cells = Math.max(cap, used)
  if (cells > 12 || (over && 4 * used - 1 > columns - status.length - 1)) return compact()
  const on = ascii ? '[#]' : '[■]'
  const off = '[ ]'
  const meter = Array.from({ length: cells }, (_, i) => (i >= cap ? '[!]' : i < used ? on : off)).join(' ')
  const meterLine = spread(meter, status, columns, ellipsis)
  const marks: Mark[] = over
    ? [
        { from: 4 * cap, to: 4 * used - 1, tone: 'bad' },
        { from: meterLine.length - status.length, to: meterLine.length, tone: 'bad' },
      ]
    : []
  return [
    { key: 'cap', text: spread('CAPACITY', figure, columns, ellipsis), tone },
    { key: 'cap-meter', text: meterLine, tone: over ? 'info' : tone, ...(over ? { marks } : {}) },
  ]
}

export function build(snapshot: WorkersSnapshot | null, ctx: Context): WorkersView {
  const g = glyphs(ctx.ascii)
  const workers = snapshot?.workers ?? []
  const counts = countsOf(workers)
  const cap = snapshot?.cap ?? DEFAULT_CAP
  const slots = { used: slotsUsed(counts), cap }
  const lines: Line[] = []
  const width = Math.max(10, ctx.columns)
  const add = (text: string, tone: Tone = 'plain', toggle?: string, buttonKey?: string) =>
    lines.push({ key: `l${lines.length}`, text: fit(text, width, g.ellipsis), tone, toggle, buttonKey })
  // Wrapped, not cut: a detail line keeps the whole value on continuation lines.
  const detail = (label: string, value: string) => {
    let lead = `    ${label}: `
    let first = lead
    if (width - lead.length < 12) {
      add(`    ${label}:`, 'dim')
      lead = '      '
      first = lead
    }
    const room = Math.max(4, width - lead.length)
    const chars = [...value]
    for (let i = 0; i === 0 || i < chars.length; i += room) add(`${i === 0 ? first : ' '.repeat(lead.length)}${chars.slice(i, i + room).join('')}`, 'dim')
  }

  if (workers.length === 0) {
    add('No workers', 'dim')
    if (snapshot !== null && snapshot.skipped > 0) add(`${snapshot.skipped} status ${snapshot.skipped === 1 ? 'file' : 'files'} unreadable`, 'warn')
    return { isEmpty: true, counts, slots, lines }
  }

  // Overview.
  const issues = [counts.failed > 0 ? `${counts.failed} failed` : '', counts.stalled > 0 ? `${counts.stalled} stalled` : ''].filter(Boolean).join(g.sep)
  const tone: Tone = counts.failed > 0 ? 'bad' : counts.stalled > 0 ? 'warn' : 'bold'
  const head = Math.min(width, 72)
  add(spread('WORKERS', issues, head, g.ellipsis), tone)
  add(countText(counts, ctx.ascii), 'dim')
  const usage = meters(ctx.limits)
  if (usage !== null) add(`Usage  ${usage}`, 'dim')

  // Capacity.
  for (const l of capacity(slots.used, cap, head, ctx.ascii, g.ellipsis)) lines.push({ ...l, key: `l${lines.length}`, text: l.text })

  const tier = width < NARROW ? 'narrow' : width < WIDE ? 'medium' : 'wide'
  const nowS = ctx.now / 1000
  const sorted = sortWorkers(workers)
  const glyphOf = (w: WorkerStatus) => STATE_GLYPH[w.state][ctx.ascii ? 1 : 0]
  const stateOf = (w: WorkerStatus) => cap1(w.state.replace('-', ' '))
  const isLive = (w: WorkerStatus) => w.state === 'running' || w.state === 'stalled'
  const elapsedOf = (w: WorkerStatus) => (w.state === 'queued' ? g.none : duration((isLive(w) ? nowS : w.updated) - w.started))
  const tasksOf = (w: WorkerStatus) => (w.tasks.length === 0 ? g.none : w.tasks.join(', '))
  const nameOf = (w: WorkerStatus) => `${w.track}${w.note === null ? '' : `: ${w.note}`}`

  // Attention: failed and stalled only, from the same workers. A press opens the worker's details.
  const attention = sorted.filter(needsAttention)
  if (attention.length > 0) {
    lines.push({ key: `l${lines.length}`, text: '', tone: 'plain' })
    add(`ATTENTION${g.sep}${attention.length} ${attention.length === 1 ? 'issue' : 'issues'}`, attention.some(w => w.state === 'failed') ? 'bad' : 'warn')
    for (const w of attention) {
      const isOpen = ctx.expanded[w.track] === true
      const verdictNote = w.verdict.result === 'rejected' ? `${g.sep}verdict rejected` : ''
      add(`${isOpen ? g.open : g.closed} ${glyphOf(w)} ${fit(w.track, 24, g.ellipsis)} ${w.state}${g.sep}${tasksOf(w)}${g.sep}${elapsedOf(w)}${verdictNote}`, STATE_TONE[w.state], w.track, `attention:${w.track}`)
      if (isOpen) details(w)
    }
  }

  function details(w: WorkerStatus) {
    detail('Track', nameOf(w))
    if (w.tasks.length > 0) detail('Tasks', w.tasks.join(', '))
    detail('Role', `${w.role}${g.sep}Agent: ${KIND_LABEL[w.kind]} / ${w.agent}`)
    detail('State', `${stateOf(w)}${g.sep}Elapsed: ${elapsedOf(w)}`)
    const reason = w.verdict.reason === null ? '' : ` (${w.verdict.reason})`
    detail('Verdict', `${w.verdict.result}${reason}`)
    if (w.kind !== 'subagent') detail('Last event', `${w.events} ${w.events === 1 ? 'event' : 'events'}${isLive(w) ? `, last update ${duration(nowS - w.updated)} ago` : ''}`)
    if (w.worktree !== '') detail('Worktree', w.worktree)
    if (w.tokens !== null) detail('Tokens', compactNumber(w.tokens))
    const cost = costLine(w.track, snapshot?.log ?? [])
    if (cost !== null) detail('Log', cost)
  }

  // Workers.
  const active = sorted.filter(w => !isHistory(w))
  const history = sorted.filter(isHistory)
  const budget = Math.max(1, ctx.maxRows)
  const row = (w: WorkerStatus) => {
    const isOpen = ctx.expanded[w.track] === true
    const mark = isOpen ? g.open : g.closed
    const t = STATE_TONE[w.state]
    if (tier === 'wide') {
      const nameW = Math.min(36, Math.max(14, ...[...active, ...history].map(x => nameOf(x).length)))
      add(`${mark} ${glyphOf(w)} ${pad(fit(nameOf(w), nameW, g.ellipsis), nameW)}  ${pad(KIND_TAG[w.kind], 4)}  ${pad(stateOf(w), 10)}  ${pad(fit(tasksOf(w), 22, g.ellipsis), 22)}  ${elapsedOf(w)}`.trimEnd(), t, w.track)
    } else {
      add(`${mark} ${glyphOf(w)} ${spread(nameOf(w), stateOf(w), width - 4, g.ellipsis)}`, t, w.track)
      const extra = tier === 'medium' ? `${g.sep}${w.role}${g.sep}${KIND_TAG[w.kind]}${g.sep}verdict ${w.verdict.result}` : ''
      add(`    ${tasksOf(w)}${g.sep}${elapsedOf(w)}${tier === 'narrow' ? `${g.sep}${KIND_TAG[w.kind]}` : ''}${extra}`, 'dim')
    }
    if (isOpen) details(w)
  }
  lines.push({ key: `l${lines.length}`, text: '', tone: 'plain' })
  if (tier === 'wide') add(`  ${' '.repeat(2)}${pad('WORKER / TRACK', Math.min(36, Math.max(14, ...[...active, ...history].map(x => nameOf(x).length))))}  ${pad('KIND', 4)}  ${pad('STATE', 10)}  ${pad('TASKS', 22)}  TIME`, 'dim')
  else add('WORKERS', 'dim')
  const shownActive = active.slice(0, budget)
  if (active.length === 0) add('No active workers', 'dim')
  for (const w of shownActive) row(w)
  let hidden = active.length - shownActive.length
  if (history.length > 0) {
    const open = ctx.expanded[HISTORY_KEY] === true
    const kinds = [counts.done > 0 ? `${counts.done} done` : '', counts['taken-back'] > 0 ? `${counts['taken-back']} taken back` : ''].filter(Boolean).join(g.sep)
    add(`${open ? g.open : g.closed} History${g.sep}${kinds}`, 'dim', HISTORY_KEY)
    if (open) {
      const room = Math.max(0, budget - shownActive.length)
      for (const w of history.slice(0, room)) row(w)
      hidden += Math.max(0, history.length - room)
    }
  }
  if (hidden > 0) add(`+${hidden} more`, 'dim')
  if (snapshot !== null && snapshot.skipped > 0) add(`${snapshot.skipped} status ${snapshot.skipped === 1 ? 'file' : 'files'} unreadable`, 'warn')
  return { isEmpty: false, counts, slots, lines }
}

// What `/workers` prints: every row opened, history included, so the text carries the detail a click would.
export function text(snapshot: WorkersSnapshot | null, ctx: Omit<Context, 'expanded'>): string {
  const expanded: Record<string, boolean> = Object.fromEntries((snapshot?.workers ?? []).map(w => [w.track, true]))
  expanded[HISTORY_KEY] = true
  return build(snapshot, { ...ctx, expanded })
    .lines.map(l => l.text.trimEnd())
    .join('\n')
}
