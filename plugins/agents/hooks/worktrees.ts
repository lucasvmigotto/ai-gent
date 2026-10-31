// The worktree board's reading (specs/022-worktree-board): everything that needs
// `$`. Its only spawns are reads: `wt.py list --json`, `git remote get-url origin`,
// `gh auth status` and one `gh pr list` per remote refresh (never per row). It
// polls twice while the pane is open (local rows, then the slower pull request
// lookup), refreshes the local rows after a Bash call that ran wt.py, git commit
// or git merge, and writes nothing. State drawing reads is in `$.state` atoms;
// the module variables only guard against overlapping runs and hold the two
// timers, which a reload empties and session.start refills. Wording is in
// worktrees-model.ts.

import { atom, read, update } from 'claude-code'
import type { EngineInterface, Register, Timer } from 'claude-code'

import { carryPrs, joinPrs, parseGh, parseWt, text as boardText } from './worktrees-model'
import type { Board } from './worktrees-model'

export const PANE = 'worktrees'

const openAtom = atom({ plugin: 'agents', key: 'worktreeIsOpen' } as const, false)
const rowsAtom = atom({ plugin: 'agents', key: 'worktreeRows' } as const, [])
const errorAtom = atom({ plugin: 'agents', key: 'worktreeError' } as const, null)
const localAtAtom = atom({ plugin: 'agents', key: 'worktreeLocalAt' } as const, 0)
const remoteAtAtom = atom({ plugin: 'agents', key: 'worktreeRemoteAt' } as const, 0)
const ghAtom = atom({ plugin: 'agents', key: 'worktreeGh' } as const, { usable: false, platform: '' })

export type BoardOptions = { worktreeBoard?: boolean; worktreeBoardSeconds?: number; worktreeBoardRemoteSeconds?: number }
export type Intervals = { localMs: number; remoteMs: number }

