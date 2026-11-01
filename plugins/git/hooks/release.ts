// The release-readiness band's reading (specs/028-release-readiness-band):
// everything that needs `$`. It runs `scripts/release.py --dry-run --json` once
// per distinct key (HEAD plus the tags merged into it) on a chain branch, and
// otherwise only reads git. It never fetches, pulls, commits, tags or pushes.
// The wording and the parse are in release-model.ts.

import { atom, read, update } from 'claude-code'
import type { EngineInterface } from 'claude-code'

import type { ReleasePlan } from '../types'
import { setBand } from './bands'
import { bandText, failureText, isReleaseBranch, keyOf, parsePlan, summaryOf } from './release-model'

// Change the tag when the plan's fields change: it declines what an earlier
// version stored, which outlives a reload.
export type Stored = (ReleasePlan & { branch: string }) | { failed: string } | null
const planAtom = atom({ plugin: 'git', key: 'releasePlan' } as const, null as Stored, { shape: 'release-1' })
const forAtom = atom({ plugin: 'git', key: 'releasePlanFor' } as const, null as string | null)
const hiddenAtom = atom({ plugin: 'git', key: 'releaseBandHidden' } as const, null as string | null)

// A read-only git call: its output, or null when it failed.
export async function git($: EngineInterface, args: readonly string[]): Promise<string | null> {
  const run = await $.process.run(['git', ...args], { timeoutMs: 10_000 }).catch(() => undefined)
  return run?.exitCode === 0 ? run.stdout.trim() : null
}

export type Ran =
  | { state: 'no-repo' | 'no-script' | 'no-start' }
  | { state: 'ran'; exitCode: number; stdout: string; stderr: string }

// The one call of scripts/release.py, for /release-preview and the band. Always
// --dry-run: nothing here releases. `flags` follow it (the band adds --json).
export async function runRelease($: EngineInterface, flags: readonly string[]): Promise<Ran> {
  const top = await git($, ['rev-parse', '--show-toplevel'])
  if (top === null) return { state: 'no-repo' }
  const script = `${top}/scripts/release.py`
  if (!(await $.fs.exists(script).catch(() => false))) return { state: 'no-script' }
  const run = await $.process.run(['python3', script, '--dry-run', ...flags], { cwd: top, timeoutMs: 60_000 }).catch(() => undefined)
  if (run === undefined) return { state: 'no-start' }
  return { state: 'ran', exitCode: run.exitCode, stdout: run.stdout, stderr: run.stderr }
}

// The band is gone: no plan, and no key, so the next chain branch computes again.
async function clearPlan($: EngineInterface): Promise<void> {
  await update($, planAtom, () => null)
  await update($, forAtom, () => null)
}

// Reads HEAD and the merged tags and runs the script only when either changed
// and HEAD is a chain branch. Safe to call often.
export async function refreshRelease($: EngineInterface): Promise<void> {
  const branch = await git($, ['rev-parse', '--abbrev-ref', 'HEAD'])
  if (branch === null || !isReleaseBranch(branch)) return clearPlan($)
  const head = await git($, ['rev-parse', 'HEAD'])
  const tags = await git($, ['tag', '--list', '--merged', 'HEAD'])
  if (head === null || tags === null) return
  const key = keyOf(head, tags)
  if ((await read($, forAtom)) === key) {
    await update($, planAtom, plan => (plan !== null && 'branch' in plan && plan.branch !== branch ? { ...plan, branch } : plan))
    return
  }
  // The key is taken before the run, so a second trigger meanwhile does not run it again.
  await update($, forAtom, () => key)
  const ran = await runRelease($, ['--json'])
  if (ran.state === 'no-repo' || ran.state === 'no-script') return clearPlan($)
  let next: Stored
  if (ran.state === 'no-start') next = { failed: failureText(null, '') }
  else if (ran.exitCode === 3) next = null
  else {
    const parsed = ran.exitCode === 0 ? parsePlan(ran.stdout) : null
    next = parsed === null ? { failed: failureText(ran.exitCode, ran.stderr) } : { ...parsed, branch }
  }
  await update($, planAtom, () => next)
}

// What the band draws, or null: the line, and whether it is a failure.
export async function releaseLine($: EngineInterface, columns: number, isBehind: (branch: string) => boolean): Promise<{ text: string; isFailure: boolean } | null> {
  const [plan, key, hidden] = [await read($, planAtom), await read($, forAtom), await read($, hiddenAtom)]
  if (plan === null || key === null || hidden === key) return null
  if ('failed' in plan) return { text: plan.failed, isFailure: true }
  return { text: bandText(plan, columns, isBehind(plan.branch)), isFailure: false }
}

export async function readPlan($: EngineInterface): Promise<Stored> {
  return read($, planAtom)
}

// The Hide button: the band stays away until the key changes.
export async function hideRelease($: EngineInterface): Promise<void> {
  const key = await read($, forAtom)
  await update($, hiddenAtom, () => key)
  await recordReleaseBand($)
}

// The band is active when there is something to draw (priority info).
export async function recordReleaseBand($: EngineInterface): Promise<void> {
  const [plan, key, hidden] = [await read($, planAtom), await read($, forAtom), await read($, hiddenAtom)]
  const isActive = plan !== null && key !== null && hidden !== key
  await setBand($, 'release', isActive ? 'info' : null, isActive && plan !== null && !('failed' in plan) ? summaryOf(plan) : undefined)
}

// After any trigger: read again, record the band. Never throws.
export async function refreshAndRecord($: EngineInterface): Promise<void> {
  await refreshRelease($).catch(() => undefined)
  await recordReleaseBand($).catch(() => undefined)
}
