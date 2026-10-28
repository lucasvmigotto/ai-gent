// The secrets plugin's Claude Code mod, two optional features; the guard hook
// and the skills stay the contract, and nothing relies on either:
//  - secret-mask (specs/016): a safety net for shared/secrets.md's rule that a
//    secret is never pasted into the conversation: when one is anyway, mask it
//    before it is recorded or sent, and say where it belongs.
//  - guard-ux (specs/015): say why the secrets guard denied or asked, and what
//    to do instead. hooks/guard.sh still decides.

import type { EngineInterface, PreToolUseResult, Register } from 'claude-code'

import { decorate } from './guard-ux'
import { compile, looksSecret, mask, summary } from './secret-mask'
import type { Masked, Rule, Scope } from './secret-mask'

type Options = { secretMask?: boolean; maskAssignments?: boolean; guardUx?: boolean }

const HELD =
  'secret-mask: this looks like it holds a credential and could not be checked, so it was not sent. Remove the secret (store it with secrets:pass) and send it again, or turn secretMask off in /config.'

// The patterns, read once (a module can import code, not JSON); a failed read is tried again next time.
let loading: Promise<Rule[]> | null = null

function loadRules($: EngineInterface): Promise<Rule[]> {
  loading ??= $.fs
    .read(`${$.plugin.root}/references/secret-patterns.json`)
    .then(text => compile(String(text)))
    .catch(error => {
      loading = null
      throw error
    })
  return loading
}

// Counts only: what was masked is never logged, shown or stored.
function announce($: EngineInterface, masked: Masked, event: string): void {
  $.ui.toast(summary(masked))
  $.ui.log(`secret-mask: ${event}: ${JSON.stringify(masked.counts)}`, { to: 'debug' })
}

async function maskText($: EngineInterface, text: string, scope: Scope, off: ReadonlySet<string>, event: string) {
  const masked = mask(text, await loadRules($), scope, off)
  if (masked.total > 0) announce($, masked, event)
  return masked
}

export const register: Register = (on, options: Options) => {
  if (options.guardUx !== false) {
    on('classic.PreToolUse', async ($, e, next) => {
      const result = await next(e)
      const { result: decorated, notice } = decorate('secrets', result)
      if (notice) $.ui.toast(notice)
      return decorated as PreToolUseResult
    }).catch(($, e, next) => next(e))
  }

  if (options.secretMask === false) return
  const off = new Set(options.maskAssignments === false ? ['maskAssignments'] : [])

  // What the person types. A slash command's line takes no rewrite here (its
  // arguments are masked at command.run), nor does a hidden prompt that
  // starts a turn of its own.
  on('prompt.submit', async ($, e, next) => {
    if (e.command) return next(e)
    if (e.isMeta && e.turnId === undefined) {
      if (looksSecret(e.text, 'prompt')) $.ui.log('secret-mask: a hidden prompt takes no rewrite; not masked', { to: 'debug' })
      return next(e)
    }
    const masked = await maskText($, e.text, 'prompt', off, 'prompt.submit')
    return masked.total > 0 ? next({ ...e, text: masked.text }) : next(e)
  }).catch(($, e, next) =>
    next.called || e.command || !looksSecret(e.text, 'prompt') ? next(e) : { drop: HELD },
  )

  on('command.run', async ($, e, next) => {
    if (e.args === '') return next(e)
    const masked = await maskText($, e.args, 'prompt', off, 'command.run')
    return masked.total > 0 ? next({ ...e, args: masked.text }) : next(e)
  }).catch(($, e, next) =>
    next.called || !looksSecret(e.args, 'prompt') ? next(e) : { text: HELD },
  )

  on('skill.prompt', async ($, e, next) => {
    const masked = await maskText($, e.text, 'skill', off, 'skill.prompt')
    return masked.total > 0 ? next({ ...e, text: masked.text }) : next(e)
  }).catch(($, e, next) =>
    next.called || !looksSecret(e.text, 'skill') ? next(e) : { text: HELD },
  )
}
