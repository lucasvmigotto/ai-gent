// The /guards list and the pane's text, pure: the rows in, lines out. Counts
// only (spec 024 FR-006): nothing here calls a guard strict or suggests a change.

import type { GuardRow } from '../types'

export const DEFAULT_LIMIT = 20
export const MAX_LIMIT = 200

// The guard ids and plugins, in the order the header lists absent ones.
export const PLUGINS = ['git', 'db', 'secrets', 'repo', 'devcontainer'] as const
export type PluginName = (typeof PLUGINS)[number]

const LABEL: Record<GuardRow['guard'], string> = {
  git: 'git:workflow',
  db: 'db guard',
  secrets: 'secrets guard',
  repo: 'repo guard',
  containers: 'containers guard',
}

const GUARDS = new Set(Object.keys(LABEL))

export type Frequent = { entry: string; guard: GuardRow['guard']; decision: GuardRow['decision']; count: number; verbs: { verb: string; count: number }[] }
export type Summary = { total: number; deny: number; ask: number; newest: GuardRow[]; frequent: Frequent[] }

// Keeps only what has a row's shape, so a stale or odd value never draws.
export function parseRows(value: unknown): GuardRow[] {
  if (!Array.isArray(value)) return []
  return value.filter(
    (r): r is GuardRow =>
      typeof r === 'object' && r !== null &&
      typeof r.at === 'number' && GUARDS.has(r.guard) && (r.decision === 'deny' || r.decision === 'ask') &&
      typeof r.entry === 'string' && typeof r.tool === 'string' && (r.verb === null || typeof r.verb === 'string'),
  )
}

export function clampLimit(value: unknown, fallback = DEFAULT_LIMIT): number {
  const n = typeof value === 'number' ? value : Number.parseInt(String(value ?? ''), 10)
  const base = Number.isFinite(n) ? Math.floor(n) : fallback
  return Math.max(1, Math.min(MAX_LIMIT, base))
}

export function summarize(rows: readonly GuardRow[], n: number): Summary {
  const sorted = [...rows].sort((a, b) => b.at - a.at)
  const groups = new Map<string, Frequent>()
  for (const r of sorted) {
    const key = `${r.guard}\u0000${r.decision}\u0000${r.entry}`
    const g = groups.get(key) ?? { entry: r.entry, guard: r.guard, decision: r.decision, count: 0, verbs: [] }
    g.count += 1
    if (r.verb !== null) {
      const v = g.verbs.find(x => x.verb === r.verb)
      if (v) v.count += 1
      else g.verbs.push({ verb: r.verb, count: 1 })
    }
    groups.set(key, g)
  }
  const frequent = [...groups.values()]
    .map(g => ({ ...g, verbs: [...g.verbs].sort((a, b) => b.count - a.count).slice(0, 3) }))
    .sort((a, b) => b.count - a.count)
    .slice(0, 5)
  const deny = rows.filter(r => r.decision === 'deny').length
  return { total: rows.length, deny, ask: rows.length - deny, newest: sorted.slice(0, clampLimit(n)), frequent }
}

function clock(at: number): string {
  const d = new Date(at)
  return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`
}

const pad = (s: string, n: number) => (s.length >= n ? s : s + ' '.repeat(n - s.length))

// The text: header, rows, "Most frequent". `width` cuts only the reason column.
export function lines(summary: Summary, notReporting: readonly string[], width = Number.POSITIVE_INFINITY): string[] {
  const missing = notReporting.length > 0 ? ` · not reporting: ${notReporting.join(', ')}` : ''
  if (summary.total === 0) {
    return [`no guard decisions this session${missing}`]
  }
  const more = summary.newest.length < summary.total ? ` (/guards ${Math.min(MAX_LIMIT, summary.total)} shows more)` : ''
  const out = [`guards — ${summary.total} decisions this session (${summary.deny} deny · ${summary.ask} ask) · newest ${summary.newest.length}${more}${missing}`]
  for (const r of summary.newest) {
    const head = ` ${clock(r.at)}  ${pad(r.decision, 4)}  ${pad(r.guard, 10)} ${pad(r.verb ?? r.tool, 18)} `
    const room = width - head.length
    const reason = room >= r.entry.length ? r.entry : `${r.entry.slice(0, Math.max(1, room - 1))}…`
    out.push(head + reason)
  }
  out.push('Most frequent')
  for (const f of summary.frequent) {
    const verbs = f.verbs.length > 0 ? `  (${f.verbs.map(v => `${v.verb} ×${v.count}`).join(', ')})` : ''
    out.push(` ${pad(`${f.count}×`, 4)}${pad(LABEL[f.guard], 17)} ${pad(f.entry, 32)} ${f.decision}${verbs}`)
  }
  return out
}
