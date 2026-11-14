// The repo plugin's Claude Code mod: the CI watch band (specs/023), after a
// push, the state of that commit's runs above the prompt and a toast on
// failure. Read only, GitHub only; gated on ciBand. The guard's own words and
// its log rows are printed by hooks/guard.sh itself (specs/015 and 024).

import type { Register } from 'claude-code'

import { plan } from './bands'
import type { Options as BandOptions } from './bands'
import { CI, ciHooks, hideBand, readCi } from './ci'
import type { CiOptions } from './ci'
import { bandText, isFailed } from './ci-model'

type Options = BandOptions & CiOptions & { ciBand?: boolean }


export const register: Register = (on, options: Options) => {
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
