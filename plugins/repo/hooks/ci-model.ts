// The CI watch band's wording and parsing (specs/023-ci-watch-band). Pure: no
// `$`, so the unit tests need no engine. ci.ts feeds it.

import type { CiDetect, CiRun, CiRuns } from '../types'
import type { Priority } from './band-priority'

// A `git push` in a command line (not an echo of one), and the ones that change nothing.
const PUSH = /(^|[;&|(])\s*(?:\w+=\S*\s+)*git\s+(?:-C\s+\S+\s+)?push\b/
const DRY = /\s(--dry-run|-n)(\s|$)/

export const isPush = (command: string): boolean => PUSH.test(command) && !DRY.test(command)

// The fields the band reads, all offered by `gh run list --json` (specs/023 FR-008).
export const RUN_FIELDS = 'databaseId,name,status,conclusion,workflowName,headSha'

// The read: runs of this commit, by sha (never "the latest"), through reporun without --write.
export const runListArgv = (pluginRoot: string, sha: string): string[] => [
  'python3', `${pluginRoot}/scripts/reporun.py`, 'cli', 'github', '--', 'run', 'list', '--commit', sha, '--json', RUN_FIELDS,
]

export const detectArgv = (pluginRoot: string): string[] => ['python3', `${pluginRoot}/scripts/reporun.py`, 'detect', '--json']

const text = (value: unknown): string => (typeof value === 'string' ? value : '')

function asRuns(value: unknown): CiRun[] | null {
  if (!Array.isArray(value)) return null
  const runs: CiRun[] = []
  for (const item of value) {
    if (typeof item !== 'object' || item === null) return null
    const raw = item as Record<string, unknown>
    if (typeof raw.databaseId !== 'number' || typeof raw.status !== 'string') return null
    runs.push({ id: raw.databaseId, workflow: text(raw.workflowName), name: text(raw.name), status: raw.status, conclusion: text(raw.conclusion) })
  }
  return runs
}

// `reporun cli` merges stderr into stdout, so a warning or reporun's own line may
// come first: the array starts at the first line that starts with `[` and parses.
// Anything else is a failed read (null), never "no runs" (that is `[]`).
export function parseRuns(output: string): CiRun[] | null {
  const lines = output.split('\n')
  for (let i = 0; i < lines.length; i++) {
    if (!lines[i].startsWith('[')) continue
    let parsed: unknown
    try {
      parsed = JSON.parse(lines.slice(i).join('\n'))
    } catch {
      continue
    }
    const runs = asRuns(parsed)
    if (runs !== null) return runs
  }
  return null
}

// ------------------------------------------------------------------ states

const FAILING = new Set(['failure', 'timed_out', 'startup_failure'])

export const isFailed = (run: CiRun): boolean => run.status === 'completed' && FAILING.has(run.conclusion)

export function markOf(run: CiRun): string {
  if (run.status !== 'completed') return '⏳'
  if (run.conclusion === 'success') return '✓'
  return FAILING.has(run.conclusion) ? '✗' : '–'
}

// Every run finished (and an empty list has nothing left to wait for).
export const isDone = (runs: readonly CiRun[]): boolean => runs.every(run => run.status === 'completed')

// For the band-priority rule: failure while a run failed, normal otherwise.
export const priorityOf = (runs: readonly CiRun[]): Priority => (runs.some(isFailed) ? 'failure' : 'normal')

// Failing runs not toasted yet.
export const newFailures = (runs: readonly CiRun[], toasted: readonly number[]): CiRun[] => runs.filter(run => isFailed(run) && !toasted.includes(run.id))

// ------------------------------------------------------------------ wording

const short = (sha: string): string => sha.slice(0, 7)
const label = (run: CiRun): string => run.workflow || run.name || `run ${run.id}`

export function bandText(ci: CiRuns, columns: number): string {
  const sha = short(ci.sha)
  if (ci.runs.length === 0) return `no runs started for ${sha} (paths, branches or a bot commit?)`
  const list = ci.runs.map(run => `${label(run)} ${markOf(run)}`).join(' · ')
  if (columns < 60) return `CI ${sha}: ${list}`
  return `CI for ${sha} on ${ci.branch}: ${list}${isDone(ci.runs) ? ' — more runs may follow' : ''}`
}

// Never log text, step names, secrets or variables: the run's own workflow name and the skill.
export const toastText = (run: CiRun, sha: string): string => `${label(run)} failed on ${short(sha)} — ask /devsecops:watch why`

// ------------------------------------------------------------------ back-off

// The poll interval, doubled twice (30 s, 60 s, 120 s), then held.
export function delayMs(phase: number, pollSeconds: number): number {
  const base = Number.isFinite(pollSeconds) && pollSeconds > 0 ? pollSeconds : 30
  return base * 1000 * 2 ** Math.min(Math.max(phase, 0), 2)
}

// ------------------------------------------------------------------ detect

// `reporun detect --json`, reduced. Null when it is not the report's shape.
export function parseDetect(output: string): CiDetect | null {
  let report: unknown
  try {
    report = JSON.parse(output)
  } catch {
    return null
  }
  if (typeof report !== 'object' || report === null) return null
  const { repository, platforms } = report as { repository?: { platform?: unknown }; platforms?: unknown }
  if (typeof platforms !== 'object' || platforms === null || Array.isArray(platforms)) return null
  const platform = typeof repository?.platform === 'string' ? repository.platform : ''
  const entry = (platforms as Record<string, { token_in_pass?: unknown; cli?: { installed?: unknown; signed_in?: unknown } }>)[platform]
  return { platform, tokenInPass: entry?.token_in_pass === true, ghSignedIn: entry?.cli?.installed === true && entry?.cli?.signed_in === true }
}

export type Gate = 'read' | 'unsupported' | 'signin' | 'pass'

// Whether the platform may be read: GitHub only, `gh` signed in, and not while the
// token is in pass (reporun cli would decrypt it) unless `ciBandUsePass` is on.
export function gate(detect: CiDetect | null, usePass: boolean): Gate {
  if (detect === null || detect.platform !== 'github') return 'unsupported'
  if (!detect.ghSignedIn) return 'signin'
  if (detect.tokenInPass && !usePass) return 'pass'
  return 'read'
}
