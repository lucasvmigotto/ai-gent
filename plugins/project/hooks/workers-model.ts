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
  queued: ['·', '.'],
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
  const tone: Tone = counts.failed > 0 ? 'bad' : counts.stalled > 0 ? 'warn' : 'plain'
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

export type Line = { key: string; text: string; tone: Tone; toggle?: string }
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

const pad = (s: string, n: number) => (s.length >= n ? s : s + ' '.repeat(n - s.length))
const fit = (s: string, n: number, ellipsis: string) => (s.length > n ? `${s.slice(0, Math.max(1, n - ellipsis.length))}${ellipsis}` : s)

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
  return `log: ${runs.length} ${runs.length === 1 ? 'run' : 'runs'}, ${duration(wall)} wall${reported ? `, ${compactNumber(tokens)} tokens` : ''}`
}

export function build(snapshot: WorkersSnapshot | null, ctx: Context): WorkersView {
  const g = glyphs(ctx.ascii)
  const workers = snapshot?.workers ?? []
  const counts = countsOf(workers)
  const cap = snapshot?.cap ?? DEFAULT_CAP
  const slots = { used: slotsUsed(counts), cap }
  const lines: Line[] = []
  const add = (text: string, tone: Tone = 'plain', toggle?: string) => lines.push({ key: `l${lines.length}`, text: fit(text, ctx.columns, g.ellipsis), tone, toggle })

  if (workers.length === 0) {
    add('No workers', 'dim')
    if (snapshot !== null && snapshot.skipped > 0) add(`${snapshot.skipped} status ${snapshot.skipped === 1 ? 'file' : 'files'} unreadable`, 'warn')
    return { isEmpty: true, counts, slots, lines }
  }

  const tone: Tone = counts.failed > 0 ? 'bad' : counts.stalled > 0 ? 'warn' : 'bold'
  add(`Workers  slots ${slots.used}/${cap}${g.sep}${countText(counts, ctx.ascii)}`, tone)
  const usage = meters(ctx.limits)
  if (usage !== null) add(`Usage  ${usage}`, 'dim')

  const tier = ctx.columns < NARROW ? 'narrow' : ctx.columns < WIDE ? 'medium' : 'wide'
  const sorted = sortWorkers(workers)
  const shown = sorted.slice(0, Math.max(1, ctx.maxRows))
  // The kind tag shows once a row is not a plain status-file agent, so a pane of only those is unchanged.
  const tagged = workers.some(w => w.kind !== 'agent')
  const tag = (w: WorkerStatus) => (tagged ? `${pad(KIND_TAG[w.kind], 4)} ` : '')
  const labelOf = (w: WorkerStatus) => `${w.tasks.join(',')} ${w.track}${w.note === null ? '' : `: ${w.note}`}`.trim()
  const flex = Math.max(12, ...shown.map(w => labelOf(w).length))
  const labelWidth = Math.min(flex, tier === 'wide' ? 40 : 28)

  if (tier !== 'narrow') {
    add(`  ${pad('STATE', 12)} ${tagged ? 'KIND ' : ''}${pad('AGENT', 16)} ${pad('ROLE', 10)} ${pad('TASKS TRACK', labelWidth)} ${pad('ELAPSED', 7)} LAST EVENT${tier === 'wide' ? '   VERDICT      WORKTREE' : ''}`, 'dim')
  }
  for (const w of shown) {
    const glyph = STATE_GLYPH[w.state][ctx.ascii ? 1 : 0]
    const isOpen = ctx.expanded[w.track] === true
    const isLive = w.state === 'running' || w.state === 'stalled'
    const nowS = ctx.now / 1000
    const elapsed = w.state === 'queued' ? g.none : duration((isLive ? nowS : w.updated) - w.started)
    const age = isLive && w.kind !== 'subagent' ? `${duration(nowS - w.updated)} ago` : g.none
    const label = labelOf(w)
    const mark = isOpen ? g.open : g.closed
    if (tier === 'narrow') {
      add(`${mark} ${glyph} ${tagged ? `${KIND_TAG[w.kind]} ` : ''}${w.agent} ${w.track} ${elapsed}`, STATE_TONE[w.state], w.track)
    } else {
      const verdict = `${VERDICT_GLYPH[w.verdict.result]?.[ctx.ascii ? 1 : 0] ?? ''} ${w.verdict.result}`
      const tail = tier === 'wide' ? `   ${pad(verdict, 12)} ${w.worktree === '' ? g.none : w.worktree}` : ''
      add(`${mark} ${pad(`${glyph} ${w.state}`, 12)} ${tag(w)}${pad(fit(w.agent, 16, g.ellipsis), 16)} ${pad(w.role, 10)} ${pad(fit(label, labelWidth, g.ellipsis), labelWidth)} ${pad(elapsed, 7)} ${pad(age, 10)}${tail}`.trimEnd(), STATE_TONE[w.state], w.track)
    }
    if (isOpen) {
      const reason = w.verdict.reason === null ? '' : ` (${w.verdict.reason})`
      add(`    verdict: ${w.verdict.result}${reason}${g.sep}${w.events} events${w.tokens !== null ? `${g.sep}${compactNumber(w.tokens)} tokens` : ''}`, 'dim')
      add(`    ${w.role}${g.sep}${w.worktree === '' ? g.none : w.worktree}`, 'dim')
      const cost = costLine(w.track, snapshot?.log ?? [])
      if (cost !== null) add(`    ${cost}`, 'dim')
    }
  }
  if (sorted.length > shown.length) add(`+${sorted.length - shown.length} more`, 'dim')
  if (snapshot !== null && snapshot.skipped > 0) add(`${snapshot.skipped} status ${snapshot.skipped === 1 ? 'file' : 'files'} unreadable`, 'warn')
  return { isEmpty: false, counts, slots, lines }
}

// What `/workers` prints: every row opened, so the text carries the detail a click would.
export function text(snapshot: WorkersSnapshot | null, ctx: Omit<Context, 'expanded'>): string {
  const expanded = Object.fromEntries((snapshot?.workers ?? []).map(w => [w.track, true]))
  return build(snapshot, { ...ctx, expanded })
    .lines.map(l => l.text)
    .join('\n')
}
