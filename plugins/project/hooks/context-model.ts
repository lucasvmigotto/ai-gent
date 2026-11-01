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

export const MIN_BAR = 6
export const MAX_BAR = 20

export const glyphs = (ascii: boolean) => ({
  icon: ascii ? '@' : '◐',
  full: ascii ? '#' : '█',
  empty: ascii ? '-' : '░',
  approaching: ascii ? '!' : '⚠',
  critical: ascii ? '!!' : '✖',
})

export function bar(percent: number, width: number, ascii: boolean): string {
  const g = glyphs(ascii)
  const filled = Math.round((percent / 100) * width)
  return g.full.repeat(filled) + g.empty.repeat(width - filled)
}

// One line of at most `columns` cells: a context icon, a bar following the free width and the
// percent used (`◐ ████░░░░ 24%`), plus a glyph (no words) once approaching or critical; narrower, the icon and the
// percent, then the percent alone. Nothing (null) for a missing, NaN or infinite figure, and where not even `NN%` fits.
export function meterOf(value: unknown, options: { columns: number; threshold: number; ascii: boolean }): Meter | null {
  const percent = parsePercent(value)
  if (percent === null) return null
  const g = glyphs(options.ascii)
  const level = levelOf(percent, options.threshold)
  const mark = level === 'ok' ? '' : ` ${g[level]}`
  const tail = `${percent}%${mark}`
  const head = `${g.icon} `
  const free = Math.min(MAX_BAR, options.columns - head.length - tail.length - 1)
  const forms = [free < MIN_BAR ? null : `${head}${bar(percent, free, options.ascii)} ${tail}`, `${head}${tail}`, tail, `${percent}%`]
  const text = forms.find((form): form is string => form !== null && form.length <= options.columns)
  return text === undefined ? null : { text, level, percent }
}
