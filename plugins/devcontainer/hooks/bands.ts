// This plugin's side of band priority (specs/030-band-priority): the band it
// has active, recorded in its `bands` atom, and the rule applied to every
// plugin's bands. The rule itself is band-priority.ts (shared/, copied).
// Plugins cannot see each other's trees, so each draws only its own bands and
// reads the others' atom. Reading is defensive: git, repo and project may be absent.

import { atom, read, update } from 'claude-code'
import type { EngineInterface } from 'claude-code'

import { DEFAULT_CAP, clampCap, moreLine, moreLiner, parseBands, visible } from './band-priority'
import type { Band } from './band-priority'

// No /bands command here: git and project declare it, and a name registered twice would collide.
export type Options = { bandCap?: number }

const bandsAtom = atom({ plugin: 'devcontainer', key: 'bands' } as const, [] as Band[])

export const cap = (options: Options): number => clampCap(options.bandCap ?? DEFAULT_CAP)

// Records the bands that are active now. A band that was already active keeps its
// `since`, so ties keep their order from one draw to the next.
export async function recordBands($: EngineInterface, active: readonly Omit<Band, 'since' | 'plugin'>[]): Promise<void> {
  const now = await $.clock.now().catch(() => 0)
  await update($, bandsAtom, before => {
    const known = new Map(before.map(band => [band.id, band.since]))
    return active.map(band => ({ ...band, plugin: 'devcontainer', since: known.get(band.id) ?? now }))
  })
}

// Every plugin's active bands: this one's, and git's, repo's and project's when installed
// and having recorded any. An absent plugin, an older one or a read that throws adds none.
export async function allBands($: EngineInterface): Promise<Band[]> {
  const own = parseBands(await read($, bandsAtom).catch(() => []))
  const git = parseBands(await read($, { plugin: 'git', key: 'bands' } as const).catch(() => undefined))
  const repo = parseBands(await read($, { plugin: 'repo', key: 'bands' } as const).catch(() => undefined))
  const project = parseBands(await read($, { plugin: 'project', key: 'bands' } as const).catch(() => undefined))
  return [...own, ...git, ...repo, ...project]
}

// What this plugin's band `id` does at this draw: whether it draws, and the dim
// line when it is the plugin whose hidden band owns it.
export async function plan($: EngineInterface, id: string, options: Options): Promise<{ draws: boolean; more: string | null }> {
  const bands = await allBands($)
  const max = cap(options)
  const shown = visible(bands, max)
  const hidden = bands.length - shown.length
  return { draws: shown.includes(id), more: moreLiner(bands, max) === id ? moreLine(hidden) : null }
}
