// Masks credentials in text the person sends, with the patterns in
// shared/secret-patterns.json (the list project:recap redacts transcripts with).
// Pure: no `$`, so the hooks in register.tsx and the tests share it as it is.

export type Pattern = {
  kind: string
  source: string
  flags: string
  replacement: string
  entry?: string
  toggle?: string
  skipIn?: string[]
}

export type Rule = Pattern & { re: RegExp }

// Where text is masked: what the person types, or a skill's written body,
// where only the high-confidence kinds apply (prose and examples stay whole).
export type Scope = 'prompt' | 'skill'

export type Masked = {
  text: string
  // How many of each kind, never the values.
  counts: Record<string, number>
  total: number
  // The pass entries for the kinds hit.
  entries: string[]
}

const LABELS: Record<string, string> = {
  'private-key': 'private key',
  assignment: 'password or secret assignment',
  'auth-header': 'auth header',
  'github-token': 'GitHub token',
  'api-key': 'API key',
  'aws-key': 'AWS key',
  'slack-token': 'Slack token',
  'url-credentials': 'URL password',
}

export function compile(json: string): Rule[] {
  const list: unknown = JSON.parse(json)?.patterns
  if (!Array.isArray(list) || list.length === 0) throw new Error('secret-patterns.json has no patterns')
  return list.map((p: Pattern) => ({ ...p, re: new RegExp(p.source, `${p.flags}g`) }))
}

// `\1`-style group references, the dialect both Python and JavaScript read.
function expand(template: string, groups: readonly string[]): string {
  return template.replace(/\\(\d)/g, (_, digit: string) => groups[Number(digit)] ?? '')
}

export function mask(text: string, rules: readonly Rule[], scope: Scope, off: ReadonlySet<string>): Masked {
  const counts: Record<string, number> = {}
  const entries = new Set<string>()
  let out = text
  for (const rule of rules) {
    if ((rule.toggle && off.has(rule.toggle)) || rule.skipIn?.includes(scope)) continue
    out = out.replace(rule.re, (...args: unknown[]) => {
      counts[rule.kind] = (counts[rule.kind] ?? 0) + 1
      if (rule.entry) entries.add(rule.entry)
      return expand(rule.replacement, args.slice(0, -2) as string[])
    })
  }
  const total = Object.values(counts).reduce((sum, n) => sum + n, 0)
  return { text: out, counts, total, entries: [...entries] }
}

// One line for the person: what was masked and where the value belongs.
export function summary(masked: Masked): string {
  const parts = Object.entries(masked.counts).map(([kind, n]) => `${LABELS[kind] ?? kind} ×${n}`)
  const where = masked.entries.length > 0 ? `pass insert ${masked.entries.join(' / ')}` : 'secrets:pass'
  return `secret-mask: masked ${parts.join(', ')} — keep secrets in pass (${where}), not in a prompt`
}

// A cheap look for text that may hold a credential, used only when masking
// itself failed: then the text is held back if it looks like one, else sent.
const KEYS = String.raw`BEGIN [A-Z ]*PRIVATE|gh[pousr]_[A-Za-z0-9]{20}|github_pat_|\bsk-[A-Za-z0-9_-]{20}|AKIA[0-9A-Z]{16}|xox[baprs]-|:\/\/[^\s\/@:]+:[^\s@]+@`
const LOOKS: Record<Scope, RegExp> = {
  prompt: new RegExp(
    String.raw`${KEYS}|\b(?:bearer|basic)\s+[A-Za-z0-9._~+\/=-]{12}|(?:password|passwd|pwd|secret|token|api_?key|access_?key|pat)\b\s*[=:]`,
    'i',
  ),
  skill: new RegExp(KEYS, 'i'),
}

export function looksSecret(text: string, scope: Scope): boolean {
  return LOOKS[scope].test(text)
}
