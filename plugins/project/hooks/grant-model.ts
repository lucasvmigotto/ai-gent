// The autonomy grant as one line of the guards view, pure: the file's text and the clock in, a line out.
// Read-only (the user alone writes the grant file). The parse follows the same rules as
// `opencode/guard/rules.mjs` (parseGrant) and `rust/crates/ai-gent-core/src/guard/grant.rs`:
// `expires` in exactly `YYYY-MM-DDTHH:MM:SSZ`, `branches` (default `develop dev`), `actions`.
// Only three fields are ever shown: the known actions, the branches and the expiry.

const KNOWN = ['push', 'pr', 'merge', 'issues'] as const
const EXPIRES = /^([0-9]{4})-([0-9]{2})-([0-9]{2})T([0-9]{2}):([0-9]{2}):([0-9]{2})Z$/

function value(text: string, key: string): string {
  const line = text.split('\n').find(l => l.startsWith(`${key}:`))
  return line === undefined ? '' : line.slice(key.length + 1).replace(/^[ \t]+/, '').replace(/\r$/, '')
}

const words = (v: string): string[] => v.split(/[ \t]+/).filter(Boolean)

// Time left, rounded down: under a minute, minutes, hours, days.
export function timeLeft(ms: number): string {
  const min = Math.floor(ms / 60000)
  if (min < 1) return 'in under 1m'
  if (min < 60) return `in ${min}m`
  if (min < 60 * 24) return `in ${Math.floor(min / 60)}h`
  return `in ${Math.floor(min / (60 * 24))}d`
}

// `text` is null when the file is missing or unreadable. Anything malformed means no grant.
export function grantLine(text: string | null, now: Date): string {
  if (text === null) return 'autonomy: none'
  const m = EXPIRES.exec(value(text, 'expires'))
  if (m === null) return 'autonomy: none'
  const [y, mo, d, h, mi, s] = m.slice(1).map(Number) as [number, number, number, number, number, number]
  const at = Date.UTC(y, mo - 1, d, h, mi, s)
  const back = new Date(at)
  if (Number.isNaN(at) || back.getUTCMonth() !== mo - 1 || back.getUTCDate() !== d || h > 23 || mi > 59 || s > 59) return 'autonomy: none'
  const left = at - now.getTime()
  if (left <= 0) return 'autonomy: expired'
  const given = new Set(words(value(text, 'actions')))
  const actions = KNOWN.filter(a => given.has(a))
  if (actions.length === 0) return 'autonomy: none lifted'
  const named = words(value(text, 'branches'))
  const branches = (named.length > 0 ? named : ['develop', 'dev']).join(' ')
  const when = `${m[1]}-${m[2]}-${m[3]} ${m[4]}:${m[5]} UTC`
  return `autonomy: ${actions.join(' ')} on ${branches}, until ${when} (${timeLeft(left)})`
}
