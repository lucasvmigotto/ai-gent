// The CI watch band's calls (specs/023-ci-watch-band): after a successful
// `git push`, find that commit's runs through reporun (read only), keep them in
// $.state, and toast once per failing run. The wording is in ci-model.ts. The
// poll is the module's own bounded back-off (never a model turn); everything
// it spawns is a read: git rev-parse, reporun detect --json, and
// `reporun cli github -- run list --commit <sha>` without --write.

import { atom, read, update } from 'claude-code'
import type { EngineInterface, Register, Timer } from 'claude-code'

import type { CiDetect, CiRuns, CiTimer } from '../types'
import { recordBands } from './bands'
import { delayMs, detectArgv, gate, isDone, isPush, newFailures, parseDetect, parseRuns, priorityOf, runListArgv, toastText } from './ci-model'

export type CiOptions = { ciBandPollSeconds?: number; ciBandMinutes?: number; ciBandUsePass?: boolean }

// Change a tag when the atom's fields change: it declines what an earlier version stored.
const runsAtom = atom({ plugin: 'repo', key: 'ciRuns' } as const, null, { shape: 'ci-1' })
const toastedAtom = atom({ plugin: 'repo', key: 'ciToasted' } as const, [] as number[])
const hiddenAtom = atom({ plugin: 'repo', key: 'ciHidden' } as const, null as string | null)
const timerAtom = atom({ plugin: 'repo', key: 'ciTimer' } as const, null as CiTimer | null)
const detectAtom = atom({ plugin: 'repo', key: 'ciDetect' } as const, null, { shape: 'ci-detect-1' })

// The band's id in the `bands` atom (priority failure or normal).
export const CI = 'ci'

// A finished band stays this long, then clears.
const CLEAR_MS = 60_000
// Consecutive failed reads before the polling stops.
const MAX_FAILURES = 3

const SIGNIN = 'CI band is off: gh is not installed or not signed in. gh auth login would add the CI state of your pushes here'
const PASS = "CI band is off: the platform token is in pass and reading it may raise a pinentry prompt; turn on ciBandUsePass in /config to allow it, or sign in with gh auth login"
const STOPPED = 'CI band stopped after three failed reads; /devsecops:watch can follow the runs'

type Run = { exitCode: number; stdout: string } | undefined

async function run($: EngineInterface, argv: readonly string[], cwd: string, timeoutMs: number): Promise<Run> {
  const out = await $.process.run([...argv], { cwd, timeoutMs }).catch(() => undefined)
  return out === undefined ? undefined : { exitCode: out.exitCode, stdout: out.stdout }
}

// Whether the band is active (something to show, not hidden) and its priority.
export async function recordCiBand($: EngineInterface): Promise<void> {
  const ci = await read($, runsAtom)
  const hidden = await read($, hiddenAtom)
  const isActive = ci !== null && hidden !== ci.sha
  await recordBands($, isActive ? [{ id: CI, priority: priorityOf(ci.runs) }] : [])
}

export async function readCi($: EngineInterface): Promise<{ ci: CiRuns; hidden: boolean } | null> {
  const ci = await read($, runsAtom)
  return ci === null ? null : { ci, hidden: (await read($, hiddenAtom)) === ci.sha }
}

type Live = { timer: Timer | undefined; noted: Set<string>; isInteractive: boolean }

function stop(live: Live): void {
  live.timer?.cancel()
  live.timer = undefined
}

// The band goes: the polling ends and the runs are forgotten.
async function clear($: EngineInterface, live: Live, sha: string): Promise<void> {
  stop(live)
  await update($, timerAtom, () => null)
  const ci = await read($, runsAtom)
  if (ci?.sha === sha) await update($, runsAtom, () => null)
  await recordCiBand($).catch(() => undefined)
}

function schedule($: EngineInterface, live: Live, options: CiOptions, root: string, cwd: string, ms: number, sha: string): void {
  stop(live)
  live.timer = $.clock.after(ms, () => void poll($, live, options, root, cwd, sha).catch(() => undefined))
}

