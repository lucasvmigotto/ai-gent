// The container and disk band's calls (specs/025-container-disk-band): run the
// plugin's own leftovers.sh --json, keep what it says in $.state and record the
// band for band priority. The wording is in containers-model.ts. The only
// process started is leftovers.sh (a read: it removes, stops and builds nothing);
// the mod runs no engine command of its own.

import { atom, read, update } from 'claude-code'
import type { EngineInterface, Register } from 'claude-code'

import type { ContainersState } from '../types'
import { recordBands } from './bands'
import { TRIGGER, commandText, priorityOf, readReport } from './containers-model'
import type { Report } from './containers-model'

export type Options = { containersBand?: boolean; containersRefreshSeconds?: number; containersCommand?: boolean }

// Change the tag when ContainersState's fields change: it declines what an earlier version stored.
const stateAtom = atom({ plugin: 'devcontainer', key: 'containers.state' } as const, null, { shape: 'containers-1' })

// The band's id in the `bands` atom.
export const CONTAINERS = 'containers'

const TIMEOUT_MS = 10_000
// A call that built or removed something reads again, but not twice within this.
const TRIGGER_GAP_MS = 1_000
const DEFAULT_REFRESH_SECONDS = 120

type Ran = { stdout: string; exitCode: number } | 'missing' | 'timeout'

async function script($: EngineInterface, args: readonly string[]): Promise<Ran> {
  const path = `${$.plugin.root}/hooks/leftovers.sh`
  if (!(await $.fs.exists(path).catch(() => true))) return 'missing'
  const cwd = await $.session.cwd().catch(() => undefined)
  const out = await $.process.run([path, ...args], cwd === undefined ? { timeoutMs: TIMEOUT_MS } : { cwd, timeoutMs: TIMEOUT_MS }).catch(() => undefined)
  return out === undefined ? 'timeout' : { stdout: out.stdout, exitCode: out.exitCode }
}

function parsed(ran: Ran): { report: Report | null; problem: 'missing' | 'timeout' | 'unreadable' | null } {
  if (ran === 'missing' || ran === 'timeout') return { report: null, problem: ran }
  const report = ran.exitCode === 0 ? readReport(ran.stdout) : null
  return { report, problem: report === null ? 'unreadable' : null }
}

async function readOnce($: EngineInterface): Promise<void> {
  const { report, problem } = parsed(await script($, ['--json']))
  const readAt = await $.clock.now()
  await update($, stateAtom, (before): ContainersState => {
    if (report !== null) return { findings: report.findings, checked: report.checked, reason: report.reason, readAt, ok: true, problem: null }
    // A timeout keeps the last text; anything else is no report.
    if (problem === 'timeout' && before !== null) return { ...before, readAt, ok: false, problem }
    return { findings: [], checked: false, reason: null, readAt, ok: false, problem }
  })
}

// Whether the band is active (something to show) and its priority.
export async function recordContainersBand($: EngineInterface): Promise<void> {
  const state = await read($, stateAtom)
  const isActive = state !== null && state.checked && state.findings.length > 0
  await recordBands($, isActive ? [{ id: CONTAINERS, priority: priorityOf(state.findings) }] : [])
}

// Module-level: a second caller joins the read that is running.
let inflight: Promise<void> | undefined

// One read of the script unless the last one is newer than `gapMs`.
export function refresh($: EngineInterface, gapMs: number): Promise<void> {
  if (inflight !== undefined) return inflight
  const reading = (async () => {
    try {
      const before = await read($, stateAtom)
      if (before !== null && gapMs > 0 && (await $.clock.now()) - before.readAt < gapMs) return
      await readOnce($)
      await recordContainersBand($)
    } finally {
      inflight = undefined
    }
  })()
  inflight = reading
  return reading
}

export async function readContainers($: EngineInterface): Promise<ContainersState | null> {
  return read($, stateAtom)
}

const refreshGap = (options: Options): number => {
  const seconds = options.containersRefreshSeconds
  return (typeof seconds === 'number' && Number.isFinite(seconds) && seconds >= 0 ? seconds : DEFAULT_REFRESH_SECONDS) * 1000
}

// The first read is detached: the session's start is not held for it.
export function startReading($: EngineInterface): void {
  $.clock.after(0, () => void refresh($, 0).catch(() => undefined))
}

// The band's own events other than session.start, which register.tsx owns (the
// engine takes one matcher-less handler per event). A statement, like the CI band's.
export function containersHooks(on: Parameters<Register>[0], options: Options): void {
  on('turn.complete', async ($, e, next) => {
    await refresh($, refreshGap(options)).catch(() => undefined)
    return next(e)
  })

  on('tool.call', { tool: 'Bash' }, async ($, e, next) => {
    const ran = await next(e)
    const failed = ran.deny !== undefined || (ran as { isError?: boolean }).isError === true
    if (!failed && TRIGGER.test(String(e.command ?? ''))) await refresh($, TRIGGER_GAP_MS).catch(() => undefined)
    return ran
  }).catch(($, e, next) => next(e))
}

// /containers: a fresh read, then the script's own text when there is something to say.
export async function containersReport($: EngineInterface): Promise<string> {
  const { report, problem } = parsed(await script($, ['--json']))
  let text = ''
  if (report !== null && report.checked && report.findings.length > 0) {
    const ran = await script($, [])
    if (ran !== 'missing' && ran !== 'timeout' && ran.exitCode === 0) text = ran.stdout
  }
  return commandText(report, text, problem ?? 'unreadable')
}
