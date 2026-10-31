// The card timeline pane as lines, and as text where nothing can draw it
// (specs/020-card-timeline-pane). Pure, so the layouts (wide, narrow, text) are
// tested without a surface.

import type { LogEntry, Span } from './cards-model'
import { axis, headerText, hhmm, overlapsOf, peak } from './cards-model'
import { fmtTok } from './token-model'
import type { LedgerCard } from './token-model'

export type Measure = { status: 'running' | 'failed' | 'done'; error: string | null; spans: Span[]; at: number }

export type Snapshot = {
  cards: LedgerCard[]
  log: LogEntry[]
  // When the mod first looked (local HH:MM), or null before it did.
  since: string | null
  measure: Measure | null
}

export type Tone = 'plain' | 'dim' | 'bold' | 'good' | 'warn' | 'bad'
export type Line = { key: string; text: string; tone: Tone; button?: { id: 'measure'; label: string } }

// Under this many columns the bars and the branch are dropped.
export const NARROW = 60
const SHOWN_LOG = 20
const LABEL = 14 // room beside a bar for `14:00–14:45`
const KNOWN = new Set(['queued', 'running', 'review', 'landed'])

const fit = (text: string, columns: number): string => (text.length > columns ? `${text.slice(0, Math.max(1, columns - 1))}…` : text)

// The spans of the cards the ledger lists: the ledger decides which cards exist.
const spansOf = (s: Snapshot): Span[] => {
  const ids = new Set(s.cards.map(c => c.id))
  return (s.measure?.status === 'done' ? s.measure.spans : []).filter(span => ids.has(span.card))
}

const range = (span: Span): string => `${hhmm(span.startMs)}–${hhmm(span.endMs)}`

function summary(s: Snapshot): string {
  const spans = spansOf(s)
  return s.measure?.status === 'done' && spans.length > 0 ? `${headerText(s.cards)} · peak ${peak(spans)} in parallel` : headerText(s.cards)
}

function measureNote(s: Snapshot): { text: string; tone: Tone } {
  if (s.measure === null) return { text: 'Spans: press Measure (reads the transcripts, can take a while)', tone: 'dim' }
  if (s.measure.status === 'running') return { text: 'Spans: measuring…', tone: 'dim' }
  if (s.measure.status === 'failed') return { text: `Spans: measure failed: ${s.measure.error ?? 'unknown error'}`, tone: 'warn' }
  return { text: 'Spans: measured', tone: 'dim' }
}

// The row's own cells: id, state, branch and worktree when the ledger has them, the Tokens cell as written.
function row(c: LedgerCard, idWidth: number, isNarrow: boolean, span: Span | undefined): string {
  const tokens = `${c.projectedText} / ${c.usedText}`
  const parts = [c.id.padEnd(idWidth), c.state.padEnd(8)]
  if (!isNarrow) parts.push(...[c.branch, c.worktree].filter((x): x is string => x !== undefined))
  parts.push(tokens)
  if (span && !isNarrow) parts.push(`script ${fmtTok(span.used)}`)
  return parts.join('  ').trimEnd()
}

function logLines(s: Snapshot): string[] {
  const shown = s.log.slice(-SHOWN_LOG).map(e => `${e.at} ${e.id} ${e.from} → ${e.to}`)
  return [s.since !== null ? `since ${s.since} (changes seen in this session)` : 'changes seen in this session', ...(shown.length > 0 ? shown : ['none yet'])]
}

export function view(s: Snapshot, columns: number): Line[] {
  const lines: Line[] = []
  const isNarrow = columns < NARROW
  const add = (text: string, tone: Tone = 'plain', button?: Line['button']): void => {
    lines.push({ key: `l${lines.length}`, text: fit(text, columns - (button ? button.label.length + 3 : 0)), tone, button })
  }

  add(summary(s), 'bold', { id: 'measure', label: 'Measure' })
  if (s.cards.length === 0) {
    add('No cards in .ai-gent/orchestrate-status.md.', 'dim')
  } else {
    const note = measureNote(s)
    add(note.text, note.tone)
    const spans = spansOf(s)
    const idWidth = Math.max(...s.cards.map(c => c.id.length))
    const bars = !isNarrow && spans.length > 0 ? axis(spans, Math.max(10, columns - 2 - LABEL)) : null
    const overlaps = overlapsOf(spans)
    for (const c of s.cards) {
      const span = spans.find(x => x.card === c.id)
      add(row(c, idWidth, isNarrow, span), KNOWN.has(c.state) ? (c.state === 'landed' ? 'dim' : 'plain') : 'dim')
      if (!isNarrow && s.measure?.status === 'done') {
        const bar = bars?.bars.find(b => b.card === c.id)
        if (span && bar) {
          const near = overlaps[c.id] ?? []
          add(`  ${' '.repeat(bar.col)}${'█'.repeat(bar.width)} ${range(span)}${near.length > 0 ? ` ∥ ${near.join(', ')}` : ''}`, 'good')
        } else {
          add('  no run yet', 'dim')
        }
      }
    }
    if (bars) add(`  ${hhmm(bars.startMs)} → ${hhmm(bars.endMs)} (local time)`, 'dim')
  }
  add('Changes', 'bold')
  for (const text of logLines(s)) add(text, text.startsWith('since') || text === 'none yet' ? 'dim' : 'plain')
  return lines
}

// The same content as plain text, for a surface that cannot dock the pane.
export function text(s: Snapshot): string {
  const out = [summary(s)]
  if (s.cards.length === 0) out.push('no cards in .ai-gent/orchestrate-status.md')
  const idWidth = Math.max(0, ...s.cards.map(c => c.id.length))
  for (const c of s.cards) out.push(row(c, idWidth, false, undefined))
  out.push('', 'Changes', ...logLines(s).map(l => `  ${l}`), '')
  const note = measureNote(s)
  const spans = spansOf(s)
  if (s.measure?.status === 'done') {
    out.push('Spans:')
    const overlaps = overlapsOf(spans)
    for (const c of s.cards) {
      const span = spans.find(x => x.card === c.id)
      if (!span) {
        out.push(`  ${c.id} no run yet`)
        continue
      }
      const near = overlaps[c.id] ?? []
      out.push(`  ${c.id} ${range(span)} (local), script used ${fmtTok(span.used)}${near.length > 0 ? `, overlaps ${near.join(', ')}` : ''}`)
    }
  } else {
    out.push(note.text.replace('press Measure (reads the transcripts, can take a while)', 'not measured (/cards-pane --measure)'))
  }
  return out.join('\n')
}
