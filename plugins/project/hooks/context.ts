// The context meter's calls (specs/031-session-chrome). The figure is read once per turn end
// and at session start (turn.complete and session.start, the nudge's own moments), kept in an
// atom, and drawn from there: drawing reads the atom, never the engine, and nothing polls.
// The wording and levels are context-model.ts; the nudge's toast is handoff.ts and shares the read.

import { atom, read, update } from 'claude-code'
import type { EngineInterface } from 'claude-code'

import { meterOf, parsePercent, thresholdOf } from './context-model'

const contextAtom = atom({ plugin: 'project', key: 'contextPercent' } as const, null as number | null)

export type ContextOptions = { contextMeter?: boolean; handoffNudgeContextPercent?: number; chromeAscii?: boolean }

// `$.session.usage().context.percent`; undefined when the engine gives none or the call fails.
export async function readContextPercent($: EngineInterface): Promise<number | undefined> {
  const usage = await $.session.usage().catch(() => undefined)
  return usage?.context.percent
}

// Keeps the figure the meter draws; a missing or invalid one clears it, never a stale or made-up value.
export async function recordContext($: EngineInterface, percent: unknown): Promise<void> {
  const next = parsePercent(percent)
  if ((await read($, contextAtom)) !== next) await update($, contextAtom, () => next)
}

// Text for the end of the prompt's hint line, or null when there is no figure or no room.
// `hintWidth` is the engine's own hint, which stays; the tail takes what is left of the row.
export async function contextTail($: EngineInterface, options: ContextOptions, columns: number | undefined, hintWidth: number): Promise<string | null> {
  const percent = await read($, contextAtom)
  const room = columns === undefined ? 24 : columns - hintWidth - 3
  const meter = meterOf(percent, { columns: room, threshold: thresholdOf(options.handoffNudgeContextPercent), ascii: options.chromeAscii === true })
  return meter === null ? null : `  ${meter.text}`
}
