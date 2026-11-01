// The context meter's calls (specs/031-session-chrome). The figure is read once per turn end
// and at session start (turn.complete and session.start, the nudge's own moments), kept in an
// atom, and drawn from there: drawing reads the atom, never the engine, and nothing polls.
// The wording and levels are context-model.ts; the nudge's toast is handoff.ts and shares the read.
// The same read carries the account's rate-limit windows (usage.ts), which own the hint line's bar
// while a five-hour window has a reading; the context meter is the fallback there.

import { atom, read, update } from 'claude-code'
import type { EngineInterface } from 'claude-code'

import { meterOf, parsePercent, thresholdOf } from './context-model'
import { nowOf, readLimits } from './usage'
import { capOf, extrasOf, hintTail } from './usage-model'
import type { Extras } from './usage-model'

const contextAtom = atom({ plugin: 'project', key: 'contextPercent' } as const, null as number | null)

export type ContextOptions = { contextMeter?: boolean; usageMeter?: boolean; handoffNudgeContextPercent?: number; handoffNudgeCapPercent?: number; chromeAscii?: boolean }

type Usage = Awaited<ReturnType<EngineInterface['session']['usage']>>

// `$.session.usage()`; undefined when the engine gives none or the call fails.
export async function readUsage($: EngineInterface): Promise<Usage | undefined> {
  return $.session.usage().catch(() => undefined)
}

// Keeps the figure the meter draws; a missing or invalid one clears it, never a stale or made-up value.
export async function recordContext($: EngineInterface, percent: unknown): Promise<void> {
  const next = parsePercent(percent)
  if ((await read($, contextAtom)) !== next) await update($, contextAtom, () => next)
}

// The engine draws the auto-mode label and the pills on the same row as the hint while a turn runs,
// and the hint it hands over may not include them: assume a row of at least this many cells.
export const ENGINE_ROW = 60

// Text for the end of the prompt's hint line, or null when there is no figure or no room. `hintWidth` is the
// engine's own hint, which stays; the tail takes what is left of the row, counting the engine's whole row
// (mode label and pills) while a turn runs. The five-hour window (usageMeter) draws first, with the week
// beside it when wide; with no five-hour reading the context meter (contextMeter) stands in.
export async function contextTail(
  $: EngineInterface,
  options: ContextOptions,
  columns: number | undefined,
  hintWidth: number,
  isWorking = false,
): Promise<string | null> {
  const row = isWorking ? Math.max(hintWidth, ENGINE_ROW) : hintWidth
  const room = columns === undefined ? 24 : columns - row - 3
  const ascii = options.chromeAscii === true
  if (options.usageMeter !== false) {
    const limits = await readLimits($)
    const five = limits.find(l => l.kind === 'five_hour')
    if (five !== undefined) {
      const week = limits.find(l => l.kind === 'seven_day')
      const text = hintTail(five, week, { columns: room, cap: capOf(options.handoffNudgeCapPercent), ascii, now: await nowOf($) })
      return text === null ? null : `  ${text}`
    }
  }
  if (options.contextMeter === false) return null
  const meter = meterOf(await read($, contextAtom), { columns: room, threshold: thresholdOf(options.handoffNudgeContextPercent), ascii })
  return meter === null ? null : `  ${meter.text}`
}

// The context and the week for the workspace line (rows for Details, criticals for its warnings), or
// nothing when contextMeter is off. `columns` is the room for a row.
export async function contextExtras($: EngineInterface, options: ContextOptions, columns: number): Promise<Extras> {
  if (options.contextMeter === false) return { rows: [], warnings: [] }
  const limits = await readLimits($)
  return extrasOf(await read($, contextAtom), limits, {
    columns,
    threshold: thresholdOf(options.handoffNudgeContextPercent),
    cap: capOf(options.handoffNudgeCapPercent),
    ascii: options.chromeAscii === true,
    now: await nowOf($),
  })
}
