// The worktree board's reading and wording (specs/022-worktree-board): pure, so
// the join, the marks, the narrow form and the text are tested without a surface.
// Local columns are exactly what `wt.py list --json` reports (FR-002), the check
// rollup is data-model.md's mapping (FR-007); nothing else is computed.

import type { Line, Tone } from './cards-view'

export type Checks = 'failing' | 'pending' | 'passing' | 'none'
export type Pr = { number: number; state: 'OPEN' | 'MERGED' | 'CLOSED'; isDraft: boolean; checks: Checks }
// `pr` is absent until gh was read (or for a detached HEAD, which is never looked up), null for no pull request.
export type Row = { branch: string; path: string; dirty: boolean; ahead: number | null; behind: number | null; pr?: Pr | null }
export type Mark = 'failing' | 'removable' | 'nothing of its own'
export type Gh = { usable: boolean; platform: string }
export type Board = {
  rows: Row[]
  // The first stderr line of the last failed wt.py run, or null.
  error: string | null
  gh: Gh
}

// Rows drawn; the rest is "+N more" (wt.py list spawns git per worktree).
export const MAX_ROWS = 30
// Under this many columns a row is `branch +a/-b ●`.
export const NARROW = 60
export const NO_ROWS = 'no worktrees under .worktrees/'

const isObject = (x: unknown): x is Record<string, unknown> => typeof x === 'object' && x !== null && !Array.isArray(x)
const count = (x: unknown): number | null | undefined => (x === null ? null : typeof x === 'number' && Number.isInteger(x) ? x : undefined)

// `wt.py list --json`: null when it is not the script's array of rows.
export function parseWt(stdout: string): Row[] | null {
  let data: unknown
  try {
    data = JSON.parse(stdout)
  } catch {
    return null
  }
  if (!Array.isArray(data)) return null
  const out: Row[] = []
  for (const x of data) {
    if (!isObject(x)) return null
    const { branch, path, dirty } = x
    const ahead = count(x.ahead)
    const behind = count(x.behind)
    if (typeof branch !== 'string' || typeof path !== 'string' || typeof dirty !== 'boolean' || ahead === undefined || behind === undefined) return null
    out.push({ branch, path, dirty, ahead, behind })
  }
  return out
}

const FAILING = new Set(['FAILURE', 'TIMED_OUT', 'CANCELLED', 'ACTION_REQUIRED', 'STARTUP_FAILURE'])

// data-model.md's table, first match wins. A check run has `conclusion` and `status`, a commit status `state`.
export function checksOf(rollup: unknown): Checks {
  if (!Array.isArray(rollup) || rollup.length === 0) return 'none'
  const checks = rollup.filter(isObject)
  if (checks.some(c => FAILING.has(String(c.conclusion)) || c.state === 'FAILURE' || c.state === 'ERROR')) return 'failing'
  if (checks.some(c => (typeof c.status === 'string' && c.status !== 'COMPLETED') || c.state === 'PENDING' || c.state === 'EXPECTED')) return 'pending'
  return checks.length > 0 ? 'passing' : 'none'
}

const STATES = new Set(['OPEN', 'MERGED', 'CLOSED'])
const RANK: Record<string, number> = { OPEN: 2, MERGED: 1, CLOSED: 0 }

// `gh pr list --json number,headRefName,state,isDraft,statusCheckRollup` by head branch. A branch with
// several pull requests keeps the open one, else the merged one, else the newest.
export function parseGh(stdout: string): Record<string, Pr> | null {
  let data: unknown
  try {
    data = JSON.parse(stdout)
  } catch {
    return null
  }
  if (!Array.isArray(data)) return null
  const out: Record<string, Pr> = {}
  for (const x of data) {
    if (!isObject(x)) return null
    const { headRefName, number, state, isDraft } = x
    if (typeof headRefName !== 'string' || typeof number !== 'number' || typeof state !== 'string' || !STATES.has(state) || typeof isDraft !== 'boolean') return null
    const pr: Pr = { number, state: state as Pr['state'], isDraft, checks: checksOf(x.statusCheckRollup) }
    const kept = out[headRefName]
    if (!kept || (RANK[pr.state] ?? 0) > (RANK[kept.state] ?? 0) || (pr.state === kept.state && pr.number > kept.number)) out[headRefName] = pr
  }
  return out
}

const DETACHED = '(detached)'

// Sets each row's pull request from the lookup; a detached HEAD is skipped.
export function joinPrs(rows: readonly Row[], prs: Record<string, Pr>): Row[] {
  return rows.map(r => (r.branch === DETACHED ? { ...r, pr: undefined } : { ...r, pr: prs[r.branch] ?? null }))
}

// Fresh local rows keep the pull request last read for the same branch.
export function carryPrs(fresh: readonly Row[], old: readonly Row[]): Row[] {
  const known = new Map(old.map(r => [r.branch, r.pr]))
  return fresh.map(r => (r.branch !== DETACHED && known.get(r.branch) !== undefined ? { ...r, pr: known.get(r.branch) } : r))
}

