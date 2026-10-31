// The devcontainer plugin's Claude Code mod, optional features that only make
// visible what the plugin already says:
//  - guard-ux (specs/015): why the containers guard denied, and what to do
//    instead. hooks/guard.sh still decides.
//  - leftovers toast (specs/017): the report leftovers.sh prints at session
//    start, which only the model saw, shown to the person once.
//  - the container and disk band (specs/025): the same findings as JSON, kept
//    above the prompt and refreshed after container work, and /containers.
//    Read only: nothing here removes, stops or builds anything.
// Each is gated on its own option (guardUx, leftoversToast, containersBand,
// containersCommand); one session.start handler serves the band and the command.

import { atom, update } from 'claude-code'
import type { PreToolUseResult, Register } from 'claude-code'

import { plan } from './bands'
import type { Options as BandOptions } from './bands'
import { CONTAINERS, containersHooks, containersReport, readContainers, startReading } from './containers'
import type { Options as ContainersOptions } from './containers'
import { bandText } from './containers-model'
import { append, decorate, record } from './guard-ux'
import type { GuardRow } from './guard-ux'
import { relayLeftovers } from './leftovers'

type Options = BandOptions & ContainersOptions & { guardUx?: boolean; guardLog?: boolean; leftoversToast?: boolean }

// The ring of this guard's recent decisions (specs/024-guard-log); the project plugin reads it.
const guardLogAtom = atom({ plugin: 'devcontainer', key: 'guardLog' } as const, [] as GuardRow[], { shape: 'guard-log-1' })

export const register: Register = (on, options: Options) => {
  if (options.guardUx !== false || options.guardLog !== false) {
    on('classic.PreToolUse', async ($, e, next) => {
      const result = await next(e)
      // The row comes from the guard's own answer, before decorate adds words.
      if (options.guardLog !== false) {
        // A failure here loses a row, never the decision.
        await (async () => {
          const row = record('containers', result, e, await $.clock.now())
          if (row) await update($, guardLogAtom, rows => append(rows, row))
        })().catch(() => undefined)
      }
      if (options.guardUx === false) return result
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

  const hasBand = options.containersBand !== false
  const hasCommand = options.containersCommand !== false

  if (hasBand || hasCommand) {
    on('session.start', async ($, e, next) => {
      const started = await next(e)
      if (hasCommand) {
        await $.command.register({ name: 'containers', description: "This project's container leftovers and low disk, as leftovers.sh reports them (report only)" }).catch(() => undefined)
      }
      if (hasBand) startReading($)
      return started
    })
  }

  if (hasCommand) {
    on('command.run', { command: 'containers' }, async $ => ({ text: await containersReport($) }))
  }

  if (!hasBand) return
  containersHooks(on, options)

  // Composes with whatever band is beneath (the checkpoint, hygiene and CI bands).
  on('ui.render', { component: 'AbovePrompt' }, async ($, e, next) => {
    const beneath = await next(e)
    if (e.props.hasSurvey || e.props.isWorking) return beneath
    const state = await readContainers($)
    const text = state === null || !state.checked ? null : bandText(state.findings, e.props.bodyColumns)
    if (text === null) return beneath

    const { draws, more } = await plan($, CONTAINERS, options)
    const { Box, Text } = $.ui.resolve(e)
    return (
      <Box flexDirection="column">
        {beneath}
        {draws ? (
          <Text color={state?.findings.some(finding => finding.kind === 'disk') ? 'warning' : undefined} wrap="truncate-end">
            {text}
          </Text>
        ) : null}
        {more === null ? null : <Text dimColor>{more}</Text>}
      </Box>
    )
  })
}
