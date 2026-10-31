import type { Finding, Priority, ReportProblem } from '../types'

// What the container and disk band says (specs/025-container-disk-band), from
// `leftovers.sh --json`. Pure: no `$`, so the band, /containers and the tests
// share it. It reads the JSON's keys only, never the script's sentences, and
// prints nothing that teaches a removal beyond the §9a pointer.

export const NARROW = 60

// The pointer /containers ends with: removal is by ID or label, never a sweep.
export const POINTER =
  "`wt.py resources` lists them with sizes (agents:parallel); removal is by ID or by this project's label (containers.md §9a) and asks for anything that isn't this project's"

export type Report = { checked: boolean; reason: string | null; findings: Finding[] }

const isObject = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null && !Array.isArray(value)
const number = (value: unknown): number | undefined => (typeof value === 'number' && Number.isFinite(value) ? value : undefined)

function readFinding(value: unknown): Finding | null {
  if (!isObject(value) || typeof value.kind !== 'string' || value.kind === '') return null
  const finding: Finding = { kind: value.kind }
  const count = number(value.count)
  const free = number(value.free_gb)
  const floor = number(value.floor_gb)
  if (count !== undefined) finding.count = count
  if (free !== undefined) finding.free_gb = free
  if (floor !== undefined) finding.floor_gb = floor
  return finding
}

// The one JSON object leftovers.sh --json prints; null when the output is anything else.
export function readReport(stdout: string): Report | null {
  let parsed: unknown
  try {
    parsed = JSON.parse(stdout)
  } catch {
    return null
  }
  if (!isObject(parsed) || typeof parsed.checked !== 'boolean') return null
  if (!parsed.checked) return { checked: false, reason: typeof parsed.reason === 'string' ? parsed.reason : null, findings: [] }
  if (!Array.isArray(parsed.findings)) return null
  const findings = parsed.findings.map(readFinding)
  if (findings.some(finding => finding === null)) return null
  return { checked: true, reason: null, findings: findings as Finding[] }
}

const plural = (n: number, one: string, many: string): string => `${n} ${n === 1 ? one : many}`

// One finding in the band's words (the data model's table).
export function findingText(finding: Finding): string {
  const n = finding.count ?? 0
  switch (finding.kind) {
    case 'disk':
      return `disk ${finding.free_gb ?? '?'} GB free, floor ${finding.floor_gb ?? '?'}`
    case 'dangling':
      return plural(n, 'dangling image', 'dangling images')
    case 'volumes':
      return `${plural(n, 'volume', 'volumes')} of a gone worktree`
    case 'buildah':
      return `${n} buildah (any project)`
    default:
      return finding.count === undefined ? finding.kind : `${finding.kind} ${finding.count}`
  }
}

// The disk line carries a mark as well as a colour.
export const findingLine = (finding: Finding): string => (finding.kind === 'disk' ? `⚠ ${findingText(finding)}` : findingText(finding))

// The band's line, in the script's order; null when there is nothing to say.
export function bandText(findings: readonly Finding[], columns: number): string | null {
  if (findings.length === 0) return null
  if (columns < NARROW) return `containers ⚠ ${findings.length} — /containers`
  const hasDisk = findings.some(finding => finding.kind === 'disk')
  return `containers: ${hasDisk ? '⚠ ' : ''}${findings.map(findingText).join(' · ')} — /containers`
}

// A disk under the floor is worth the room of a normal band; leftovers give way first.
export const priorityOf = (findings: readonly Finding[]): Priority => (findings.some(finding => finding.kind === 'disk') ? 'normal' : 'info')

const REASONS: Record<string, string> = {
  'no-engine': 'no container engine found',
  'not-a-checkout': 'not a git checkout',
  disabled: 'AI_GENT_NO_LEFTOVERS_CHECK is set',
}

// What /containers prints. `report` is null when the script gave none (`problem` says why);
// `script` is the script's own text form, shown under the findings.
export function commandText(report: Report | null, script: string, problem: ReportProblem = 'unreadable'): string {
  if (report === null) {
    if (problem === 'timeout') return 'The report timed out: leftovers.sh did not answer in 10 seconds.'
    if (problem === 'missing') return 'The report is not available: hooks/leftovers.sh is missing from the plugin.'
    return 'The report could not be read: leftovers.sh --json did not print JSON.'
  }
  if (!report.checked) {
    const reason = report.reason ?? 'unknown'
    return `Nothing was checked (${reason}): ${REASONS[reason] ?? 'the script gave no further reason'}.`
  }
  if (report.findings.length === 0) return 'No leftovers or disk problem reported for this project.'
  const lines = ['Container leftovers (report only, nothing removed):', ...report.findings.map(finding => `- ${findingLine(finding)}`)]
  if (script.trim() !== '') lines.push('', script.trim())
  lines.push('', POINTER)
  return lines.join('\n')
}

// Bash commands that built, ran or removed container resources, or moved worktrees.
export const TRIGGER = /^\s*(?:\w+=\S*\s+)*(?:sudo\s+)?(?:(?:podman|docker|buildah)\s+(?:build|bud|run|rm|rmi|volume|compose)\b|(?:python3?\s+)?\S*wt\.py\s+(?:add|remove|finish)\b)/
