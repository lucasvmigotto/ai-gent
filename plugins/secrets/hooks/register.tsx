// The secrets plugin's Claude Code mod, two optional features; the guard hook
// and the skills stay the contract, and nothing relies on either:
//  - secret-mask (specs/016): a safety net for shared/secrets.md's rule that a
//    secret is never pasted into the conversation: when one is anyway, mask it
//    before it is recorded or sent, and say where it belongs.
//  - output warning (specs/026): when a tool's result holds a credential, say so
//    and, for Bash and WebFetch, replace it in the row Claude reads.
//  - guard-ux (specs/015): say why the secrets guard denied or asked, and what
//    to do instead. hooks/guard.sh still decides.

import type { EngineInterface, PreToolUseResult, Register } from 'claude-code'

import { decorate } from './guard-ux'
import { createLimiter, redactTools, scanContent } from './output-warning'
import type { Block } from './output-warning'
import { compile, looksSecret, mask, summary, warning } from './secret-mask'
import type { Masked, Rule, Scope } from './secret-mask'

type Options = {
  secretMask?: boolean
  maskAssignments?: boolean
  guardUx?: boolean
  secretOutputWarning?: boolean
  secretOutputRedact?: boolean
  secretOutputTools?: string
}

const NOT_CHECKED =
  'output-secret: could not check or redact this result — treat any credential in it as exposed'
const MORE =
  'output-secret: more results held credentials this session, not shown one by one — treat them as exposed too'
const MAX_CALLS = 500

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

// Which tool a result row belongs to: the row's origin, else the name recorded
// by the tool.call that made it (looked up by the tool_use_id its block carries).
function toolOf(origin: { kind?: string; tool?: string }, content: readonly Block[], calls: ReadonlyMap<string, string>): string {
  if (origin.kind === 'tool' && origin.tool && origin.tool !== 'unknown') return origin.tool
  for (const block of content) {
    const name = block.type === 'tool_result' && typeof block.tool_use_id === 'string' ? calls.get(block.tool_use_id) : undefined
    if (name) return name
  }
  return 'unknown'
}

// Fail open throughout: a row is stored as received if the scan or rewrite fails.
function outputWarning(on: Parameters<Register>[0], options: Options): void {
  const redactOn = options.secretOutputRedact !== false
  const tools = redactTools(options.secretOutputTools)
  const limit = createLimiter()
  const calls = new Map<string, string>()

  // The fallback for a row whose origin has no tool name.
  on('tool.call', async ($, e, next) => {
    const id = (e as { tool_use_id?: string }).tool_use_id
    if (id) {
      calls.set(id, e.tool)
      if (calls.size > MAX_CALLS) calls.delete(calls.keys().next().value as string)
    }
    return next(e)
  }).catch(($, e, next) => next(e))

  on('session.append', { door: 'tool-result' }, async ($, e, next) => {
    const content = e.message.content as Block[]
    const tool = toolOf(e.origin as { kind?: string; tool?: string }, content, calls)
    const redact = redactOn && tools.has(tool)
    const scan = scanContent(content, await loadRules($), redact)
    const stored = scan.masked.total > 0 && redact ? await next({ ...e, message: { ...e.message, content: scan.content } }) : await next(e)
    if (scan.masked.total === 0) return stored
    try {
      const rate = limit()
      if (rate === 'toast') $.ui.toast(warning(scan.masked, tool, redact, scan.truncated))
      else if (rate === 'summary') $.ui.toast(MORE)
      $.ui.log(`output-secret: ${tool}: ${JSON.stringify(scan.masked.counts)}`, { to: 'debug' })
    } catch {
      // the row is stored; a toast that fails changes nothing
    }
    return stored
  }).catch(($, e, next) => {
    if (next.called) return next(e)
    $.ui.toast(NOT_CHECKED)
    return next(e)
  })
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

  if (options.secretOutputWarning !== false) outputWarning(on, options)

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
