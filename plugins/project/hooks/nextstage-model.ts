// The next-stage toast's pure parts (spec 027): whether a recommendation is new
// and what the toast says. The recommendation itself is the pane's
// `compute($).next`; nothing here decides which stage is next.

import type { Next } from '../types'
import { ARTIFACTS } from './pipeline-model'

export const HINT = 'hint only — /project:status for the report'

// What is remembered and compared: the stage and the reason.
export const keyOf = (next: Next): string => `${next.stage} ${next.reason}`

// A recommendation is new when nothing was seen before or its stage or reason changed.
export const differs = (last: Next | null, next: Next): boolean => last === null || keyOf(last) !== keyOf(next)

// What a written path is called in the toast: a pipeline artifact, or a Spec Kit
// file directly under specs/NNN-*/. Anything else has no label.
export function labelOf(path: string): string | null {
  const artifact = ARTIFACTS.find(spec => path.endsWith(`/${spec.path}`))
  if (artifact) return artifact.label
  const feature = /\/specs\/(\d{3}-[^/]+)\/([^/]+)\.md$/.exec(path)
  return feature ? `${feature[1]} ${feature[2]}` : null
}

// `<label> written — next: <stage> (<reason>)`; rule 1 asks the user and points at file:line.
export function toastText(next: Next, written: readonly string[]): string {
  if (next.rule === 1) return `${next.stage} (${next.reason})${next.ref ? ` — ${next.ref}` : ''} · ${HINT}`
  const lead = written.length > 0 ? `${written.join(', ')} written — ` : ''
  return `${lead}next: ${next.stage} (${next.reason}) · ${HINT}`
}
