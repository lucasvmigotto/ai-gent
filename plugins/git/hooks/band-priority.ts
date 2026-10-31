// Which bands above the prompt draw (specs/030-band-priority). Pure: no engine
// calls. One copy per hooks module that draws a band (scripts/consistency.py
// --sync-shared-code keeps them equal), because plugins cannot see each other's
// trees: each one applies this rule to the bands every plugin recorded.

export type Priority = 'hold' | 'failure' | 'normal' | 'info'

// One active band, as a plugin records it in its `bands` atom.
export type Band = { id: string; plugin: string; priority: Priority; since: number }

// Highest first (FR-001).
export const PRIORITIES: readonly Priority[] = ['hold', 'failure', 'normal', 'info']

export const DEFAULT_CAP = 2

// userConfig bandCap: a whole number from 1 to 5, else the default.
export function clampCap(value: unknown): number {
  if (typeof value !== 'number' || !Number.isFinite(value)) return DEFAULT_CAP
  return Math.min(5, Math.max(1, Math.round(value)))
}

const rank = (band: Band): number => PRIORITIES.indexOf(band.priority)

// By priority, then the one active longer, then by id so the order is stable.
export function ordered(bands: readonly Band[]): Band[] {
  return [...bands].sort((a, b) => rank(a) - rank(b) || a.since - b.since || (a.id < b.id ? -1 : a.id > b.id ? 1 : 0))
}

// The ids to draw: the first `cap` by order, and every `hold` band even above the cap (FR-005).
export function visible(bands: readonly Band[], cap: number): string[] {
  const all = ordered(bands)
  const holds = all.filter(band => band.priority === 'hold')
  const room = Math.max(0, clampCap(cap) - holds.length)
  const others = all.filter(band => band.priority !== 'hold').slice(0, room)
  return ordered([...holds, ...others]).map(band => band.id)
}

// The dim line for what did not draw; null when nothing is hidden.
export function moreLine(hidden: number): string | null {
  return hidden > 0 ? `+${hidden} more band${hidden === 1 ? '' : 's'} (/bands)` : null
}

// The id of the one hidden band whose plugin draws the line, so exactly one plugin does.
export function moreLiner(bands: readonly Band[], cap: number): string | null {
  const shown = new Set(visible(bands, cap))
  return ordered(bands).find(band => !shown.has(band.id))?.id ?? null
}

// What a plugin's atom holds, checked: anything else is no band (an absent or older plugin).
export function parseBands(value: unknown): Band[] {
  if (!Array.isArray(value)) return []
  return value.filter(
    (item): item is Band =>
      typeof item === 'object' &&
      item !== null &&
      typeof item.id === 'string' &&
      typeof item.plugin === 'string' &&
      PRIORITIES.includes(item.priority) &&
      typeof item.since === 'number',
  )
}

// The /bands text: every active band, its plugin, priority and whether it draws.
export function listing(bands: readonly Band[], cap: number): string {
  if (bands.length === 0) return 'No active bands.'
  const shown = new Set(visible(bands, cap))
  const lines = ordered(bands).map(band => `- ${band.id} (${band.plugin}, ${band.priority}): ${shown.has(band.id) ? 'shown' : 'hidden'}`)
  return [`${bands.length} active band${bands.length === 1 ? '' : 's'}, at most ${clampCap(cap)} shown (holds always):`, ...lines].join('\n')
}