// One read of the runs, then the next one or the end. Every exit is a decision
// about the band's own state; nothing here writes anywhere but $.state.
async function poll($: EngineInterface, live: Live, options: CiOptions, root: string, cwd: string, sha: string): Promise<void> {
  const timer = await read($, timerAtom)
  const ci = await read($, runsAtom)
  // Superseded by a newer push, hidden, or already ended: no read.
  if (timer === null || ci === null || ci.sha !== sha || (await read($, hiddenAtom)) === sha) return
  if ((await $.clock.now()) >= timer.stopAt) return clear($, live, sha)

  const out = await run($, runListArgv(root, sha), cwd, 60_000)
  const runs = out !== undefined && out.exitCode === 0 ? parseRuns(out.stdout) : null
  // Another push may have taken the band while the call ran.
  if ((await read($, runsAtom))?.sha !== sha) return

  const delay = delayMs(timer.phase, options.ciBandPollSeconds ?? 30)
  if (runs === null) {
    const failures = timer.failures + 1
    await update($, detectAtom, () => null) // the gate is checked again after a failed read
    if (failures >= MAX_FAILURES) {
      stop(live)
      await update($, timerAtom, () => null)
      $.ui.toast(STOPPED)
      return
    }
    await update($, timerAtom, () => ({ ...timer, failures, phase: timer.phase + 1 }))
    return schedule($, live, options, root, cwd, delay, sha)
  }

  await update($, runsAtom, current => (current === null ? null : { ...current, runs }))
  const toasted = await read($, toastedAtom)
  const fresh = newFailures(runs, toasted)
  if (fresh.length > 0) {
    await update($, toastedAtom, current => [...current, ...fresh.map(r => r.id)])
    for (const failed of fresh) $.ui.toast(toastText(failed, sha), { timeoutMs: 12_000 })
  }
  await recordCiBand($).catch(() => undefined)

  // An empty list is final only at the second look: a run may take a moment to appear.
  if (isDone(runs) && (runs.length > 0 || timer.phase >= 1)) {
    stop(live)
    await update($, timerAtom, () => null)
    live.timer = $.clock.after(CLEAR_MS, () => void clear($, live, sha).catch(() => undefined))
    return
  }
  await update($, timerAtom, () => ({ ...timer, failures: 0, phase: timer.phase + 1 }))
  schedule($, live, options, root, cwd, delay, sha)
}

// What detect says, from the atom or one `reporun detect --json` per session.
async function detected($: EngineInterface, root: string, cwd: string): Promise<CiDetect | null> {
  const cached = await read($, detectAtom)
  if (cached !== null) return cached
  const out = await run($, detectArgv(root), cwd, 30_000)
  const parsed = out !== undefined && out.exitCode === 0 ? parseDetect(out.stdout) : null
  await update($, detectAtom, () => parsed)
  return parsed
}

function note($: EngineInterface, live: Live, key: string, text: string): void {
  if (live.noted.has(key)) return
  live.noted.add(key)
  $.ui.toast(text, { timeoutMs: 10_000 })
}

// A push that exited 0: gate, find the commit, take over the band and start the reads.
async function onPush($: EngineInterface, live: Live, options: CiOptions): Promise<void> {
  const cwd = await $.session.cwd()
  if (!(await $.fs.exists(`${cwd}/.github/workflows`).catch(() => false))) return
  const root = $.plugin.root
  const verdict = gate(await detected($, root, cwd), options.ciBandUsePass === true)
  if (verdict === 'signin') return note($, live, 'signin', SIGNIN)
  if (verdict === 'pass') return note($, live, 'pass', PASS)
  if (verdict !== 'read') return

  const sha = (await run($, ['git', 'rev-parse', 'HEAD'], cwd, 10_000))
  const branch = (await run($, ['git', 'rev-parse', '--abbrev-ref', 'HEAD'], cwd, 10_000))
  if (sha?.exitCode !== 0 || !/^[0-9a-f]{7,64}$/.test(sha.stdout.trim())) return
  const now = await $.clock.now()
  const minutes = options.ciBandMinutes ?? 15
  stop(live)
  await update($, runsAtom, () => ({ sha: sha.stdout.trim(), branch: branch?.exitCode === 0 ? branch.stdout.trim() : 'HEAD', runs: [], startedAt: now }))
  await update($, hiddenAtom, () => null)
  await update($, timerAtom, () => ({ phase: 0, failures: 0, stopAt: now + (Number.isFinite(minutes) && minutes > 0 ? minutes : 15) * 60_000 }))
  // The first read is detached: the push's result is not held for it.
  schedule($, live, options, root, cwd, 0, sha.stdout.trim())
}

// The pending timer and the once-per-session notes. Module-level because the
// engine reads `ciHooks(on, …)` as a statement, whose value nothing may keep.
const live: Live = { timer: undefined, noted: new Set(), isInteractive: true }

// The Hide button: the band stays hidden for this push and its polling ends.
export async function hideBand($: EngineInterface): Promise<void> {
  const ci = await read($, runsAtom)
  if (ci === null) return
  stop(live)
  await update($, hiddenAtom, () => ci.sha)
  await update($, timerAtom, () => null)
  await recordCiBand($).catch(() => undefined)
}

// The band's own events (a statement in register.tsx, like the agents plugin's `landing`).
export function ciHooks(on: Parameters<Register>[0], options: CiOptions): void {
  on('session.start', async ($, e, next) => {
    live.isInteractive = (e as { isInteractive?: boolean }).isInteractive !== false
    stop(live)
    live.noted.clear()
    for (const reset of [update($, runsAtom, () => null), update($, timerAtom, () => null), update($, detectAtom, () => null), update($, toastedAtom, () => []), update($, hiddenAtom, () => null)]) {
      await reset.catch(() => undefined)
    }
    await recordCiBand($).catch(() => undefined)
    return next(e)
  })

  on('tool.call', { tool: 'Bash' }, async ($, e, next) => {
    const ran = await next(e)
    const failed = ran.deny !== undefined || (ran as { isError?: boolean }).isError === true
    if (!failed && live.isInteractive && isPush(String(e.command ?? ''))) {
      await onPush($, live, options).catch(() => undefined)
    }
    return ran
  }).catch(($, e, next) => next(e))
}
