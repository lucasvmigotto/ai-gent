// The hand-off nudge's wording and thresholds, pure: what counts as a full
// context or a low cap, and what the toast says about the checkpoint. The
// calls that feed it are in handoff.ts.

export type Cap = { name: string; cap: number; used: number; left: number }

// What is known about the hand-off file when a nudge fires.
export type Standing = {
  // 012's checkpoint atom (isStale: a commit is newer than the file); null: no file.
  checkpoint: { isStale: boolean; mtimeMs: number } | null
  // `git rev-list --count --after=<mtime> HEAD`; null without git.
  commits: number | null
  // `git status --porcelain` is not empty; null without git.
  isDirty: boolean | null
  now: number
}

const RUN = 'run /project:checkpoint'

// At or above the threshold; a session with no figure never fires.
export function isContextHigh(percent: number | undefined, threshold: number): boolean {
  return typeof percent === 'number' && percent >= threshold
}

// The caps out of `token_report.py budget --json`; null when it is not that output.
export function parseCaps(stdout: string): Cap[] | null {
  try {
    const budget = JSON.parse(stdout) as { caps?: unknown }
    if (!Array.isArray(budget?.caps)) return null
    return budget.caps.filter(
      (cap): cap is Cap =>
        typeof cap?.name === 'string' && typeof cap.cap === 'number' && typeof cap.used === 'number' && typeof cap.left === 'number',
    )
  } catch {
    return null
  }
}

// The caps with less than `percent` of their size left, the tightest first.
export function lowCaps(caps: readonly Cap[], percent: number): Cap[] {
  const share = (cap: Cap): number => (cap.left / cap.cap) * 100
  return caps
    .filter(cap => cap.cap > 0 && share(cap) < percent)
    .sort((a, b) => share(a) - share(b))
}

export function fmtTokens(n: number): string {
  if (n >= 1_000_000) return `${Number((n / 1_000_000).toFixed(1))}M`
  if (n >= 1000) return `${Math.round(n / 1000)}k`
  return String(n)
}

function age(ms: number): string {
  const minutes = Math.max(0, Math.round(ms / 60_000))
  if (minutes < 60) return `${minutes} min`
  if (minutes < 60 * 48) return `${Math.round(minutes / 60)} h`
  return `${Math.round(minutes / 1440)} d`
}

// What to say about the checkpoint, and whether it needs nothing more. It says
// that commits came after it, never that it is wrong.
export function standing(s: Standing): { isCurrent: boolean; text: string } {
  if (s.checkpoint === null) return { isCurrent: false, text: 'There is no checkpoint' }
  const { isStale, mtimeMs } = s.checkpoint
  const tree = s.isDirty ? ', and the working tree has uncommitted changes' : ''
  if (s.commits !== null && s.commits > 0) {
    return { isCurrent: false, text: `The checkpoint is ${s.commits} ${s.commits === 1 ? 'commit' : 'commits'} older than HEAD${tree}` }
  }
  if (isStale) return { isCurrent: false, text: `The checkpoint is older than the last commit${tree}` }
  if (s.isDirty) return { isCurrent: false, text: 'The working tree has uncommitted changes since the checkpoint' }
  if (s.commits === null && s.isDirty === null) return { isCurrent: false, text: `The checkpoint was written ${age(s.now - mtimeMs)} ago` }
  return { isCurrent: true, text: 'The checkpoint is current' }
}

function toast(lead: string, s: Standing, tail: string): string {
  const said = standing(s)
  return said.isCurrent ? `${lead}. ${said.text}; nothing more is needed.` : `${lead}. ${said.text} — ${RUN}${tail}`
}

export function contextToast(percent: number, s: Standing): string {
  return toast(`Context ${percent}% full`, s, ' before the conversation is compacted.')
}

export function compactToast(s: Standing): string {
  return toast('The conversation is about to be compacted', s, ' first: a summary loses what it keeps.')
}

export function capToast(cap: Cap, s: Standing): string {
  return toast(`${cap.name} cap nearly spent (${fmtTokens(cap.left)} of ${fmtTokens(cap.cap)} left)`, s, '.')
}
