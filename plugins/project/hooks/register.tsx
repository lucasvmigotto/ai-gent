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
import { allBands, bandsText, declareLeader, folding, plan, recordBands } from './bands'
import type { Options as BandOptions } from './bands'
import { CHECKPOINT, gated, parse } from './checkpoint'
import { beforeWrite, nextStageAtTurnEnd, noteWrite, seedNextStage } from './nextstage'
import type { NextStageOptions } from './nextstage'
import { contextExtras, contextTail, readUsage, recordContext } from './context'
import { recordLimits } from './usage'
import type { ContextOptions } from './context'
import { nudgeAtTurnEnd, nudgeBeforeCompact } from './handoff'
import type { HandoffOptions } from './handoff'
import { PANE, close, open, press, readExpanded, readIsOpen, readPaneState, readSnapshot, refresh as refreshPipeline, startPipeline } from './pipeline'
import { drawDashboard } from './pipeline-draw'
import { build, text as pipelineText, view } from './pipeline-view'
import type { Tone } from './pipeline-view'
import { lines as guardLines, clampLimit, summarize } from './guards'
import { readGuards } from './guards-read'
import { repoRoot } from './repo'
import { drawWorkspace } from './workspace'
import { workspaceView } from './workspace-model'

// Change the tag when Checkpoint's fields change: it declines what an earlier
// version stored, which outlives a reload.
const checkpoint = atom({ plugin: 'project', key: 'checkpoint' } as const, null, {
  shape: 'checkpoint-1',
})
const isBandHidden = atom({ plugin: 'project', key: 'isBandHidden' } as const, false)
const isWorkspaceOpen = atom({ plugin: 'project', key: 'workspaceExpanded' } as const, false)

type Options = BandOptions & HandoffOptions & NextStageOptions & ContextOptions & { workspaceLine?: boolean; holdsBand?: boolean; holdsGate?: boolean; pipelinePane?: boolean; pipelinePollSeconds?: number; pipelineLayout?: string; pipelineAscii?: boolean; pipelineMaxRows?: number; guardLogLimit?: number; guardsPane?: boolean }

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

const GUARDS_PANE = 'guards'

// Declares /guards and /guards-pane. Never opens the pane by itself.
async function registerGuards($: EngineInterface): Promise<void> {
  await $.command.register({ name: 'guards', description: 'List the last guard decisions and the entries that fired most' }).catch(() => undefined)
  await $.command.register({ name: 'guards-pane', description: 'Dock the guard decisions list in a pane' }).catch(() => undefined)
}

// Records the context figure (when its meter is on) and the rate-limit windows from one usage read.
async function recordUsage($: EngineInterface, usage: Awaited<ReturnType<typeof readUsage>>, withContext: boolean): Promise<void> {
  if (withContext) await recordContext($, usage?.context.percent)
  await recordLimits($, usage?.rateLimits)
}

