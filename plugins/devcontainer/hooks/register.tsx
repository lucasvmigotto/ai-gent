// The devcontainer plugin's Claude Code mod, optional features that only make
// visible what the plugin already says:
//  - guard-ux (specs/015): why the containers guard denied, and what to do
//    instead. hooks/guard.sh still decides.
//  - leftovers toast (specs/017): the report leftovers.sh prints at session
//    start, which only the model saw, shown to the person once.

import type { PreToolUseResult, Register } from 'claude-code'

import { decorate } from './guard-ux'
import { relayLeftovers } from './leftovers'

type Options = { guardUx?: boolean; leftoversToast?: boolean }

export const register: Register = (on, options: Options) => {
  if (options.guardUx !== false) {
    on('classic.PreToolUse', async ($, e, next) => {
      const result = await next(e)
      const { result: decorated, notice } = decorate('containers', result)
      if (notice) $.ui.toast(notice)
      return decorated as PreToolUseResult
    }).catch(($, e, next) => next(e))
  }

  if (options.leftoversToast !== false) {
    on('session.append', async ($, e, next) => {
      const origin = e.origin as { kind?: string; event?: string }
      if (e.door === 'hook-context' && origin.kind === 'hook' && origin.event === 'SessionStart') {
        await relayLeftovers($, e.message).catch(() => undefined)
      }
      return next(e)
    }).catch(($, e, next) => next(e))
  }
}
