// The devcontainer plugin's Claude Code mod: the container and disk band
// (specs/025), the findings of leftovers.sh --json kept above the prompt and
// refreshed after container work, and /containers. Read only: nothing here
// removes, stops or builds anything. Each is gated on its own option
// (containersBand, containersCommand); one session.start handler serves both.
// The session-start report is shown by hooks/session-start.sh itself.

import type { Register } from 'claude-code'

import { plan } from './bands'
import type { Options as BandOptions } from './bands'
import { CONTAINERS, containersHooks, containersReport, readContainers, startReading } from './containers'
import type { Options as ContainersOptions } from './containers'
import { bandText } from './containers-model'

type Options = BandOptions & ContainersOptions

export const register: Register = (on, options: Options) => {
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
