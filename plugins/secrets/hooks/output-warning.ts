// The pure parts of the output warning (specs/026): which tools are redacted,
// the scan of a tool-result row's text, and the toast rate. No `$`, so the hook
// in register.tsx and the tests share them. Nothing here returns, logs or
// keeps a match: only kinds, counts and the rewritten text.

import { looksSecret, mask } from './secret-mask'
import type { Masked, Rule } from './secret-mask'

// Text past this many characters is passed on unscanned.
export const SCAN_LIMIT = 256 * 1024

export const DEFAULT_TOOLS = 'Bash,WebFetch'

export type Block = { type: string; [field: string]: unknown }

export type Scan = {
  content: Block[]
  masked: Masked
  // True when some text was longer than SCAN_LIMIT and its tail was not scanned.
  truncated: boolean
}

const NONE: ReadonlySet<string> = new Set()

// The tools whose results are redacted, from a comma-separated option.
export function redactTools(list: string | undefined): Set<string> {
  return new Set(
    (list ?? DEFAULT_TOOLS)
      .split(',')
      .map(name => name.trim())
      .filter(name => name !== ''),
  )
}

// Scans the text blocks of a row, and of each tool_result's content, for the
// high-confidence kinds. With `redact` the matches are replaced in the returned
// content; otherwise it is the input, untouched. A row with no match comes back
// as the very same array.
export function scanContent(content: readonly Block[], rules: readonly Rule[], redact: boolean): Scan {
  const total: Masked = { text: '', counts: {}, total: 0, entries: [] }
  let truncated = false
  let changed = false

  const text = (value: string): string => {
    const head = value.length > SCAN_LIMIT ? value.slice(0, SCAN_LIMIT) : value
    if (head.length < value.length) truncated = true
    if (!looksSecret(head, 'result')) return value
    const masked = mask(head, rules, 'result', NONE)
    if (masked.total === 0) return value
    for (const [kind, n] of Object.entries(masked.counts)) total.counts[kind] = (total.counts[kind] ?? 0) + n
    for (const entry of masked.entries) if (!total.entries.includes(entry)) total.entries.push(entry)
    total.total += masked.total
    changed = true
    return masked.text + value.slice(head.length)
  }

  const out = content.map((block): Block => {
    if (block.type === 'text' && typeof block.text === 'string') {
      const next = text(block.text)
      return next === block.text ? block : { ...block, text: next }
    }
    if (block.type !== 'tool_result') return block
    if (typeof block.content === 'string') {
      const next = text(block.content)
      return next === block.content ? block : { ...block, content: next }
    }
    if (!Array.isArray(block.content)) return block
    let inner = false
    const parts = (block.content as Block[]).map((part): Block => {
      if (part.type !== 'text' || typeof part.text !== 'string') return part
      const next = text(part.text)
      if (next === part.text) return part
      inner = true
      return { ...part, text: next }
    })
    return inner ? { ...block, content: parts } : block
  })

  return { content: redact && changed ? out : (content as Block[]), masked: total, truncated }
}

// How a warning is shown: a toast, the one summary after the third, or nothing.
export type Rate = 'toast' | 'summary' | 'skip'

// At most `max` toasts per `windowMs`, then one summary until the window frees up.
export function createLimiter(now: () => number = Date.now, max = 3, windowMs = 60_000): () => Rate {
  let shown: number[] = []
  let summarized = false
  return () => {
    const t = now()
    shown = shown.filter(at => t - at < windowMs)
    if (shown.length < max) {
      shown.push(t)
      summarized = false
      return 'toast'
    }
    if (summarized) return 'skip'
    summarized = true
    return 'summary'
  }
}
