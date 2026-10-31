// What the token status shows, from the orchestration ledger and `token_report.py
// budget --json`. Pure: no `$`, so the hooks and the tests share it as it is.
// token_report.py stays the only thing that measures or judges a budget; this
// reads figures the ledger and the script already wrote, and formats them. It
// computes no token count of its own.

// The ledger's state words (agents:orchestrate, "Ledger"): a card in one of
// these is still in flight; `landed` is done.
export const IN_FLIGHT = new Set(['queued', 'running', 'review'])

export type LedgerCard = {
  id: string
  state: string
  // The Tokens cell's two halves as written ("~420k", "391k"), or '—'.
  projectedText: string
  usedText: string
  projected: number | null
  used: number | null
  // The optional Branch and Worktree columns (specs/020-card-timeline-pane); only set when the column and a value exist.
  branch?: string
  worktree?: string
}

// "~420k", "391k", "1.24M" → a number; "—" or nothing → null.
export function parseTok(text: string): number | null {
  const hit = /^~?\s*([\d.]+)\s*([kKmM]?)\s*$/.exec(text.trim())
  if (!hit) return null
  const n = Number(hit[1])
  if (!Number.isFinite(n)) return null
  const unit = (hit[2] ?? '').toLowerCase()
  return unit === 'k' ? n * 1_000 : unit === 'm' ? n * 1_000_000 : n
}

// token_report.py's compact form: 391k, 1.24M.
export function fmtTok(n: number): string {
  return n < 1_000_000 ? `${Math.round(n / 1000)}k` : `${(n / 1_000_000).toFixed(2)}M`
}

// `7.40M/h`: the script's per-hour figure in the same compact form.
export function fmtRate(perHour: number): string {
  return `${fmtTok(perHour)}/h`
}

// The script's `eta_minutes` as `~25m`, `~2h10m`, `~15h`, or `>2d` past 48 hours.
export function fmtEta(minutes: number): string {
  if (minutes > 48 * 60) return '>2d'
  if (minutes < 60) return `~${minutes}m`
  const hours = Math.floor(minutes / 60)
  const rest = minutes % 60
  return rest === 0 ? `~${hours}h` : `~${hours}h${rest}m`
}

const cells = (line: string): string[] => line.trim().replace(/^\||\|$/g, '').split('|').map(cell => cell.trim())

// The card table of .ai-gent/orchestrate-status.md: a `Card` column (the id in
// backticks, as token_report.py reads it), a `State` column, a `Tokens` column.
export function parseLedger(text: string): LedgerCard[] {
  const cards: LedgerCard[] = []
  let header: string[] | null = null
  let isBody = false
  for (const line of text.split(/\r?\n/)) {
    if (!line.startsWith('|')) {
      header = null
      isBody = false
      continue
    }
    const row = cells(line)
    if (header === null) {
      header = row.map(cell => cell.toLowerCase())
      continue
    }
    if (!isBody) {
      isBody = true // the separator row
      continue
    }
    const at = (name: string): number => header?.findIndex(h => h === name) ?? -1
    if (at('card') < 0) continue
    const id = row.find(cell => cell.startsWith('`') && cell.endsWith('`') && cell.length > 2)?.slice(1, -1)
    if (!id) continue
    const stateCell = row[Math.max(at('state'), at('status'))] ?? ''
    const [projectedText = '—', usedText = '—'] = (row[at('tokens')] ?? '').split('/').map(part => part.trim() || '—')
    const optional = (name: string): string | undefined => {
      const cell = (row[at(name)] ?? '').replace(/`/g, '').trim()
      return cell === '' || cell === '—' ? undefined : cell
    }
    const branch = optional('branch')
    const worktree = optional('worktree')
    cards.push({
      id,
      state: stateCell.toLowerCase().replace(/[*_`]/g, '').trim().split(/\s+/)[0] ?? '',
      projectedText,
      usedText,
      projected: parseTok(projectedText),
      used: parseTok(usedText),
      ...(branch !== undefined ? { branch } : {}),
      ...(worktree !== undefined ? { worktree } : {}),
    })
  }
  return cards
}

