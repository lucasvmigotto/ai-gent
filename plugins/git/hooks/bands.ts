// This plugin's side of band priority (specs/030-band-priority): the bands it
// has active, recorded in its `bands` atom, and the rule applied to every
// plugin's bands. The rule itself is band-priority.ts (shared/, copied).
// Plugins cannot see each other's trees, so each draws only its own bands and
// reads the others' atom; across plugins the cap holds as far as each plugin
// that draws a band does this. Reading is defensive: project may be absent.

import { atom, read, update } from 'claude-code'
import type { EngineInterface } from 'claude-code'

import { DEFAULT_CAP, clampCap, listing, moreLine, moreLiner, parseBands, visible } from './band-priority'
import type { Band } from './band-priority'

export type Options = { bandCap?: number; bandsCommand?: boolean }

const bandsAtom = atom({ plugin: 'git', key: 'bands' } as const, [] as Band[])

export const cap = (options: Options): number => clampCap(options.bandCap ?? DEFAULT_CAP)

// Records whether this plugin's band `id` is active now (`priority`) or not (null),
// leaving its other bands as they are. A band that was already active keeps its
// `since`, so ties keep their order from one draw to the next.
export async function setBand($: EngineInterface, id: string, priority: Band['priority'] | null): Promise<void> {
  const now = await $.clock.now().catch(() => 0)
  await update($, bandsAtom, before => {
    const since = before.find(band => band.id === id)?.since
    const rest = before.filter(band => band.id !== id)
    return priority === null ? rest : [...rest, { id, priority, plugin: 'git', since: since ?? now }]
  })
}

// Every plugin's active bands: this one's, and project's when it is installed and
// has recorded any. An absent plugin, an older one or a read that throws adds none.
export async function allBands($: EngineInterface): Promise<Band[]> {
  const own = parseBands(await read($, bandsAtom).catch(() => []))
  const others = parseBands(await read($, { plugin: 'project', key: 'bands' } as const).catch(() => undefined))
  return [...own, ...others]
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

export async function bandsText($: EngineInterface, options: Options): Promise<string> {
  return listing(await allBands($), cap(options))
}
