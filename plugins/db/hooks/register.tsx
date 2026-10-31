// The db plugin's Claude Code mod: presents what the db guard guard decides (specs/015-guard-ux).
// hooks/guard.sh still decides; this only explains a deny or an ask that
// carries that guard's marker, and passes everything else through untouched.

import { atom, update } from 'claude-code'
import type { PreToolUseResult, Register } from 'claude-code'

import { append, decorate, record } from './guard-ux'
import type { GuardRow } from './guard-ux'

type Options = { guardUx?: boolean; guardLog?: boolean }

// The ring of this guard's recent decisions (specs/024-guard-log); the project plugin reads it.
const guardLogAtom = atom({ plugin: 'db', key: 'guardLog' } as const, [] as GuardRow[], { shape: 'guard-log-1' })

export const register: Register = (on, options: Options) => {
  if (options.guardUx === false && options.guardLog === false) return

  on('classic.PreToolUse', async ($, e, next) => {
    const result = await next(e)
    // The row comes from the guard's own answer, before decorate adds words.
    if (options.guardLog !== false) {
      // A failure here loses a row, never the decision.
      await (async () => {
        const row = record('db', result, e, await $.clock.now())
        if (row) await update($, guardLogAtom, rows => append(rows, row))
      })().catch(() => undefined)
    }
    if (options.guardUx === false) return result
    const { result: decorated, notice } = decorate('db', result)
    if (notice) $.ui.toast(notice)
    return decorated as PreToolUseResult
  }).catch(($, e, next) => next(e))
}
