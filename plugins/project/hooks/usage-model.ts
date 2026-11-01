// The usage-limit meter's wording and levels, pure (specs/031-session-chrome): the account's
// rate-limit windows (`$.session.usage().rateLimits`, pushed by `session.measure`). The five-hour
// window is the bar on the prompt's hint line (`◷ ██████░░ 41%`, limitMeter); the context and the
// week (seven_day, spend_limit) are one row in the workspace line's Details, with the critical ones
// also in its warnings (extrasOf). A window with no reading draws nothing. The marks reuse the
// nudge's cap option: critical from 100 minus `handoffNudgeCapPercent` (90 at the default 10),
// approaching from 75. usage.ts feeds it.

import { MAX_BAR, MIN_BAR, bar, glyphs, levelOf, parsePercent } from './context-model'
import type { Level } from './context-model'

export const DEFAULT_CAP = 10
// A window turns approaching from this used share.
export const APPROACH_AT = 75
// The bars in the Details row are this wide at most.
const ROW_BAR = 8

export type Kind = 'five_hour' | 'seven_day' | 'spend_limit'
export type Limit = { kind: Kind; percent: number; resetsAt?: string }

const KINDS: readonly string[] = ['five_hour', 'seven_day', 'spend_limit']

// The cap from the options (`handoffNudgeCapPercent`): a number from 1 to 99, else the default.
export function capOf(value: unknown): number {
  return typeof value === 'number' && Number.isFinite(value) && value >= 1 && value < 100 ? value : DEFAULT_CAP
}

export function limitLevel(percent: number, cap: number): Level {
  if (percent >= 100 - cap) return 'critical'
  return percent >= APPROACH_AT ? 'approaching' : 'ok'
}

// The engine's `rateLimits` as the windows this meter draws: the three known kinds, a finite
// percent (floored at 0, one decimal, past 100 kept), the last of a kind winning. Anything else is dropped.
export function parseLimits(raw: unknown): Limit[] {
  if (!Array.isArray(raw)) return []
  const found = new Map<Kind, Limit>()
  for (const item of raw as unknown[]) {
    if (typeof item !== 'object' || item === null) continue
    const { kind, percentUsed, resetsAt } = item as Record<string, unknown>
    if (typeof kind !== 'string' || !KINDS.includes(kind)) continue
    if (typeof percentUsed !== 'number' || !Number.isFinite(percentUsed)) continue
    const percent = Math.round(Math.max(0, percentUsed) * 10) / 10
    found.set(kind as Kind, typeof resetsAt === 'string' ? { kind: kind as Kind, percent, resetsAt } : { kind: kind as Kind, percent })
  }
  return KINDS.flatMap(kind => found.get(kind as Kind) ?? [])
}

const DAYS = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat']
const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec']
const two = (n: number) => String(n).padStart(2, '0')

// When a window recharges, from its `resetsAt` and the time now (epoch ms): `any moment` once past, `in 35m`
// under an hour, `in 4h 12m` up to a day, else the local date (`Sat 24 Oct 20:00`). Null for a missing or
// unparseable stamp, or no clock: nothing is invented. The delta is epoch arithmetic (DST-safe); only the
// far date reads the local calendar.
export function untilText(resetsAt: string | undefined, now: number | undefined): string | null {
  if (resetsAt === undefined || now === undefined || !Number.isFinite(now)) return null
  const target = new Date(resetsAt).getTime()
  if (Number.isNaN(target)) return null
  const diff = Math.floor((target - now) / 1000)
  if (diff <= 0) return 'any moment'
  if (diff < 3600) return `in ${Math.floor(diff / 60)}m`
  if (diff <= 86400) return `in ${Math.floor(diff / 3600)}h ${Math.floor((diff % 3600) / 60)}m`
  const at = new Date(target)
  return `${DAYS[at.getDay()]} ${two(at.getDate())} ${MONTHS[at.getMonth()]} ${two(at.getHours())}:${two(at.getMinutes())}`
}

const LABEL: Record<Kind, string> = { five_hour: '5h', seven_day: '7d', spend_limit: '$' }

type Options = { columns: number; cap: number; ascii: boolean; now?: number }
type MeterOptions = Options & { maxBar?: number; fullOnly?: boolean }

const count = (text: string) => [...text].length
const shown = (percent: number) => Math.round(percent)

const markOf = (level: Level, ascii: boolean) => (level === 'ok' ? '' : ` ${glyphs(ascii)[level]}`)

// The five-hour window as the hint line's bar, at most `columns` cells: `◷ ██████░░ 41% ↻ in 4h 12m ⚠`, the
// recharge delta (untilText, only with a valid `resetsAt` and `now`) after the percent and the glyph mark
// last. Narrower the delta goes first, then the bar, then the icon; an exhausted window (100 and over) keeps
// its delta longest, dropping the bar before it. Null where not even `NN%` fits.
export function limitMeter(limit: Limit, o: MeterOptions): string | null {
  const icon = o.ascii ? '~' : '◷'
  const recharge = o.ascii ? 'r' : '↻'
  const percent = `${shown(limit.percent)}%`
  const mark = markOf(limitLevel(limit.percent, o.cap), o.ascii)
  const until = untilText(limit.resetsAt, o.now)
  const delta = until === null ? '' : ` ${recharge} ${until}`
  const head = `${icon} `
  const barWith = (tail: string): string | null => {
    const free = Math.min(o.maxBar ?? MAX_BAR, o.columns - head.length - tail.length - 1)
    return free < MIN_BAR ? null : `${head}${bar(Math.min(100, limit.percent), free, o.ascii)} ${tail}`
  }
  const withDelta = `${percent}${delta}${mark}`
  const plain = `${percent}${mark}`
  const exhausted = limit.percent >= 100
  const forms = [
    delta === '' ? null : barWith(withDelta),
    ...(exhausted && delta !== '' ? [`${head}${withDelta}`] : []),
    barWith(plain),
    `${head}${plain}`,
    percent,
  ]
  const fit = (form: string | null): form is string => form !== null && count(form) <= o.columns
  // `fullOnly`: only the richest form (the bar, with the delta when there is one), else nothing.
  if (o.fullOnly === true) {
    const rich = delta === '' ? forms[2] : forms[0]
    return rich !== undefined && fit(rich) ? rich : null
  }
  return forms.find(fit) ?? null
}