const RUN_TIMEOUT_MS = 30_000
// A Bash command that changes what the board shows: wt.py, a commit or a merge.
const CHANGES = /(?:^|[\s;&|(])(?:\S*wt\.py|git(?:\s+-\S+(?:\s+\S+)?)*\s+(?:commit|merge))(?:\s|$)/

export const intervals = (options: BoardOptions): Intervals => ({
  localMs: Math.max(1, options.worktreeBoardSeconds ?? 15) * 1000,
  remoteMs: Math.max(1, options.worktreeBoardRemoteSeconds ?? 120) * 1000,
})

let isLocalRunning = false
let isRemoteRunning = false
let localTicker: Timer | null = null
let remoteTicker: Timer | null = null

const firstLine = (text: string): string => text.split('\n').find(line => line.trim() !== '')?.trim() ?? ''

// Reads wt.py's rows; a failure keeps the last good rows and records why.
// Overlapping runs are skipped. Returns whether the rows were read.
export async function refreshLocal($: EngineInterface): Promise<boolean> {
  if (isLocalRunning) return false
  isLocalRunning = true
  try {
    const started = await $.clock.now()
    await update($, localAtAtom, () => started)
    const cwd = await $.session.cwd()
    const run = await $.process.run(['python3', `${$.plugin.root}/scripts/wt.py`, 'list', '--json'], { cwd, timeoutMs: RUN_TIMEOUT_MS }).catch(() => undefined)
    const parsed = run?.exitCode === 0 ? parseWt(run.stdout) : null
    if (parsed === null) {
      const why = run === undefined ? 'it did not finish or could not start' : run.exitCode !== 0 ? firstLine(run.stderr) || `exit ${run.exitCode}` : 'its output was not the expected JSON'
      await update($, errorAtom, () => why)
      return false
    }
    await update($, errorAtom, () => null)
    await update($, rowsAtom, old => carryPrs(parsed, old))
    return true
  } finally {
    isLocalRunning = false
  }
}

// One `gh pr list` for every row; only when gh is usable on GitHub. A failure keeps what was read.
export async function refreshRemote($: EngineInterface): Promise<void> {
  if (isRemoteRunning || !(await read($, ghAtom)).usable) return
  isRemoteRunning = true
  try {
    const started = await $.clock.now()
    await update($, remoteAtAtom, () => started)
    const cwd = await $.session.cwd()
    const run = await $.process
      .run(['gh', 'pr', 'list', '--state', 'all', '--limit', '100', '--json', 'number,headRefName,state,isDraft,statusCheckRollup'], { cwd, timeoutMs: RUN_TIMEOUT_MS })
      .catch(() => undefined)
    const prs = run?.exitCode === 0 ? parseGh(run.stdout) : null
    if (prs !== null) await update($, rowsAtom, old => joinPrs(old, prs))
  } finally {
    isRemoteRunning = false
  }
}

const platformOf = (url: string): string => {
  const u = url.toLowerCase()
  if (u.includes('github')) return 'github'
  if (u.includes('gitlab')) return 'gitlab'
  if (u.includes('dev.azure.com') || u.includes('visualstudio.com')) return 'azure'
  if (u.includes('bitbucket')) return 'bitbucket'
  return 'other'
}

// The platform from origin's URL and, on GitHub, whether `gh auth status` exits 0 (its output is never read).
export async function detect($: EngineInterface): Promise<void> {
  const cwd = await $.session.cwd()
  const remote = await $.process.run(['git', 'remote', 'get-url', 'origin'], { cwd, timeoutMs: RUN_TIMEOUT_MS }).catch(() => undefined)
  const platform = remote?.exitCode === 0 ? platformOf(remote.stdout) : 'none'
  const auth = platform === 'github' ? await $.process.run(['gh', 'auth', 'status'], { cwd, timeoutMs: RUN_TIMEOUT_MS }).catch(() => undefined) : undefined
  await update($, ghAtom, () => ({ usable: auth?.exitCode === 0, platform }))
}

export async function snapshot($: EngineInterface): Promise<Board> {
  return { rows: await read($, rowsAtom), error: await read($, errorAtom), gh: await read($, ghAtom) }
}

// A floor, not a schedule: a run that happened sooner (a Bash-triggered one) counts.
export async function localTick($: EngineInterface, ms: number): Promise<void> {
  if (!(await read($, openAtom))) return stop()
  const now = await $.clock.now()
  if (now - (await read($, localAtAtom)) >= ms) await refreshLocal($).catch(() => undefined)
}

export async function remoteTick($: EngineInterface, ms: number): Promise<void> {
  if (!(await read($, openAtom))) return stop()
  const now = await $.clock.now()
  if (now - (await read($, remoteAtAtom)) >= ms) await refreshRemote($).catch(() => undefined)
}

function startPolls($: EngineInterface, every: Intervals): void {
  localTicker ??= $.clock.every(every.localMs, () => void localTick($, every.localMs))
  remoteTicker ??= $.clock.every(every.remoteMs, () => void remoteTick($, every.remoteMs))
}

export function stop(): void {
  localTicker?.cancel()
  remoteTicker?.cancel()
  localTicker = null
  remoteTicker = null
}

// `/worktree-board`: reads the rows, then opens the pane (focused) and starts the polls, or, with
// no worktrees, says so and opens nothing, or, where no surface draws it, answers with the text form.
export async function command($: EngineInterface, every: Intervals): Promise<{ text: string }> {
  await detect($).catch(() => undefined)
  await refreshLocal($).catch(() => undefined)
  const before = await snapshot($)
  if (before.rows.length === 0) {
    await update($, openAtom, () => false)
    stop()
    return { text: boardText(before) }
  }
  const placed = await $.ui.open({ id: PANE, title: 'Worktrees', focus: true }).catch(() => ({ isPlaced: false }))
  await refreshRemote($).catch(() => undefined)
  if (placed.isPlaced) {
    await update($, openAtom, () => true)
    startPolls($, every)
    return { text: 'Worktree board opened.' }
  }
  await update($, openAtom, () => false)
  stop()
  return { text: boardText(await snapshot($)) }
}

export async function close($: EngineInterface): Promise<void> {
  stop()
  await update($, openAtom, () => false)
}

// At the session's start: forget what was running, and poll again if the board was open before a reload.
export async function resume($: EngineInterface, every: Intervals): Promise<void> {
  stop()
  isLocalRunning = false
  isRemoteRunning = false
  if (await read($, openAtom)) startPolls($, every)
}

// After a Bash call that ran wt.py, git commit or git merge: the local rows again at once (not gh).
export async function bashRefresh($: EngineInterface, command: string): Promise<void> {
  if (!CHANGES.test(command) || !(await read($, openAtom))) return
  await refreshLocal($).catch(() => undefined)
}

// The module's own hook, beside landing.ts's: it only looks at what the call was.
export function worktreeBash(on: Parameters<Register>[0]): void {
  on('tool.call', { tool: 'Bash' }, async ($, e, next) => {
    const ran = await next(e)
    if (ran.deny === undefined) $.clock.after(0, () => void bashRefresh($, String(e.command ?? '')).catch(() => undefined))
    return ran
  })
}
