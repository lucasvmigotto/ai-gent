// The repo plugin's Claude Code mod: presents what the repo guard guard decides (specs/015-guard-ux).
// hooks/guard.sh still decides; this only explains a deny or an ask that
// carries that guard's marker, and passes everything else through untouched.

import type { PreToolUseResult, Register } from 'claude-code'

import { decorate } from './guard-ux'

type Options = { guardUx?: boolean }

export const register: Register = (on, options: Options) => {
  if (options.guardUx === false) return

  on('classic.PreToolUse', async ($, e, next) => {
    const result = await next(e)
    const { result: decorated, notice } = decorate('repo', result)
    if (notice) $.ui.toast(notice)
    return decorated as PreToolUseResult
  }).catch(($, e, next) => next(e))
}
