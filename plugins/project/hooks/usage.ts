// The usage-limit meter's calls (specs/031-session-chrome). The account's rate-limit windows
// (`$.session.usage().rateLimits`) are kept in an atom, updated from the pushed `session.measure`
// and from the one usage read the context meter already makes at session start and each turn end.
// Drawing reads the atom, never the engine; nothing polls. The wording and levels are usage-model.ts.

import { atom, read, update } from 'claude-code'
import type { EngineInterface } from 'claude-code'

import { parseLimits } from './usage-model'
import type { Limit } from './usage-model'

const limitsAtom = atom({ plugin: 'project', key: 'rateLimits' } as const, [] as Limit[])

// Keeps the windows the meter draws; missing or invalid readings clear them, never a stale or made-up one.
export async function recordLimits($: EngineInterface, raw: unknown): Promise<void> {
  const next = parseLimits(raw)
  const known = await read($, limitsAtom)
  if (JSON.stringify(known) !== JSON.stringify(next)) await update($, limitsAtom, () => next)
}

export async function readLimits($: EngineInterface): Promise<Limit[]> {
  return read($, limitsAtom)
}

// The time now for the recharge deltas; undefined where the engine gives none (no delta is drawn then).
export async function nowOf($: EngineInterface): Promise<number | undefined> {
  const now = await $.clock.now().catch(() => undefined)
  return typeof now === 'number' ? now : undefined
}
