// The workspace line above the prompt, pure (specs/031-session-chrome): the bands that carry a
// compact summary, folded into one line and a warnings line, sized to the width. A hold or a
// failure never folds (band-priority.ts `isFoldable`), so a critical item is always drawn by its
// own band, never behind this line or its Details button.

import { ordered, split } from './band-priority'
import type { Band, Tone } from './band-priority'

export type Mode = 'wide' | 'medium' | 'narrow'
export const WIDE = 80
export const MEDIUM = 50

export type WorkspaceView = {
  mode: Mode
  // Null when no folded band is ok or info.
  line: string | null
  // Null when no folded band is a warning.
  warnings: string | null
  // One row per folded band, whole: what the Details button shows.
  details: string[]
  // How many bands folded.
  count: number
}

const glyphs = (ascii: boolean) => ({
  sep: ascii ? ' | ' : ' · ',
  ok: ascii ? 'ok' : '✓',
  warn: ascii ? '!' : '⚠',
  ellipsis: ascii ? '...' : '…',
  collapsed: ascii ? '>' : '▸',
  expanded: ascii ? 'v' : '▾',
})

export const toggleGlyph = (isOpen: boolean, ascii: boolean): string => glyphs(ascii)[isOpen ? 'expanded' : 'collapsed']

export const modeOf = (columns: number): Mode => (columns >= WIDE ? 'wide' : columns >= MEDIUM ? 'medium' : 'narrow')

// Cells a text takes: the warning sign is drawn two wide.
const width = (text: string): number => [...text].length + [...text].filter(ch => ch === '⚠').length

function cut(text: string, max: number, ellipsis: string): string {
  if (width(text) <= max) return text
  let out = ''
  for (const ch of text) {
    if (width(out + ch + ellipsis) > max) break
    out += ch
  }
  return out + ellipsis
}

// As many items as fit, then `+N`; a first item too long is cut with an ellipsis.
function fit(prefix: string, items: readonly string[], sep: string, max: number, ellipsis: string): string {
  for (let shown = items.length; shown >= 1; shown--) {
    const extra = items.length - shown
    const text = prefix + items.slice(0, shown).join(sep) + (extra > 0 ? ` +${extra}` : '')
    if (width(text) <= max) return text
  }
  return cut(prefix + (items[0] ?? ''), max, ellipsis)
}

// `extras` are the context and week rows (usage-model.ts extrasOf): Details rows after the bands', and the
// critical ones as warnings, so a critical limit never hides behind the collapsed Details. With rows only,
// the line is just the Details button.
export function workspaceView(
  bands: readonly Band[],
  options: { columns: number; ascii: boolean; extras?: { rows: readonly string[]; warnings: readonly string[] } },
): WorkspaceView | null {
  const { folded } = split(bands, true)
  const extras = options.extras ?? { rows: [], warnings: [] }
  if (folded.length === 0 && extras.rows.length === 0 && extras.warnings.length === 0) return null
  const g = glyphs(options.ascii)
  const mode = modeOf(options.columns)
  // Room for the Details button beside the line.
  const max = Math.max(12, options.columns - 14)
  const items = (tone: Tone[]): string[] =>
    ordered(folded)
      .filter(band => tone.includes(band.summary?.tone ?? 'info'))
      .map(band => (band.summary?.tone === 'ok' ? `${g.ok} ${band.summary.text}` : (band.summary?.text ?? '')))
  const calm = items(['ok', 'info'])
  const warns = [...items(['warn']), ...extras.warnings]
  return {
    mode,
    line: calm.length === 0 ? null : fit(mode === 'wide' ? 'Workspace: ' : '', calm, g.sep, max, g.ellipsis),
    warnings: warns.length === 0 ? null : fit(`${g.warn} `, warns, g.sep, max, g.ellipsis),
    details: [
      ...ordered(folded).map(band => cut(`${band.plugin}: ${band.summary?.text ?? band.id}`, Math.max(12, options.columns - 2), g.ellipsis)),
      ...extras.rows,
    ],
    count: folded.length,
  }
}