// The week on the hint line: `▦ 7d 12% ↻ Sat 24 Oct 20:00` with the delta, else `▦ 7d 12%`, glyph mark last.
function weekText(limit: Limit, o: Options, withDelta: boolean): string {
  const icon = o.ascii ? '=' : '▦'
  const until = withDelta ? untilText(limit.resetsAt, o.now) : null
  const delta = until === null ? '' : ` ${o.ascii ? 'r' : '↻'} ${until}`
  return `${icon} 7d ${shown(limit.percent)}%${delta}${markOf(limitLevel(limit.percent, o.cap), o.ascii)}`
}

// The hint line's usage tail for the five-hour window, at most `columns` cells: wide, the five-hour bar
// with its delta and the week beside it (with its delta, then its percent only); narrower the week goes
// (delta first, then the segment), then the five-hour delta, then its bar. Null with no five-hour window.
export function hintTail(five: Limit, week: Limit | undefined, o: Options): string | null {
  const sep = o.ascii ? ' | ' : ' · '
  if (week !== undefined) {
    for (const withDelta of [true, false]) {
      const second = weekText(week, o, withDelta)
      if (withDelta && second === weekText(week, o, false)) continue
      const first = limitMeter(five, { ...o, columns: o.columns - count(second) - count(sep), maxBar: 8, fullOnly: true })
      if (first !== null) return `${first}${sep}${second}`
    }
  }
  return limitMeter(five, o)
}

export type Extras = { rows: string[]; warnings: string[] }

type Piece = { text: (form: 'bar' | 'icon' | 'plain', width: number, word: boolean, delta: boolean) => string; level: Level; critical: string }

// What the workspace line adds: Details rows for the context and the week, and, for each one that is critical,
// a warning that shows even while Details is closed. `context` is the raw context percent, `week` the
// seven_day and spend_limit windows; either may be absent. Rows fit `columns` (richest first: the words,
// then smaller bars, then no bars, then bare percents), splitting over two rows before dropping a piece.
export function extrasOf(context: unknown, limits: Limit[], o: Options & { threshold: number }): Extras {
  const recharge = o.ascii ? 'r' : '↻'
  const g = glyphs(o.ascii)
  const ctx = parsePercent(context)
  const pieces: Piece[] = []
  if (ctx !== null) {
    const level = levelOf(ctx, o.threshold)
    pieces.push({
      level,
      critical: `${g.critical} context ${ctx}%`,
      text: (form, width, word) => {
        const mark = `${ctx}%${markOf(level, o.ascii)}`
        if (form === 'plain') return `${ctx}%`
        return `${g.icon} ${form === 'bar' ? `${bar(ctx, width, o.ascii)} ` : ''}${mark}${word ? ' context' : ''}`
      },
    })
  }
  const icons: Partial<Record<Kind, string>> = { seven_day: o.ascii ? '=' : '▦' }
  for (const kind of ['seven_day', 'spend_limit'] as const) {
    const limit = limits.find(l => l.kind === kind)
    if (limit === undefined) continue
    const level = limitLevel(limit.percent, o.cap)
    pieces.push({
      level,
      critical: `${g.critical} ${LABEL[kind]} ${shown(limit.percent)}%`,
      text: (form, width, _word, withDelta) => {
        const until = withDelta ? untilText(limit.resetsAt, o.now) : null
        const delta = until === null ? '' : ` ${recharge} ${until}`
        const body = `${LABEL[kind]} ${shown(limit.percent)}%`
        if (form === 'plain') return body
        const mark = markOf(level, o.ascii)
        if (kind === 'spend_limit') return `${body}${delta}${mark}`
        const bars = form === 'bar' ? `${bar(Math.min(100, limit.percent), width, o.ascii)} ` : ''
        return `${icons[kind]} ${LABEL[kind]} ${bars}${shown(limit.percent)}%${delta}${mark}`
      },
    })
  }
  const warnings = pieces.filter(p => p.level === 'critical').map(p => p.critical)
  if (pieces.length === 0) return { rows: [], warnings }

  const sep = o.ascii ? ' | ' : ' · '
  const row = (list: Piece[], form: 'bar' | 'icon' | 'plain', word: boolean, delta: boolean, width: number) => list.map(p => p.text(form, width, word, delta)).join(sep)
  const tiers: ['bar' | 'icon' | 'plain', boolean, boolean][] = [['bar', true, true], ['bar', true, false], ['bar', false, false], ['icon', false, false], ['plain', false, false]]
  const best = (list: Piece[]): string | null => {
    for (const [form, word, delta] of tiers) {
      for (let width = form === 'bar' ? ROW_BAR : 0; width >= (form === 'bar' ? MIN_BAR : 0); width--) {
        const text = row(list, form, word, delta, width)
        if (count(text) <= o.columns) return text
      }
    }
    return null
  }
  const one = best(pieces)
  if (one !== null) return { rows: [one], warnings }
  const rows = pieces.map(p => best([p])).filter((r): r is string => r !== null)
  return { rows, warnings }
}
