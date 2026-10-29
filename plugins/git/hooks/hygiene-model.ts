import type { BranchState, ChainBranch } from '../types'

// What the repo-hygiene band says, from git's own output. Pure: no `$`, so the
// band and the tests share it as it is. It only says what git:workflow already
// requires after a release ("git pull --ff-only on both"); nothing here runs
// git, fetches, or decides anything.

// The subjects .github/workflows/release.yml writes, and shared/release-platforms.md
// prescribes for other projects. scripts/consistency.py checks both files still say so.
export const RELEASE_ON_MAIN = /^chore\(release\): (\d+\.\d+\.\d+)$/
export const RELEASE_BACK_INTO_DEV = /^chore\(release\): merge (\d+\.\d+\.\d+) back into (?:dev|develop)$/

// The chain's long-lived branches the band compares, with the release subject each receives.
export const CHAIN: readonly ChainBranch[] = ['main', 'master', 'dev', 'develop']

export function isMainLine(name: string): boolean {
  return name === 'main' || name === 'master'
}

// `git rev-list --left-right --count local...origin/local` prints "ahead<TAB>behind".
export function parseCounts(out: string): { ahead: number; behind: number } | null {
  const [ahead, behind] = out.trim().split(/\s+/).map(Number)
  return Number.isInteger(ahead) && Number.isInteger(behind) ? { ahead: ahead as number, behind: behind as number } : null
}

// The release versions in `git log --format=%s local..origin/local`.
export function releasesIn(name: string, subjects: readonly string[]): string[] {
  const pattern = isMainLine(name) ? RELEASE_ON_MAIN : RELEASE_BACK_INTO_DEV
  return subjects.map(subject => pattern.exec(subject.trim())?.[1]).filter((v): v is string => v !== undefined)
}

// The branch names present both locally and as origin/<name>, from `for-each-ref %(refname)`.
export function branchesWithRemote(refs: string): ChainBranch[] {
  const lines = new Set(refs.split(/\r?\n/).map(line => line.trim()))
  return CHAIN.filter(name => lines.has(`refs/heads/${name}`) && lines.has(`refs/remotes/origin/${name}`))
}

export const NARROW = 50

function days(ms: number): number {
  return Math.floor(ms / 86_400_000)
}

// The band's one line, or null when every branch is level with its remote.
export function sentence(states: readonly BranchState[], columns: number, fetchedAgoMs: number | null): string | null {
  const off = states.filter(s => s.behind > 0 || s.ahead > 0)
  if (off.length === 0) return null

  const stale = fetchedAgoMs !== null && days(fetchedAgoMs) >= 1 ? ` (as of the last fetch, ${days(fetchedAgoMs)} day${days(fetchedAgoMs) === 1 ? '' : 's'} ago)` : ''
  const diverged = off.filter(s => s.ahead > 0 && s.behind > 0)
  const behind = off.filter(s => s.behind > 0 && s.ahead === 0)

  if (columns < NARROW) {
    const parts = [...behind.map(s => `${s.name} ↓${s.behind}`), ...diverged.map(s => `${s.name} ⇅`)]
    return parts.length === 0 ? null : `${parts.join(' · ')} — ${diverged.length > 0 ? 'git:workflow' : 'pull --ff-only'}`
  }

  const parts: string[] = []
  for (const s of behind) {
    const release = s.releases.length > 0 ? ` (release ${s.releases[0]})` : ''
    parts.push(`${s.name} is ${s.behind} behind origin/${s.name}${release}`)
  }
  for (const s of diverged) parts.push(`${s.name} has diverged from origin/${s.name} (${s.ahead} ahead, ${s.behind} behind)`)
  if (parts.length === 0) return null

  const action: string[] = []
  if (behind.length > 0) action.push(`git pull --ff-only on ${behind.map(s => s.name).join(' and ')} before the next commit`)
  if (diverged.length > 0) action.push(`${diverged.map(s => s.name).join(' and ')}: see git:workflow, don't pull`)
  return `${parts.join('; ')} — ${action.join('; ')}${stale}`
}

// Commands that move refs: after one, the comparison is read again.
export const MOVES_REFS = /(^|[;&|(])\s*(?:\w+=\S*\s+)*git\s+(?:-C\s+\S+\s+)?(fetch|pull|merge|rebase|commit|checkout|switch|reset)\b/
