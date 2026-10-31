import type { ReleasePlan } from '../types'
import { CHAIN } from './hygiene-model'

// What the release-readiness band says (specs/028-release-readiness-band), from
// `scripts/release.py --dry-run --json`. Pure: no `$`. Nothing here computes a
// version or a bump; it reads what the script printed.

// The keys both release scripts print. scripts/consistency.py checks these lists
// equal the ones in scripts/release.py and shared/release-single.py.
export const JSON_KEYS = ['version', 'bump', 'since', 'targets', 'changelog'] as const
export const TARGET_KEYS = ['kind', 'label', 'from', 'to', 'bump'] as const

const BUMPS = ['major', 'minor', 'patch']

// Below this many columns the band reads `next 2.23.0 (minor)`.
const WIDE = 60

const isRecord = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null && !Array.isArray(value)

// The plan, or null when stdout is not the expected JSON. `plugins` counts only
// targets of kind "plugin" (the CHANGELOG line is not a target).
export function parsePlan(stdout: string): ReleasePlan | null {
  let value: unknown
  try {
    value = JSON.parse(stdout)
  } catch {
    return null
  }
  if (!isRecord(value) || !JSON_KEYS.every(key => key in value)) return null
  const { version, bump, since, targets } = value
  if (typeof version !== 'string' || typeof bump !== 'string' || !BUMPS.includes(bump)) return null
  if (since !== null && typeof since !== 'string') return null
  if (!Array.isArray(targets) || !targets.every(item => isRecord(item) && TARGET_KEYS.every(key => typeof item[key] === 'string'))) return null
  const plugins = targets.filter(item => (item as Record<string, unknown>).kind === 'plugin').length
  return { version, bump: bump as ReleasePlan['bump'], since, plugins }
}

// The band's line. `isBehind`: the local branch lags origin, so the plan reads stale history.
export function bandText(plan: ReleasePlan, columns: number, isBehind: boolean): string {
  if (columns < WIDE) return `next ${plan.version} (${plan.bump})`
  const since = plan.since === null ? 'first release' : `since ${plan.since}`
  const count = plan.plugins > 0 ? ` · ${plan.plugins} plugin${plan.plugins === 1 ? '' : 's'} bump` : ''
  const stale = isBehind ? ' (local branch is behind origin)' : ''
  return `next release ${plan.version} (${plan.bump}, ${since})${count}${stale} — /release-preview for the plan`
}

// The dim line for a run that gave no plan: exit code (null: never started) and stderr.
export function failureText(exitCode: number | null, stderr: string): string {
  if (exitCode === null) return 'release preview failed: scripts/release.py could not be started'
  if (exitCode === 0) return 'release preview failed: scripts/release.py printed no plan'
  const first = stderr.trim().split('\n')[0] ?? ''
  const refresh = exitCode === 2 ? ' — re-run devsecops:pipeline to refresh scripts/release.py' : ''
  return `release preview failed (exit ${exitCode})${first === '' ? '' : `: ${first}`}${refresh}`
}

// The dry run reads HEAD's history, so only a chain branch's plan is the next release.
export function isReleaseBranch(branch: string): boolean {
  return (CHAIN as readonly string[]).includes(branch)
}

// What a plan was computed for: HEAD, a newline, the tags merged into it.
export function keyOf(head: string, tags: string): string {
  return `${head}\n${tags}`
}
