// The project plugin's Claude Code mod, two optional features that only make
// visible what the skills already record (other agents read it through them):
//  - project:checkpoint: a band above the prompt with the holds the hand-off
//    file records, and a question before a push, merge, tag or release while
//    any hold is in force.
//  - project:status: a pane with the pipeline's artifacts, feature statuses,
//    open items and a next-stage hint, redrawn as the files change.

import { atom, read, update } from 'claude-code'
import type { EngineInterface, Register } from 'claude-code'

import type { Checkpoint } from '../types'
import { bandsText, plan, recordBands } from './bands'
import type { Options as BandOptions } from './bands'
import { CHECKPOINT, gated, parse } from './checkpoint'
import { PANE, close, open, press, readExpanded, readIsOpen, readSnapshot, refresh as refreshPipeline, startPipeline } from './pipeline'
import { text as pipelineText, view } from './pipeline-view'
import type { Tone } from './pipeline-view'
import { repoRoot } from './repo'

// Change the tag when Checkpoint's fields change: it declines what an earlier
// version stored, which outlives a reload.
const checkpoint = atom({ plugin: 'project', key: 'checkpoint' } as const, null, {
  shape: 'checkpoint-1',
})
const isBandHidden = atom({ plugin: 'project', key: 'isBandHidden' } as const, false)

type Options = BandOptions & { holdsBand?: boolean; holdsGate?: boolean; pipelinePane?: boolean; pipelinePollSeconds?: number }

const COLOR: Partial<Record<Tone, string>> = { good: 'success', warn: 'warning', bad: 'error' }

// The id of the checkpoint band in the `bands` atom (priority hold).
const HOLDS = 'holds'

// Records whether the holds band is active: it has holds, and the person has not hidden it.
async function recordHolds($: EngineInterface, current: Checkpoint | null, isOn: boolean): Promise<void> {
  const isActive = isOn && current !== null && current.holds.length > 0 && !(await read($, isBandHidden))
  await recordBands($, isActive ? [{ id: HOLDS, priority: 'hold' }] : [])
}

// Re-reads the file when it changed; clears the band when it is gone.
async function readCheckpoint($: EngineInterface): Promise<Checkpoint | null> {
  const dir = await repoRoot($)
  const path = `${dir}/${CHECKPOINT}`
  const stat = await $.fs.stat(path).catch(() => undefined)
  if (stat?.kind !== 'file') {
    await update($, checkpoint, () => null)
    return null
  }
  const last = await $.process
    .run(['git', '-C', dir, 'log', '-1', '--format=%ct'], { timeoutMs: 5000 })
    .catch(() => undefined)
  const committed = Number(last?.stdout.trim()) * 1000
  const isStale = Number.isFinite(committed) && committed > stat.mtimeMs
  const known = await read($, checkpoint)
  if (known?.path === path && known.mtimeMs === stat.mtimeMs) {
    if (known.isStale !== isStale) await update($, checkpoint, () => ({ ...known, isStale }))
    return { ...known, isStale }
  }
  const fresh: Checkpoint = {
    path,
    ...parse(await $.fs.read(path)),
    isStale,
    mtimeMs: stat.mtimeMs,
  }
  await update($, checkpoint, () => fresh)
  return fresh
}

// Reads the file and records whether the holds band is active.
async function refresh($: EngineInterface, isOn: boolean): Promise<Checkpoint | null> {
  const current = await readCheckpoint($)
  await recordHolds($, current, isOn).catch(() => undefined)
  return current
}

