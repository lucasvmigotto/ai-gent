// The pipeline pane as lines: what to say, in what tone, with which button.
// Pure, so the layout (wide, narrow, headless text) is tested without a surface.

import type { Snapshot } from '../types'
import { letter } from './pipeline-model'

export type Tone = 'plain' | 'dim' | 'bold' | 'good' | 'warn' | 'bad'

export type Line = {
  key: string
  text: string
  tone: Tone
  // A button beside the text; `id` says what it does (hooks/register.tsx).
  button?: { id: 'markers' | 'status'; label: string }
}

// Under this many columns the features table is `NNN name P/W/I`.
export const NARROW = 40
const SHOWN_MARKERS = 20

const SIDE_LABEL: Record<string, string> = { frontend: 'F', backend: 'B', status: 'S', qa: 'Q' }

function fit(text: string, columns: number): string {
  return text.length > columns ? `${text.slice(0, Math.max(1, columns - 1))}…` : text
}

export function view(snapshot: Snapshot, columns: number, expanded: Readonly<Record<string, boolean>>): Line[] {
  const lines: Line[] = []
  const add = (text: string, tone: Tone = 'plain', button?: Line['button']): void => {
    lines.push({ key: `l${lines.length}`, text: fit(text, columns - (button ? button.label.length + 3 : 0)), tone, button })
  }
  const narrow = columns < NARROW

  add('Pipeline', 'bold')
  if (snapshot.artifacts.every(a => !a.isPresent)) {
    add('No pipeline artifacts yet. Start with /project:init.', 'warn')
  } else {
    add('Artifacts', 'bold')
    for (const a of snapshot.artifacts) {
      if (a.isPresent && a.staleSince) add(`~ ${a.label}: older than ${a.staleSince.path} (${a.staleSince.date})`, 'warn')
      else if (a.isPresent) add(`✓ ${a.label}${narrow ? '' : `  ${a.path}`}`, 'good')
      else if (a.isProblem) add(`✗ ${a.label}: missing, a later stage already ran`, 'bad')
      else add(`· ${a.label}`, 'dim')
    }
    if (!snapshot.hasGit) add('Staleness needs git.', 'dim')
  }

  add('Features', 'bold')
  if (!snapshot.hasReadme) {
    add('No specs/README.md yet.', 'dim')
  } else if (snapshot.features.length === 0) {
    add('specs/README.md lists no feature.', 'dim')
  }
  for (const f of snapshot.features) {
    const mark = f.bucket === 'done' ? '✓' : f.bucket === 'ready' ? '▶' : '⏸'
    const tone: Tone = f.bucket === 'done' ? 'dim' : f.bucket === 'ready' ? 'bold' : 'plain'
    const sides = Object.entries(f.sides)
    if (narrow) {
      add(`${mark} ${f.number} ${f.name} ${sides.map(([, v]) => letter(v)).join('/')}`, tone)
    } else {
      const cells = sides.map(([k, v]) => `${SIDE_LABEL[k] ?? k}:${v || '—'}`).join(' ')
      const waits = f.waiting.length > 0 ? ` · waits for ${f.waiting.join(', ')}` : ''
      add(`${mark} ${f.number} ${f.name}  ${f.priority}  ${cells}${waits}`, tone)
    }
  }

  const { counts, items, isTruncated } = snapshot.markers
  const isOpen = expanded.markers === true
  const many = (n: number, one: string): string => `${n} ${one}${n === 1 ? '' : 's'}`
  add(
    `${many(counts.clarification, 'clarification')} · ${many(counts.assumption, 'assumption')} · ${many(counts.gap, 'gap')}${isTruncated ? ' (more not counted)' : ''}`,
    counts.clarification > 0 ? 'warn' : 'plain',
    items.length > 0 ? { id: 'markers', label: isOpen ? 'Hide' : 'Show' } : undefined,
  )
  if (isOpen) {
    for (const m of items.slice(0, SHOWN_MARKERS)) add(`  ${m.file}:${m.line} ${m.text}`, 'dim')
    if (items.length > SHOWN_MARKERS) add(`  +${items.length - SHOWN_MARKERS} more`, 'dim')
  }

  const { next } = snapshot
  add(`Next (a hint): ${next.stage}, ${next.reason}${next.ref ? ` [${next.ref}]` : ''}`, 'bold', { id: 'status', label: '/project:status' })
  return lines
}

// What `/pipeline` prints where no surface can draw the pane.
export function text(snapshot: Snapshot): string {
  return view(snapshot, 200, { markers: true })
    .map(line => line.text)
    .join('\n')
}
