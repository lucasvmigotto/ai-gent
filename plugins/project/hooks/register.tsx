// project:checkpoint in the session: a band above the prompt with the holds
// the hand-off file records, and a question before a push, merge, tag or
// release while any hold is in force. The file stays the record; this only
// keeps it in view (Claude Code only; other agents read it through the skill).

import { atom, read, update } from 'claude-code'
import type { EngineInterface, Register } from 'claude-code'

import type { Checkpoint } from '../types'
import { CHECKPOINT, gated, parse } from './checkpoint'

// Change the tag when Checkpoint's fields change: it declines what an earlier
// version stored, which outlives a reload.
const checkpoint = atom({ plugin: 'project', key: 'checkpoint' } as const, null, {
  shape: 'checkpoint-1',
})
const isBandHidden = atom({ plugin: 'project', key: 'isBandHidden' } as const, false)

type Options = { holdsBand?: boolean; holdsGate?: boolean }

// The repository the session runs in; a reload finds it again.
let root: string | null = null

async function repoRoot($: EngineInterface): Promise<string> {
  if (root !== null) return root
  const cwd = await $.session.cwd()
  const top = await $.process
    .run(['git', 'rev-parse', '--show-toplevel'], { timeoutMs: 5000 })
    .catch(() => undefined)
  root = top?.exitCode === 0 && top.stdout.trim() ? top.stdout.trim() : cwd
  return root
}

// Re-reads the file when it changed; clears the band when it is gone.
async function refresh($: EngineInterface): Promise<Checkpoint | null> {
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

export const register: Register = (on, options: Options) => {
  const showBand = options.holdsBand !== false
  const askOnGated = options.holdsGate !== false

  on('session.start', async ($, e, next) => {
    const started = await next(e)
    await refresh($).catch(() => undefined)
    return started
  })

  on('turn.complete', async ($, e, next) => {
    await refresh($).catch(() => undefined)
    return next(e)
  })

  // A hold applies until the user lifts it, and a go-ahead from an earlier
  // session doesn't carry over: ask, naming the holds, before the step runs.
  on('tool.call', { tool: 'Bash' }, async ($, e, next) => {
    const kind = askOnGated ? gated(e.command) : null
    if (kind === null) return next(e)
    const current = await refresh($)
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

  on('ui.render', { component: 'AbovePrompt' }, async ($, e, next) => {
    const current = await read($, checkpoint)
    const isQuiet =
      !showBand ||
      e.props.hasSurvey ||
      current === null ||
      current.holds.length === 0 ||
      (await read($, isBandHidden))
    if (isQuiet) return next(e)

    const { Box, Button, Text } = $.ui.resolve(e)
    const width = Math.max(20, e.props.bodyColumns - 12)
    const shown = current.holds.slice(0, Math.max(1, Math.min(3, e.props.maxRows - 1)))
    const more = current.holds.length - shown.length

    return (
      <Box flexDirection="column">
        <Box>
          <Text bold>Holds</Text>
          <Text dimColor>
            {' '}
            {CHECKPOINT}
            {current.updated ? ` · updated ${current.updated}` : ''}
            {current.isStale ? ' · older than the last commit' : ''}
            {' '}
          </Text>
          <Button key="hide" label="Hide" onPress={() => update($, isBandHidden, () => true)} />
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
}
