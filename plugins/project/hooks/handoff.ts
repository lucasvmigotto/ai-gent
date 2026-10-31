// The hand-off nudge's calls: it reads the context figure, the checkpoint's
// standing in git and, when token_report.py sits beside the plugin, the caps,
// and toasts once per condition that the checkpoint is behind. It writes
// nothing, runs no skill and submits no prompt. The wording and thresholds are
// in handoff-model.ts; this file feeds them.

import { atom, read, update } from 'claude-code'
import type { EngineInterface } from 'claude-code'

import type { Checkpoint } from '../types'
import { capToast, compactToast, contextToast, isContextHigh, lowCaps, parseCaps } from './handoff-model'
import type { Standing } from './handoff-model'
import { repoRoot } from './repo'

// Change the tag when HandoffToasted's fields change: it declines what an
// earlier version stored, which outlives a reload.
const toasted = atom({ plugin: 'project', key: 'handoffToasted' } as const, { context: false, compact: false, caps: {}, lastBudgetAt: null }, {
  shape: 'handoff-1',
})

const BUDGET = '.ai-gent/budget.yaml'
const THROTTLE_MS = 5 * 60_000

export type HandoffOptions = { handoffNudge?: boolean; handoffNudgeContextPercent?: number; handoffNudgeCapPercent?: number }

// The checkpoint as 012's atom has it (register.tsx reads the file).
export type CheckpointReader = () => Promise<Checkpoint | null>

async function git($: EngineInterface, dir: string, args: string[]): Promise<string | null> {
  const run = await $.process.run(['git', '-C', dir, ...args], { timeoutMs: 5000 }).catch(() => undefined)
  return run?.exitCode === 0 ? run.stdout : null
}

// Read-only: the commits after the file, and whether the tree has changes.
async function standingNow($: EngineInterface, checkpoint: Checkpoint | null): Promise<Standing> {
  const dir = await repoRoot($)
  let commits: number | null = null
  if (checkpoint !== null) {
    const count = await git($, dir, ['rev-list', '--count', `--after=${new Date(checkpoint.mtimeMs).toISOString()}`, 'HEAD'])
    const n = Number(count?.trim())
    commits = count !== null && Number.isFinite(n) ? n : null
  }
  const status = await git($, dir, ['status', '--porcelain'])
  return {
    checkpoint: checkpoint === null ? null : { isStale: checkpoint.isStale, mtimeMs: checkpoint.mtimeMs },
    commits,
    isDirty: status === null ? null : status.trim() !== '',
    now: await $.clock.now(),
  }
}

// At turn end: toast once when the context passes the threshold, and look at the
// caps (detached, throttled) when the script and a budget are there.
export async function nudgeAtTurnEnd($: EngineInterface, options: HandoffOptions, checkpointOf: CheckpointReader): Promise<void> {
  if (options.handoffNudge === false) return
  const flags = await read($, toasted)
  if (!flags.context) {
    const usage = await $.session.usage().catch(() => undefined)
    const percent = usage?.context.percent
    if (typeof percent === 'number' && isContextHigh(percent, options.handoffNudgeContextPercent ?? 80)) {
      await update($, toasted, f => ({ ...f, context: true }))
      $.ui.toast(contextToast(Math.round(percent), await standingNow($, await checkpointOf())), { timeoutMs: 15_000 })
    }
  }
  await scheduleCaps($, options, checkpointOf)
}

// Before a compaction (manual or auto): the same toast, once, without a percent.
export async function nudgeBeforeCompact($: EngineInterface, trigger: string, options: HandoffOptions, checkpointOf: CheckpointReader): Promise<void> {
  if (options.handoffNudge === false || (trigger !== 'manual' && trigger !== 'auto')) return
  const flags = await read($, toasted)
  if (flags.context || flags.compact) return
  await update($, toasted, f => ({ ...f, compact: true }))
  $.ui.toast(compactToast(await standingNow($, await checkpointOf())), { timeoutMs: 15_000 })
}

// `budget` reads a week of transcripts: it runs detached, at most every five
// minutes, and only where token_report.py is found beside the plugin and a
// budget file exists. Otherwise the check is off, silently.
async function scheduleCaps($: EngineInterface, options: HandoffOptions, checkpointOf: CheckpointReader): Promise<void> {
  const script = `${$.plugin.root}/../agents/scripts/token_report.py`
  if (!(await $.fs.exists(script).catch(() => false))) return
  const dir = await repoRoot($)
  if (!(await $.fs.exists(`${dir}/${BUDGET}`).catch(() => false))) return
  const now = await $.clock.now()
  const last = (await read($, toasted)).lastBudgetAt
  if (last !== null && now - last < THROTTLE_MS) return
  await update($, toasted, f => ({ ...f, lastBudgetAt: now }))
  $.clock.after(0, () => checkCaps($, script, dir, options, checkpointOf).catch(() => undefined))
}

async function checkCaps($: EngineInterface, script: string, dir: string, options: HandoffOptions, checkpointOf: CheckpointReader): Promise<void> {
  const run = await $.process.run(['python3', script, 'budget', '--json', '--projected=0'], { cwd: dir, timeoutMs: 120_000 }).catch(() => undefined)
  const caps = run?.exitCode === 0 ? parseCaps(run.stdout) : null
  if (caps === null) return
  const flags = await read($, toasted)
  const fresh = lowCaps(caps, options.handoffNudgeCapPercent ?? 10).filter(cap => !flags.caps[cap.name])
  if (fresh.length === 0) return
  await update($, toasted, f => ({ ...f, caps: { ...f.caps, ...Object.fromEntries(fresh.map(cap => [cap.name, true])) } }))
  const standing = await standingNow($, await checkpointOf())
  for (const cap of fresh) $.ui.toast(capToast(cap, standing), { timeoutMs: 15_000 })
}
