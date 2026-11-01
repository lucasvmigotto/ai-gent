// The repo plugin's Claude Code mod, two independent parts:
//  - guard-ux (specs/015): presents what the repo guard decides. hooks/guard.sh
//    still decides; this only explains a deny or an ask that carries that
//    guard's marker, and passes everything else through untouched.
//  - the CI watch band (specs/023): after a push, the state of that commit's
//    runs above the prompt and a toast on failure. Read only, GitHub only.
// Each is gated on its own option (guardUx, ciBand).

import { atom, update } from 'claude-code'
import type { PreToolUseResult, Register } from 'claude-code'

import { plan } from './bands'
import type { Options as BandOptions } from './bands'
import { CI, ciHooks, hideBand, readCi } from './ci'
import type { CiOptions } from './ci'
import { bandText, isFailed } from './ci-model'
import { append, decorate, record } from './guard-ux'
import type { GuardRow } from './guard-ux'

type Options = BandOptions & CiOptions & { guardUx?: boolean; guardLog?: boolean; ciBand?: boolean }

// The ring of this guard's recent decisions (specs/024-guard-log); the project plugin reads it.
const guardLogAtom = atom({ plugin: 'repo', key: 'guardLog' } as const, [] as GuardRow[], { shape: 'guard-log-1' })

export const register: Register = (on, options: Options) => {
  if (options.guardUx !== false || options.guardLog !== false) {
    on('classic.PreToolUse', async ($, e, next) => {
      const result = await next(e)
      // The row comes from the guard's own answer, before decorate adds words.
      if (options.guardLog !== false) {
        // A failure here loses a row, never the decision.
        await (async () => {
          const row = record('repo', result, e, await $.clock.now())
          if (row) await update($, guardLogAtom, rows => append(rows, row))
        })().catch(() => undefined)
      }
      if (options.guardUx === false) return result
      const { result: decorated, notice } = decorate('repo', result)
      if (notice) $.ui.toast(notice)
      return decorated as PreToolUseResult
    }).catch(($, e, next) => next(e))
  }

  if (options.ciBand === false) return
  ciHooks(on, options)

  // Composes with whatever band is beneath (the checkpoint and hygiene bands).
  on('ui.render', { component: 'AbovePrompt' }, async ($, e, next) => {
    const beneath = await next(e)
    if (e.props.hasSurvey || e.props.isWorking) return beneath
    const state = await readCi($)
    if (state === null || state.hidden) return beneath

    const { draws, more } = await plan($, CI, options)
    const { Box, Button, Text } = $.ui.resolve(e)
    return (
      <Box flexDirection="column">
        {beneath}
        {draws ? (
          <Box>
            <Text color={state.ci.runs.some(isFailed) ? 'error' : undefined} wrap="truncate-end">
              {bandText(state.ci, e.props.bodyColumns)}
            </Text>
            <Text> </Text>
            <Button key="hide" label="Hide" onPress={() => hideBand($)} />
          </Box>
        ) : null}
        {more === null ? null : <Text dimColor>{more}</Text>}
      </Box>
    )
  })
}
