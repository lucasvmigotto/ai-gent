// The git plugin's Claude Code mod, optional features that only make visible
// what git:workflow and its guard already say:
//  - guard-ux (specs/015): why the guard denied or asked, and what to do
//    instead. hooks/guard.sh still decides.
//  - release readiness (specs/028): a band with the next release from
//    scripts/release.py --dry-run --json, on a chain branch.
//  - repo hygiene (specs/017): a band when local main or dev lag their remote
//    (after a release, "git pull --ff-only on both"), a toast for the update
//    notice only the model saw, and /release-preview. None of it fetches,
//    pulls, commits or tags.

import type { PreToolUseResult, Register } from 'claude-code'

import { decorate } from './guard-ux'
import { MOVES_REFS, sentence } from './hygiene-model'
import { bandsText, plan } from './bands'
import type { Options as BandOptions } from './bands'
import { MOVES_RELEASE } from './release-model'
import { hideRelease, refreshAndRecord, releaseLine } from './release'
import { readHygiene, recordHygieneBand, refreshHygiene, refreshHygieneThrottled, relayUpdate, releasePreview } from './hygiene'

type Options = BandOptions & { guardUx?: boolean; hygieneBand?: boolean; sessionToasts?: boolean; releasePreview?: boolean; releaseBand?: boolean }

// The band's id in the `bands` atom (priority normal).
const HYGIENE = 'hygiene'
// The release band's id (priority info).
const RELEASE = 'release'

// Reads the comparison again and records whether the band is active.
const refresh = async ($: Parameters<typeof refreshHygiene>[0]) => {
  await refreshHygiene($).catch(() => undefined)
  await recordHygieneBand($).catch(() => undefined)
}

export const register: Register = (on, options: Options) => {
  if (options.guardUx !== false) {
    on('classic.PreToolUse', async ($, e, next) => {
      const result = await next(e)
      const { result: decorated, notice } = decorate('git', result)
      if (notice) $.ui.toast(notice)
      return decorated as PreToolUseResult
    }).catch(($, e, next) => next(e))
  }

  const hasBand = options.hygieneBand !== false
  const hasPreview = options.releasePreview !== false
  const hasRelease = options.releaseBand !== false

  on('session.start', async ($, e, next) => {
    const started = await next(e)
    if (hasPreview) {
      await $.command.register({ name: 'release-preview', description: "Preview the next release: scripts/release.py --dry-run, which never releases" }).catch(() => undefined)
    }
    if (options.bandsCommand !== false) {
      await $.command.register({ name: 'bands', description: 'List the bands above the prompt, and which are hidden by the cap' }).catch(() => undefined)
    }
    if (hasBand) await refresh($)
    // Detached: the session start is not held for the dry run.
    if (hasRelease) void refreshAndRecord($)
    return started
  })

  if (hasBand || hasRelease) {
    on('turn.complete', async ($, e, next) => {
      if (hasBand) {
        await refreshHygieneThrottled($).catch(() => undefined)
        await recordHygieneBand($).catch(() => undefined)
      }
      if (hasRelease) void refreshAndRecord($)
      return next(e)
    })

    // A command that moved refs changes the comparison and the plan: read them again now.
    on('tool.call', { tool: 'Bash' }, async ($, e, next) => {
      const ran = await next(e)
      if (hasBand && MOVES_REFS.test(e.command)) await refresh($)
      if (hasRelease && MOVES_RELEASE.test(e.command)) await refreshAndRecord($)
      return ran
    }).catch(($, e, next) => next(e))

    // Composes with whatever band is beneath (the project plugin's checkpoint band).
    on('ui.render', { component: 'AbovePrompt' }, async ($, e, next) => {
      const beneath = await next(e)
      if (e.props.hasSurvey || e.props.isWorking) return beneath
      const hygiene = hasBand ? await readHygiene($) : null
      const ago = hygiene === null || hygiene.fetchedAt === null ? null : (await $.clock.now()) - hygiene.fetchedAt
      const text = hygiene === null ? null : sentence(hygiene.branches, e.props.bodyColumns, ago)
      const isBehind = (branch: string) => hygiene?.branches.some(state => state.name === branch && state.behind > 0) === true
      const release = hasRelease ? await releaseLine($, e.props.bodyColumns, isBehind) : null
      if (text === null && release === null) return beneath

      const { draws, more } = text === null ? { draws: false, more: null } : await plan($, HYGIENE, options)
      const shown = release === null ? { draws: false, more: null } : await plan($, RELEASE, options)
      const { Box, Button, Text } = $.ui.resolve(e)
      return (
        <Box flexDirection="column">
          {beneath}
          {text !== null && draws ? (
            <Text color="warning" wrap="truncate-end">
              {text}
            </Text>
          ) : null}
          {release !== null && shown.draws ? (
            <Box>
              <Text dimColor={release.isFailure} wrap="truncate-end">
                {release.text}
              </Text>
              <Text> </Text>
              <Button key="hide" label="Hide" onPress={() => hideRelease($)} />
            </Box>
          ) : null}
          {more === null ? shown.more === null ? null : <Text dimColor>{shown.more}</Text> : <Text dimColor>{more}</Text>}
        </Box>
      )
    })
  }

  // The update-check hook's line reaches only the model; this shows it to the person.
  if (options.sessionToasts !== false) {
    on('session.append', async ($, e, next) => {
      const origin = e.origin as { kind?: string; event?: string }
      if (e.door === 'hook-context' && origin.kind === 'hook' && origin.event === 'SessionStart') {
        await relayUpdate($, e.message).catch(() => undefined)
      }
      return next(e)
    }).catch(($, e, next) => next(e))
  }

  if (options.bandsCommand !== false) {
    on('command.run', { command: 'bands' }, async $ => ({ text: await bandsText($, options) }))
  }

  if (hasPreview) {
    on('command.run', { command: 'release-preview' }, async $ => ({ text: await releasePreview($) }))
  }
}
