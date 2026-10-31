// The agents plugin's Claude Code mod: the token status (specs/014-token-status).
// A status line with the cards in flight, their projection against what they
// used, and what the budget leaves; a toast when the wave stops fitting or a
// card passes its projection; /tokens for the full text. token_report.py stays
// the only thing that measures or judges a budget, and the mod writes nothing.

import type { Register } from 'claude-code'

import { refresh, refreshThrottled, tokensText } from './tokens'
import type { TokenOptions } from './tokens'

type Options = { tokenStatus?: boolean; tokenRefreshSeconds?: number } & TokenOptions

export const register: Register = (on, options: Options) => {
  if (options.tokenStatus === false) return
  const seconds = options.tokenRefreshSeconds ?? 120
  // The burn rate and its ETA toast (specs/018-token-burn-rate) ride the same refresh.
  const rate: TokenOptions = { tokenBurnRate: options.tokenBurnRate, tokenEtaWarnMinutes: options.tokenEtaWarnMinutes }

  on('session.start', async ($, e, next) => {
    const started = await next(e)
    await $.command.register({ name: 'tokens', description: 'Show the token budget for the cards in flight (token_report.py budget)' }).catch(() => undefined)
    // Detached: the budget reads a week of transcripts and must not hold the session's start.
    $.clock.after(0, () => void refresh($, rate).catch(() => undefined))
    return started
  })

  on('turn.complete', async ($, e, next) => {
    $.clock.after(0, () => void refreshThrottled($, seconds, rate).catch(() => undefined))
    return next(e)
  })

  on('command.run', { command: 'tokens' }, async $ => ({ text: await tokensText($, rate) }))
}
