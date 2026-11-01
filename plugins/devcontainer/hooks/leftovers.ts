// leftovers.sh prints this project's container leftovers and a low disk at
// session start, but only the model sees it. This shows the person the same
// words once, as a toast: the script's own lines, never a check of the mod's own.

import { atom, read, update } from 'claude-code'
import type { EngineInterface } from 'claude-code'

import { hookLines, leftoversBlock, rowText } from './hook-lines'

const toastedAtom = atom({ plugin: 'devcontainer', key: 'toasted' } as const, {})

export async function relayLeftovers($: EngineInterface, message: unknown): Promise<void> {
  const block = leftoversBlock(hookLines(rowText(message)))
  if (block === null || (await read($, toastedAtom)).leftovers === true) return
  await update($, toastedAtom, current => ({ ...current, leftovers: true }))
  $.ui.toast(block, { timeoutMs: 12_000 })
}
