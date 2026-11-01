// The context meter's wording and levels, pure (specs/031-session-chrome): what the line
// under the prompt says about how full the context window is. The figure is the engine's
// own `$.session.usage().context.percent` (tokens over the window, 0 to 100), never a token
// total; context.ts feeds it. The nudge's threshold (`handoffNudgeContextPercent`, default 80)
// is the one number: the meter is critical from it, approaching from three quarters of it.

export type Level = 'ok' | 'approaching' | 'critical'

export const DEFAULT_THRESHOLD = 80
// The meter turns amber from this share of the threshold (60 % at the default 80 %).
export const APPROACH_RATIO = 0.75

// A threshold from the options: a number from 1 to 100, else the default.
export function thresholdOf(value: unknown): number {
  return typeof value === 'number' && Number.isFinite(value) && value >= 1 && value <= 100 ? value : DEFAULT_THRESHOLD
}

// The percent as a whole number clamped to 0-100; null for anything that is not a finite number.
export function parsePercent(value: unknown): number | null {
  if (typeof value !== 'number' || !Number.isFinite(value)) return null
  return Math.min(100, Math.max(0, Math.round(value)))
}

export function levelOf(percent: number, threshold: number): Level {
  if (percent >= threshold) return 'critical'
  return percent >= Math.floor(threshold * APPROACH_RATIO) ? 'approaching' : 'ok'
}

export type Meter = { text: string; level: Level; percent: number }

const MIN_BAR = 6
const MAX_BAR = 20

const glyphs = (ascii: boolean) => ({
  full: ascii ? '#' : '█',
  empty: ascii ? '-' : '░',
  sep: ascii ? '|' : '·',
  approaching: ascii ? '!' : '⚠',
  critical: ascii ? '!!' : '✖',
})

export function bar(percent: number, width: number, ascii: boolean): string {
  const g = glyphs(ascii)
  const filled = Math.round((percent / 100) * width)
  return g.full.repeat(filled) + g.empty.repeat(width - filled)
}

// One line of at most `columns` cells, richest form first: label, bar following the free width,
// used and remaining, the level's word; poorer forms drop the parts until it fits. Nothing
// (null) for a missing, NaN or infinite figure, and where not even `ctx NN%` fits.
export function meterOf(value: unknown, options: { columns: number; threshold: number; ascii: boolean }): Meter | null {
  const percent = parsePercent(value)
  if (percent === null) return null
  const g = glyphs(options.ascii)
  const level = levelOf(percent, options.threshold)
  const mark = level === 'ok' ? '' : ` ${g[level]} ${level === 'critical' ? 'critical' : 'approaching'}`
  const used = `${percent}% used`
  const remaining = `${100 - percent}% remaining`
  const room = (rest: number): number => Math.min(MAX_BAR, rest)
  const forms: string[] = []
  const withBar = (head: string, tail: string): string | null => {
    const free = room(options.columns - head.length - tail.length - 2)
    return free < MIN_BAR ? null : `${head} ${bar(percent, free, options.ascii)} ${tail}`
  }
  for (const tail of [`${used} ${g.sep} ${remaining}${mark}`, `${used}${mark}`, used]) {
    const form = withBar('Context', tail)
    if (form !== null) forms.push(form)
  }
  forms.push(`Context ${used}${mark}`, `Context ${used}`, `ctx ${percent}%${level === 'ok' ? '' : ` ${g[level]}`}`, `ctx ${percent}%`)
  const text = forms.find(form => form.length <= options.columns)
  return text === undefined ? null : { text, level, percent }
}
