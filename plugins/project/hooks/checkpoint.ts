// Reads .ai-gent/checkpoint.md the way scripts/checkpoint.py writes it: the
// "## Holds" and "## Rules in force" sections, one entry per top-level bullet.

import type { Checkpoint } from '../types'

export const CHECKPOINT = '.ai-gent/checkpoint.md'

// An entry that only says the section is empty.
const EMPTY = /^(none|nothing|n\/a|-|—|\(none\))\.?$/i

export function section(text: string, name: string): string[] {
  const lines = text.replace(/<!--[\s\S]*?-->/g, '').split(/\r?\n/)
  const start = lines.findIndex(line => line.trim() === `## ${name}`)
  if (start < 0) return []
  const entries: string[] = []
  for (const line of lines.slice(start + 1)) {
    if (/^#{1,2} /.test(line)) break
    const bullet = /^[-*] +(.*)$/.exec(line)
    if (bullet) entries.push((bullet[1] ?? '').trim())
    else if (/^\s+\S/.test(line) && entries.length > 0) {
      entries[entries.length - 1] += ` ${line.trim()}`
    }
  }
  return entries.filter(entry => entry !== '' && !EMPTY.test(entry))
}

export function parse(text: string): Pick<Checkpoint, 'holds' | 'rules' | 'updated'> {
  return {
    holds: section(text, 'Holds'),
    rules: section(text, 'Rules in force'),
    updated: /^Updated (\S+)/m.exec(text)?.[1] ?? null,
  }
}

// Commands whose go-ahead a checkpoint never carries over (project:recap):
// a push, a merge, a tag or a release. Listing tags is not creating one.
const GATED: readonly [RegExp, string][] = [
  [/^git\s+(?:-C\s+\S+\s+)?push\b/, 'push'],
  [/^git\s+(?:-C\s+\S+\s+)?merge\b(?!.*\s--abort\b)/, 'merge'],
  [/^git\s+(?:-C\s+\S+\s+)?tag\s+[^-\s]/, 'tag'],
  [/^git\s+(?:-C\s+\S+\s+)?tag\s+(?:-[asf]\b|--annotate\b|--sign\b|--force\b|-d\b|--delete\b)/, 'tag'],
  [/^gh\s+pr\s+merge\b/, 'merge'],
  [/^gh\s+release\s+create\b/, 'release'],
]

// What kind of gated step a shell command runs, if any: each segment of a
// compound command is checked, after any leading VAR=value assignments.
export function gated(command: string): string | null {
  for (const segment of command.split(/&&|\|\||[;|\n]/)) {
    const words = segment.trim().replace(/^(?:\w+=\S*\s+)+/, '')
    for (const [pattern, kind] of GATED) {
      if (pattern.test(words)) return kind
    }
  }
  return null
}