export const register: Register = (on, options: Options) => {
  const showWorkspace = options.workspaceLine !== false
  const showMeter = options.contextMeter !== false
  const showUsage = options.usageMeter !== false
  const readsUsage = showMeter || showUsage
  const showBand = options.holdsBand !== false
  const askOnGated = options.holdsGate !== false

  on('session.start', async ($, e, next) => {
    const started = await next(e)
    if (options.bandsCommand !== false) {
      await $.command.register({ name: 'bands', description: 'List the bands above the prompt, and which are hidden by the cap' }).catch(() => undefined)
    }
    await registerGuards($)
    await refresh($, showBand).catch(() => undefined)
    await declareLeader($, showWorkspace).catch(() => undefined)
    if (readsUsage) await recordUsage($, await readUsage($), showMeter).catch(() => undefined)
    seedNextStage($, options)
    if (options.pipelinePane !== false) {
      const canDock = e.isInteractive && e.surface !== null && e.surface !== 'mobile'
      await startPipeline($, options.pipelinePollSeconds ?? 5, canDock).catch(() => undefined)
    }
    return started
  })

  on('turn.complete', async ($, e, next) => {
    const current = await refresh($, showBand).catch(() => null)
    await declareLeader($, showWorkspace).catch(() => undefined)
    // One read of the context figure serves the meter and the nudge (specs/031-session-chrome).
    const usage = readsUsage ? await readUsage($) : undefined
    const known = readsUsage && options.handoffNudge !== false ? { percent: usage?.context.percent } : undefined
    if (readsUsage) await recordUsage($, usage, showMeter).catch(() => undefined)
    // The hand-off nudge (spec 021): a toast, once, when the context is nearly full or a cap runs low.
    await nudgeAtTurnEnd($, options, async () => current ?? (await readCheckpoint($)), known).catch(() => undefined)
    // The next-stage toast (spec 027): once per turn, when a pipeline file was written.
    await nextStageAtTurnEnd($, options).catch(() => undefined)
    return next(e)
  })

  // Before a compaction the summary loses what the checkpoint keeps: say so, once,
  // and pass the event on unchanged, whatever the trigger.
  on('session.compact', async ($, e, next) => {
    await nudgeBeforeCompact($, e.trigger, options, () => readCheckpoint($)).catch(() => undefined)
    return next(e)
  }).catch(($, e, next) => next(e))

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
    if (e.props.hasSurvey) return beneath
    const current = await read($, checkpoint)
    const isHoldsQuiet = !showBand || current === null || current.holds.length === 0 || (await read($, isBandHidden))
    const holds = isHoldsQuiet || !(await plan($, HOLDS, options)).draws ? null : current

    // The bands that carry a summary, folded into one line while this plugin leads (never a hold or a failure).
    const view =
      showWorkspace && !e.props.isWorking && (await folding($))
        ? workspaceView(await allBands($), {
            columns: e.props.bodyColumns,
            ascii: options.chromeAscii === true,
            extras: await contextExtras($, options, e.props.bodyColumns - 2),
          })
        : null
    const workspace =
      view === null
        ? null
        : drawWorkspace($, e, view, {
            isOpen: await read($, isWorkspaceOpen),
            ascii: options.chromeAscii === true,
            onToggle: () => update($, isWorkspaceOpen, open => !open),
          })
    if (holds === null) {
      if (workspace === null) return beneath
      const { Box } = $.ui.resolve(e)
      return (
        <Box flexDirection="column">
          {beneath}
          {workspace}
        </Box>
      )
    }

    const { Box, Button, Text } = $.ui.resolve(e)
    const width = Math.max(20, e.props.bodyColumns - 12)
    const shown = holds.holds.slice(0, Math.max(1, Math.min(3, e.props.maxRows - 1)))
    const more = holds.holds.length - shown.length

    return (
      <Box flexDirection="column">
        {beneath}
        {workspace}
        <Box>
          <Text bold>Holds</Text>
          <Text dimColor>
            {' '}
            {CHECKPOINT}
            {holds.updated ? ` · updated ${holds.updated}` : ''}
            {holds.isStale ? ' · older than the last commit' : ''}
            {' '}
          </Text>
          <Button key="hide" label="Hide" onPress={async () => {
              await update($, isBandHidden, () => true)
              await recordHolds($, holds, showBand).catch(() => undefined)
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

  // The usage bar at the end of the prompt's hint line: the engine's own hint stays and the meter is its
  // `tail` (dim, cut where the row ends). The five-hour window (usageMeter) with the week beside it when
  // wide, else the context (contextMeter). Opt-in: it adds to a line the engine owns.
  if (readsUsage) {
    on('ui.render', { component: 'PromptHint' }, async ($, e, next) => {
      const tail = await contextTail($, options, e.viewport?.columns, [...e.props.hint].length, e.props.isWorking)
      return next(tail === null ? e : { ...e, props: { ...e.props, tail } })
    })
  }

  // The account's rate-limit windows, pushed after each turn and when a window moves a whole point:
  // no polling, no extra usage read. A missing or invalid window clears.
  if (readsUsage) {
    on('session.measure', async ($, e, next) => {
      await recordLimits($, e.rateLimits).catch(() => undefined)
      return next(e)
    }).catch(($, e, next) => next(e))
  }

  if (options.bandsCommand !== false) {
    on('command.run', { command: 'bands' }, async $ => ({ text: await bandsText($, options) }))
  }

  // A stage that just wrote an artifact or flipped a status shows at once in the
  // open pane, without waiting for the next poll, and is noted for the next-stage
  // toast, which speaks at the turn's end.
  on('tool.call', { tool: ['Edit', 'Write'] }, async ($, e, next) => {
    const path = 'file_path' in e && typeof e.file_path === 'string' ? e.file_path : ''
    const isWatched = path.includes('/docs/product/') || path.includes('/specs/')
    const isNew = await beforeWrite($, options, path, e.tool === 'Write')
    const ran = await next(e)
    if (!isWatched) return ran
    await noteWrite($, options, path, isNew).catch(() => undefined)
    if (options.pipelinePane !== false && (await readIsOpen($))) await refreshPipeline($).catch(() => undefined)
    return ran
  }).catch(($, e, next) => next(e))

  // The guard log (spec 024): /guards and /guards-pane over the guard plugins' atoms.
  on('command.run', { command: 'guards' }, async ($, e) => {
    const { rows, missing } = await readGuards($)
    return { text: guardLines(summarize(rows, clampLimit(e.args, clampLimit(options.guardLogLimit))), missing).join('\n') }
  })

  on('command.run', { command: 'guards-pane' }, async $ => {
    if (options.guardsPane === false) return { text: 'The guards pane is off (guardsPane); /guards prints the list.' }
    const placed = await $.ui.open({ id: GUARDS_PANE, title: 'Guards', focus: true })
    if (placed.isPlaced) return { text: 'Guards pane opened.' }
    const { rows, missing } = await readGuards($)
    return { text: ['no pane here — /guards prints the list', ...guardLines(summarize(rows, clampLimit(options.guardLogLimit)), missing)].join('\n') }
  })

  on('ui.render', { component: 'Pane', requestId: 'guards' }, async ($, e) => {
    const { Box, Text } = $.ui.resolve(e)
    const { rows, missing } = await readGuards($)
    const width = Math.max(20, e.props.bodyColumns)
    return (
      <Box flexDirection="column">
        {guardLines(summarize(rows, clampLimit(options.guardLogLimit)), missing, width).map((line, i) => (
          <Text key={i} wrap="truncate-end">{line}</Text>
        ))}
      </Box>
    )
  })

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

  on('ui.render', { component: 'Pane', requestId: 'pipeline' }, async ($, e, next) => {
    const snapshot = await readSnapshot($)
    const { Box, Button, Text } = $.ui.resolve(e)
    if (snapshot === null) return <Text dimColor>Reading the pipeline…</Text>

    const expanded = await readExpanded($)
    if (options.pipelineLayout !== 'compact') {
      const state = await readPaneState($)
      // The badge's verdict is the poll's: it is LIVE while the poll keeps a
      // positive interval and has checked, so `now` is the last check. Nothing
      // reads the clock while drawing.
      const model = build(snapshot, {
        columns: e.props.bodyColumns,
        expanded,
        filter: state.filter,
        checkedAt: state.checkedAt,
        now: state.checkedAt,
        pollMs: state.pollMs,
        ascii: options.pipelineAscii === true,
        maxRows: Math.max(1, Math.floor(options.pipelineMaxRows ?? 60)),
      })
      if (model.mode !== 'compact' && !model.isEmpty) {
        return drawDashboard($, e, model, {
          onStatus: () => press($, 'status'),
          onMarkers: () => press($, 'markers'),
          onFilter: filter => press($, 'filter', filter),
          onToggle: key => press($, 'toggle', key),
          onFill: command => press($, 'fill', command),
        })
      }
    }

    const lines = view(snapshot, e.props.bodyColumns, expanded)
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