// Failing, else removable (a merged pull request and a clean tree: never `ahead`, which a rebase merge leaves above 0),
// else nothing of its own (ahead 0 and clean).
export function markOf(r: Row): Mark | null {
  if (r.pr?.checks === 'failing') return 'failing'
  if (r.pr?.state === 'MERGED' && !r.dirty) return 'removable'
  if (r.ahead === 0 && !r.dirty) return 'nothing of its own'
  return null
}

const counts = (r: Row): string => (r.ahead === null || r.behind === null ? '?' : `+${r.ahead}/-${r.behind}`)
const PLATFORMS: Record<string, string> = { github: 'GitHub', gitlab: 'GitLab', azure: 'Azure DevOps', bitbucket: 'Bitbucket' }

// Whether the pull request column is drawn: gh usable on GitHub.
const hasPrColumn = (b: Board): boolean => b.gh.usable && b.gh.platform === 'github'

function summary(b: Board): string {
  const marks = b.rows.map(markOf)
  const parts = [`${b.rows.length} worktree${b.rows.length === 1 ? '' : 's'}`]
  const dirty = b.rows.filter(r => r.dirty).length
  const failing = marks.filter(m => m === 'failing').length
  const removable = marks.filter(m => m === 'removable').length
  if (dirty > 0) parts.push(`${dirty} uncommitted`)
  if (failing > 0) parts.push(`${failing} failing`)
  if (removable > 0) parts.push(`${removable} removable`)
  return parts.join(' · ')
}

function prCell(r: Row, b: Board): string[] {
  if (!hasPrColumn(b)) return []
  if (r.pr === undefined) return [r.branch === DETACHED ? '—' : '…']
  if (r.pr === null) return ['no PR']
  const state = r.pr.state === 'OPEN' ? (r.pr.isDraft ? 'draft' : 'open') : r.pr.state.toLowerCase()
  return [`#${r.pr.number} ${state}`, ...(r.pr.checks === 'none' ? [] : [`checks ${r.pr.checks}`])]
}

function row(r: Row, b: Board, width: number, isNarrow: boolean): { text: string; tone: Tone } {
  const mark = markOf(r)
  const tone: Tone = mark === 'failing' ? 'bad' : mark === 'removable' || mark === 'nothing of its own' ? 'dim' : 'plain'
  if (isNarrow) return { text: [r.branch, counts(r), r.dirty ? '●' : ''].filter(Boolean).join(' '), tone }
  const cells = [r.branch.padEnd(width), counts(r).padEnd(6), r.dirty ? '●' : ' ', ...prCell(r, b)]
  if (mark === 'removable') cells.push(`removable: wt.py remove ${r.branch}`)
  else if (mark !== null) cells.push(mark)
  return { text: cells.join('  ').trimEnd(), tone }
}

function notes(b: Board): { text: string; tone: Tone }[] {
  const out: { text: string; tone: Tone }[] = []
  if (b.error !== null) out.push({ text: `wt.py failed: ${b.error}`, tone: 'warn' })
  if (b.rows.some(r => r.ahead === null || r.behind === null)) out.push({ text: 'no dev branch found: ahead and behind are unknown (?)', tone: 'dim' })
  if (b.gh.platform === 'github' && !b.gh.usable) out.push({ text: 'gh is not signed in: gh auth login would add the pull request and checks', tone: 'dim' })
  else if (b.gh.platform !== 'github' && b.gh.platform !== '') {
    const name = PLATFORMS[b.gh.platform]
    out.push({ text: name ? `origin is on ${name}: its pull requests are not read yet (GitHub only)` : 'no GitHub origin: pull requests and checks are not read yet', tone: 'dim' })
  }
  return out
}

const fit = (text: string, columns: number): string => (text.length > columns ? `${text.slice(0, Math.max(1, columns - 1))}…` : text)

export function view(b: Board, columns: number): Line[] {
  const lines: Line[] = []
  const add = (text: string, tone: Tone = 'plain'): void => {
    lines.push({ key: `l${lines.length}`, text: fit(text, columns), tone })
  }
  if (b.rows.length === 0) {
    add(NO_ROWS, 'dim')
  } else {
    add(summary(b), 'bold')
    const shown = b.rows.slice(0, MAX_ROWS)
    const width = Math.max(...shown.map(r => r.branch.length))
    for (const r of shown) {
      const cell = row(r, b, width, columns < NARROW)
      add(cell.text, cell.tone)
    }
    if (b.rows.length > MAX_ROWS) add(`+${b.rows.length - MAX_ROWS} more`, 'dim')
  }
  for (const n of notes(b)) add(n.text, n.tone)
  return lines
}

// The same content as plain text, for a surface that cannot dock the pane.
export function text(b: Board): string {
  if (b.rows.length === 0 && b.error === null) return NO_ROWS
  return view(b, Number.MAX_SAFE_INTEGER).map(l => l.text).join('\n')
}