export const register: Register = (on, options: Options) => {
  const showBand = options.holdsBand !== false
  const askOnGated = options.holdsGate !== false

  on('session.start', async ($, e, next) => {
    const started = await next(e)
    if (options.bandsCommand !== false) {
      await $.command.register({ name: 'bands', description: 'List the bands above the prompt, and which are hidden by the cap' }).catch(() => undefined)
    }
    await refresh($, showBand).catch(() => undefined)
    if (options.pipelinePane !== false) {
      const canDock = e.isInteractive && e.surface !== null && e.surface !== 'mobile'
      await startPipeline($, options.pipelinePollSeconds ?? 5, canDock).catch(() => undefined)
    }
    return started
  })

  on('turn.complete', async ($, e, next) => {
    await refresh($, showBand).catch(() => undefined)
    return next(e)
  })

  // A hold applies until the user lifts it, and a go-ahead from an earlier
  // session doesn't carry over: ask, naming the holds, before the step runs.
  on('tool.call', { tool: 'Bash' }, async ($, e, next) => {
    const kind = askOnGated ? gated(e.command) : null
    if (kind === null) return next(e)
    const current = await refresh($, showBand)
    if (current === null || current.holds.length === 0) return next(e)

    const holds = current.holds.map(hold => `- ${hold}`).join('\n')
    const answer = await $.ui
      .ask(
        `The checkpoint has ${current.holds.length === 1 ? 'a hold' : `${current.holds.length} holds`} in force:\n${holds}\nRun this ${kind} anyway?`,
        { header: 'Holds', options: ['Stop', `Run the ${kind}`] },
      )
      .catch(() => 'Stop')
    if (answer === `Run the ${kind}`) return next(e)
    return {
      deny: `Stopped: ${CHECKPOINT} has holds in force (${current.holds.join('; ')}). Ask the user whether a hold is lifted; update the checkpoint when it is.`,
    }
  }).catch(($, e, next) => next(e))

  // Composes with whatever band is beneath (the git plugin's unpulled-release band), so both show.
  on('ui.render', { component: 'AbovePrompt' }, async ($, e, next) => {
    const beneath = await next(e)
    const current = await read($, checkpoint)
    const isQuiet =
      !showBand ||
      e.props.hasSurvey ||
      current === null ||
      current.holds.length === 0 ||
      (await read($, isBandHidden))
    if (isQuiet) return beneath
    if (!(await plan($, HOLDS, options)).draws) return beneath

    const { Box, Button, Text } = $.ui.resolve(e)
    const width = Math.max(20, e.props.bodyColumns - 12)
    const shown = current.holds.slice(0, Math.max(1, Math.min(3, e.props.maxRows - 1)))
    const more = current.holds.length - shown.length

    return (
      <Box flexDirection="column">
        {beneath}
        <Box>
          <Text bold>Holds</Text>
          <Text dimColor>
            {' '}
            {CHECKPOINT}
            {current.updated ? ` · updated ${current.updated}` : ''}
            {current.isStale ? ' · older than the last commit' : ''}
            {' '}
          </Text>
          <Button key="hide" label="Hide" onPress={async () => {
              await update($, isBandHidden, () => true)
              await recordHolds($, current, showBand).catch(() => undefined)
            }}
          />
        </Box>
        {shown.map(hold => (
          <Text wrap="truncate-end">
            {'  '}
            {hold.length > width ? `${hold.slice(0, width - 1)}…` : hold}
          </Text>
        ))}
        {more > 0 ? <Text dimColor>{`  +${more} more`}</Text> : null}
      </Box>
    )
  })

  if (options.bandsCommand !== false) {
    on('command.run', { command: 'bands' }, async $ => ({ text: await bandsText($, options) }))
  }

  if (options.pipelinePane === false) return
  const seconds = options.pipelinePollSeconds ?? 5

  on('command.run', { command: 'pipeline-pane' }, async $ => {
    if (await open($, seconds, true)) return { text: 'Pipeline pane opened.' }
    const snapshot = await readSnapshot($)
    return { text: snapshot ? pipelineText(snapshot) : 'The pipeline could not be read.' }
  })

  on('ui.close', async ($, e, next) => {
    const closed = await next(e)
    if (e.id === PANE) await close($)
    return closed
  }).catch(($, e, next) => next(e))

  // A stage that just wrote an artifact or flipped a status shows at once,
  // without waiting for the next poll.
  on('tool.call', { tool: ['Edit', 'Write'] }, async ($, e, next) => {
    const ran = await next(e)
    const path = 'file_path' in e && typeof e.file_path === 'string' ? e.file_path : ''
    if ((path.includes('/docs/product/') || path.includes('/specs/')) && (await readIsOpen($))) {
      await refreshPipeline($).catch(() => undefined)
    }
    return ran
  }).catch(($, e, next) => next(e))

  on('ui.render', { component: 'Pane', requestId: 'pipeline' }, async ($, e, next) => {
    const snapshot = await readSnapshot($)
    const { Box, Button, Text } = $.ui.resolve(e)
    if (snapshot === null) return <Text dimColor>Reading the pipeline…</Text>

    const lines = view(snapshot, e.props.bodyColumns, await readExpanded($))
    return (
      <Box flexDirection="column">
        {lines.map(line => (
          <Box key={line.key}>
            <Text bold={line.tone === 'bold'} dimColor={line.tone === 'dim'} color={COLOR[line.tone]}>
              {line.text}
            </Text>
            {line.button ? (
              <Button key={`button-${line.button.id}`} label={line.button.label} onPress={() => press($, line.button?.id ?? 'status')} />
            ) : null}
          </Box>
        ))}
      </Box>
    )
  })
}
