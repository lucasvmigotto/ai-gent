// Reads the five guard plugins' `guardLog` atoms with $.state.get, each on its
// own so an absent plugin (the read throws), or one storing another shape,
// only lands in `missing`. Only the owning plugin ever writes an atom. A log
// never written reads as undefined at version 0 whether its plugin is
// installed or not, so that case is "no decisions", not "not reporting".

import type { EngineInterface } from 'claude-code'

import type { GuardRow } from '../types'
import { grantLine } from './grant-model'
import { parseRows } from './guards'
import type { PluginName } from './guards'
import { mainRoot } from './repo'

const MAX_GRANT_CHARS = 4096

// The autonomy line, from the main checkout's grant file. Read only; a file that cannot be read is no grant.
export async function readGrantLine($: EngineInterface): Promise<string> {
  const root = await mainRoot($).catch(() => null)
  if (root === null) return grantLine(null, new Date())
  const text = await $.fs.read(`${root}/.ai-gent/autonomy`).then(t => String(t).slice(0, MAX_GRANT_CHARS)).catch(() => null)
  return grantLine(text, new Date())
}

const SHAPE = 'guard-log-1'

// The rows of one read, or null when the plugin does not report.
function unwrap(read: { value?: unknown } | null): GuardRow[] | null {
  if (read === null) return null
  const value = read.value
  if (value === undefined) return []
  if (typeof value !== 'object' || value === null) return null
  const shaped = value as { shape?: unknown; value?: unknown }
  return shaped.shape === SHAPE ? parseRows(shaped.value) : null
}

// The rows of every plugin that answered, and the names of those that did not.
export async function readGuards($: EngineInterface): Promise<{ rows: GuardRow[]; missing: PluginName[] }> {
  const answers: [PluginName, GuardRow[] | null][] = [
    ['git', unwrap(await $.state.get({ plugin: 'git', key: 'guardLog' } as const).catch(() => null))],
    ['db', unwrap(await $.state.get({ plugin: 'db', key: 'guardLog' } as const).catch(() => null))],
    ['secrets', unwrap(await $.state.get({ plugin: 'secrets', key: 'guardLog' } as const).catch(() => null))],
    ['repo', unwrap(await $.state.get({ plugin: 'repo', key: 'guardLog' } as const).catch(() => null))],
    ['devcontainer', unwrap(await $.state.get({ plugin: 'devcontainer', key: 'guardLog' } as const).catch(() => null))],
  ]
  const rows: GuardRow[] = []
  const missing: PluginName[] = []
  for (const [name, found] of answers) {
    if (found === null) missing.push(name)
    else rows.push(...found)
  }
  return { rows, missing }
}
