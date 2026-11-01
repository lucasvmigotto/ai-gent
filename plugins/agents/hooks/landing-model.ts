// What the landing toast says (specs/019-card-cost-on-landing). Pure: no `$`, so
// the hooks and the tests share it as it is. Every figure shown is a cell of the
// ledger's Tokens column as written; the only derived value is the percentage,
// computed from the cells' parsed (already rounded) values, half to even like
// Python's `{:+.0f}`. token_report.py's `pct_error` fed the same values gives the
// same text (pct-fixtures.ts is the shared table); tracker.md's Error column is
// computed from exact figures, so it can differ from this by one point.

import type { LedgerCard } from './token-model'
import { IN_FLIGHT } from './token-model'

export type Wave = { landed: number; over: number; under: number }
export const NO_WAVE: Wave = { landed: 0, over: 0, under: 0 }

export type Landing = {
  id: string
  projectedText: string
  usedText: string
  // The signed whole percent, or null with no projection (missing or zero).
  percent: number | null
  // The same, as pct_error writes it ('+31%'), or null.
  percentText: string | null
  hint: 'over' | 'under' | null
  // used against projected on the parsed cells, for the wave's counts.
  side: 'over' | 'under' | null
}

// `pct_error`'s text: '+31%', '-7%', '-0%' (the sign follows the exact ratio).
export function pctError(projected: number, used: number): string | null {
  if (!projected) return null
  const raw = ((used - projected) / projected) * 100
  const abs = Math.abs(raw)
  const floor = Math.floor(abs)
  const frac = abs - floor // exact for a double
  const n = frac > 0.5 ? floor + 1 : frac < 0.5 ? floor : floor % 2 === 0 ? floor : floor + 1
  return `${raw < 0 ? '-' : '+'}${n}%`
}

export function errorPercent(projected: number, used: number): number | null {
  const text = pctError(projected, used)
  return text === null ? null : Number(text.slice(0, -1))
}

export type Seen = Record<string, string>

// The cards newly `landed` with a used figure. `seen` (card → state last read),
// `toasted` and `firstLook` are the three atoms; the new values come back. The
// first read of a session (nothing seen yet) records the landed cards that
// already have their figure and toasts none. A card that leaves `landed` leaves
// both lists, so its next landing toasts again; a row that vanishes is forgotten.
export function landedNews(
  cards: readonly LedgerCard[],
  seen: Seen,
  toasted: readonly string[],
  firstLook: readonly string[],
  hintPercent = 25,
): { news: Landing[]; seen: Seen; toasted: string[]; firstLook: string[] } {
  const isFirstRead = Object.keys(seen).length === 0
  const nextSeen: Seen = {}
  const nextToasted: string[] = []
  const nextFirst: string[] = []
  const news: Landing[] = []
  for (const card of cards) {
    nextSeen[card.id] = card.state
    if (card.state !== 'landed') continue
    if (firstLook.includes(card.id)) {
      nextFirst.push(card.id)
      continue
    }
    if (toasted.includes(card.id)) {
      nextToasted.push(card.id)
      continue
    }
    if (card.used === null) continue // the figure comes with `tasks`
    if (isFirstRead) {
      nextFirst.push(card.id)
      continue
    }
    nextToasted.push(card.id)
    const hasProjection = card.projected !== null && card.projected > 0
    const percentText = hasProjection ? pctError(card.projected as number, card.used) : null
    const percent = percentText === null ? null : Number(percentText.slice(0, -1))
    const big = percent !== null && hintPercent > 0 && Math.abs(percent) >= hintPercent
    news.push({
      id: card.id,
      projectedText: card.projectedText,
      usedText: card.usedText,
      percent,
      percentText,
      hint: big ? (percent > 0 ? 'over' : 'under') : null,
      side: !hasProjection ? null : card.used > (card.projected as number) ? 'over' : card.used < (card.projected as number) ? 'under' : null,
    })
  }
  return { news, seen: nextSeen, toasted: nextToasted, firstLook: nextFirst }
}

const HINTS = {
  over: ' — wrong tags or effort? see token-economy',
  under: ' — projection too high; budget shrank for nothing',
}

export function landingText(n: Landing): string {
  if (n.percentText === null) return `${n.id} landed: used ${n.usedText} (no projection)`
  return `${n.id} landed: used ${n.usedText} vs ${n.projectedText} projected (${n.percentText})${n.hint ? HINTS[n.hint] : ''}`
}

export function addToWave(wave: Wave, news: readonly Landing[]): Wave {
  return {
    landed: wave.landed + news.length,
    over: wave.over + news.filter(n => n.side === 'over').length,
    under: wave.under + news.filter(n => n.side === 'under').length,
  }
}

// Due when nothing is in flight, a card landed this session, and no landed card
// with a projection still waits for its used figure (its toast would land in a
// wave of its own).
export function waveReady(cards: readonly LedgerCard[], wave: Wave): boolean {
  if (cards.length === 0 || wave.landed === 0) return false
  if (cards.some(card => IN_FLIGHT.has(card.state))) return false
  return !cards.some(card => card.state === 'landed' && card.used === null && card.projected !== null)
}

export function waveSummary(wave: Wave): string {
  return `wave landed: ${wave.landed} card${wave.landed === 1 ? '' : 's'}, ${wave.over} over, ${wave.under} under projection · /tokens for the budget`
}
