// The card timeline pane's logic (specs/020-card-timeline-pane). Pure: no `$`, so
// the hooks and the tests share it. It derives only counts, state transitions,
// interval overlap and the axis scale; every token figure is a ledger cell or a
// field of `token_report.py tasks --dry-run --json`.

import type { LedgerCard } from './token-model'

export const LOG_MAX = 200
const ORDER = ['running', 'review', 'queued', 'landed'] as const

export type Change = { id: string; from: string; to: string }
export type LogEntry = Change & { at: string }

// One card's span, from the script's JSON (a card with several records is one).
export type Span = {
  card: string
  startMs: number
  endMs: number
  used: number
  projected: number
  agents: number
  calls: number
}

// `token_report.py tasks` needs the project's slug: the cwd with every character
// outside A-Za-z0-9- turned into `-`, as cmd_budget derives it.
export const slugOf = (cwd: string): string => cwd.replace(/[^A-Za-z0-9-]/g, '-')

// `5 cards · 2 running · 1 review · 1 queued · 1 landed`; a state outside the four counts as other.
export function headerText(cards: readonly LedgerCard[]): string {
  const counts = new Map<string, number>()
  for (const card of cards) {
    const key = (ORDER as readonly string[]).includes(card.state) ? card.state : 'other'
    counts.set(key, (counts.get(key) ?? 0) + 1)
  }
  const parts = [...ORDER, 'other'].flatMap(key => (counts.get(key) ? [`${counts.get(key)} ${key}`] : []))
  return [`${cards.length} ${cards.length === 1 ? 'card' : 'cards'}`, ...parts].join(' · ')
}

// What differs from the states last read. The first read (nothing seen) invents
// nothing; a card new to the ledger comes from `—`.
export function transitions(cards: readonly LedgerCard[], seen: Readonly<Record<string, string>>): { changes: Change[]; seen: Record<string, string> } {
  const isFirst = Object.keys(seen).length === 0
  const changes: Change[] = []
  const next: Record<string, string> = {}
  for (const card of cards) {
    next[card.id] = card.state
    if (isFirst) continue
    const before = seen[card.id] ?? '—'
    if (before !== card.state) changes.push({ id: card.id, from: before, to: card.state })
  }
  return { changes, seen: next }
}

export function appendLog(log: readonly LogEntry[], changes: readonly Change[], at: string): LogEntry[] {
  return [...log, ...changes.map(change => ({ ...change, at }))].slice(-LOG_MAX)
}

// The viewer's local HH:MM.
export function hhmm(ms: number): string {
  const d = new Date(ms)
  return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`
}

// `tasks --dry-run --json` to one span per card (earliest start to latest end), or null if it is not that.
export function parseMeasure(stdout: string): Span[] | null {
  let json: unknown
  try {
    json = JSON.parse(stdout)
  } catch {
    return null
  }
  const records = (json as { cards?: unknown } | null)?.cards
  if (!Array.isArray(records)) return null
  const spans = new Map<string, Span>()
  for (const raw of records) {
    const r = raw as Record<string, unknown>
    const startMs = Date.parse(String(r.start))
    const endMs = Date.parse(String(r.end))
    if (typeof r.card !== 'string' || Number.isNaN(startMs) || Number.isNaN(endMs)) continue
    const num = (v: unknown): number => (typeof v === 'number' && Number.isFinite(v) ? v : 0)
    const known = spans.get(r.card)
    // Token figures are the script's, never summed here: a second record of a card (a fix round) brings its own.
    const isLater = known === undefined || endMs >= known.endMs
    spans.set(r.card, {
      card: r.card,
      startMs: Math.min(known?.startMs ?? startMs, startMs),
      endMs: Math.max(known?.endMs ?? endMs, endMs),
      used: isLater ? num(r.used) : known.used,
      projected: isLater ? num(r.projected) : known.projected,
      agents: isLater ? num(r.agents) : known.agents,
      calls: isLater ? num(r.calls) : known.calls,
    })
  }
  return [...spans.values()]
}

const overlaps = (a: Span, b: Span): boolean => a.startMs < b.endMs && b.startMs < a.endMs

// The most spans open at once; a span that ends as another starts does not overlap it.
export function peak(spans: readonly Span[]): number {
  const edges = spans.flatMap(s => [[s.startMs, 1] as const, [s.endMs, -1] as const])
  edges.sort((a, b) => a[0] - b[0] || a[1] - b[1])
  let open = 0
  let most = 0
  for (const [, delta] of edges) {
    open += delta
    most = Math.max(most, open)
  }
  return most
}

export function overlapsOf(spans: readonly Span[]): Record<string, string[]> {
  return Object.fromEntries(spans.map(s => [s.card, spans.filter(o => o.card !== s.card && overlaps(s, o)).map(o => o.card)]))
}

export type Bar = { card: string; col: number; width: number }

// Each span as a column and a width on a `columns`-wide axis from the earliest start to the latest end; at least one cell.
export function axis(spans: readonly Span[], columns: number): { startMs: number; endMs: number; bars: Bar[] } {
  const startMs = Math.min(...spans.map(s => s.startMs))
  const endMs = Math.max(...spans.map(s => s.endMs))
  const total = endMs - startMs
  const cols = Math.max(1, columns)
  const bars = spans.map(s => {
    const col = total === 0 ? 0 : Math.min(cols - 1, Math.floor(((s.startMs - startMs) / total) * cols))
    const width = total === 0 ? 1 : Math.max(1, Math.round(((s.endMs - s.startMs) / total) * cols))
    return { card: s.card, col, width: Math.min(width, cols - col) }
  })
  return { startMs, endMs, bars }
}