export function inFlight(cards: readonly LedgerCard[]): LedgerCard[] {
  return cards.filter(card => IN_FLIGHT.has(card.state))
}

// `token_report.py budget --json`.
export type Budget = {
  verdict: 'proceed' | 'shrink' | 'stop'
  why: string
  fit: number
  // `rate` and `eta_minutes` are the burn rate's additions (spec 018): the script measures, this formats.
  rate?: { per_hour: number; window_minutes: number; messages: number } | null
  caps: { name: string; cap: number; used: number; left: number; eta_minutes?: number | null }[]
  cards: { label: string; model: string; projected: number; p75: number | null; confidence: string | null }[]
  batches: string[][]
}

export function parseBudget(stdout: string): Budget | null {
  try {
    const data: unknown = JSON.parse(stdout)
    const budget = data as Budget
    return typeof budget?.verdict === 'string' && Array.isArray(budget.caps) ? budget : null
  } catch {
    return null
  }
}

// The status line, cut to `width`: the budget part and the warning are kept,
// the cards give way (`+N more`).
export function statusText(cards: readonly LedgerCard[], budget: Budget | null, width: number, withEta = true): string | null {
  const eta = (cap: Budget['caps'][number]): string => (withEta && typeof cap.eta_minutes === 'number' ? ` (${fmtEta(cap.eta_minutes)})` : '')
  const caps = budget ? budget.caps.map(cap => `${cap.name} left ${fmtTok(cap.left)}${eta(cap)}`) : []
  const warn = budget && budget.verdict !== 'proceed' ? [`⚠ ${budget.verdict}`] : []
  if (cards.length === 0 && caps.length === 0) return null

  const tail = [...caps, ...warn]
  const reserved = ['tokens', ...tail].join(' · ').length + 12
  const shown: string[] = []
  let used = reserved
  for (const card of cards) {
    const part = `${card.id} ${card.projectedText}/${card.usedText}`
    if (used + part.length + 3 > width && shown.length > 0) break
    shown.push(part)
    used += part.length + 3
  }
  const more = cards.length - shown.length
  return ['tokens', ...shown, ...(more > 0 ? [`+${more} more`] : []), ...tail].join(' · ')
}

export type Toasts = { verdict: string | null; overruns: { id: string; text: string }[] }

// What is new since the last look: a verdict that turned to shrink or stop, and
// each card whose used figure passed its projection (once per card).
export function news(cards: readonly LedgerCard[], budget: Budget | null, lastVerdict: string | null, already: readonly string[]): Toasts {
  const verdict = budget && budget.verdict !== lastVerdict && budget.verdict !== 'proceed' ? `budget: ${budget.verdict} — ${budget.why}` : null
  const overruns = cards
    .filter(card => card.projected !== null && card.used !== null && card.used > card.projected && !already.includes(card.id))
    .map(card => ({
      id: card.id,
      text: `${card.id} used ${card.usedText}, projected ${card.projectedText} (+${Math.round(((card.used as number) / (card.projected as number) - 1) * 100)}%)`,
    }))
  return { verdict, overruns }
}

// The caps whose ETA fell under `threshold` minutes since the last look: one toast
// per cap per crossing. `warned` is the caps already under; the new list is every
// cap under now (a cap with nothing left is 014's stop, not an ETA). 0 turns it off.
export function etaNews(budget: Budget | null, warned: readonly string[], threshold: number): { toasts: string[]; warned: string[] } {
  if (!budget || threshold <= 0) return { toasts: [], warned: [] }
  const under = budget.caps.filter(cap => cap.left > 0 && typeof cap.eta_minutes === 'number' && cap.eta_minutes < threshold)
  const rate = budget.rate ? ` at ${fmtRate(budget.rate.per_hour)}` : ''
  const toasts = under
    .filter(cap => !warned.includes(cap.name))
    .map(cap => `${cap.name} cap lasts ${fmtEta(cap.eta_minutes as number)}${rate} — pause the wave or finish the card in flight`)
  return { toasts, warned: under.map(cap => cap.name) }
}
