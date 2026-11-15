// The workers pane drawn: the lines of workers-model.ts with Box, Text and Button from
// `$.ui.resolve(e)` only. It reads no file, runs nothing and writes no atom while drawing;
// a press goes to a handler. Colour never stands alone: every row has a glyph and the state word.

import type { EngineInterface, ResolveInput } from 'claude-code'

import type { Line, Tone, WorkersView } from './workers-model'

const COLOR: Partial<Record<Tone, string>> = { good: 'success', warn: 'warning', bad: 'error', info: 'suggestion' }

function look(tone: Tone): { color?: string; dimColor?: boolean; bold?: boolean } {
  if (tone === 'plain') return {}
  if (tone === 'dim') return { dimColor: true }
  if (tone === 'bold') return { bold: true }
  return { color: COLOR[tone] }
}

// A line's text cut at its marks: [text, tone] pieces that concatenate to the text.
export function segments(line: Line): [string, Tone][] {
  const out: [string, Tone][] = []
  let at = 0
  for (const m of [...(line.marks ?? [])].sort((a, b) => a.from - b.from)) {
    if (m.from > at) out.push([line.text.slice(at, m.from), line.tone])
    out.push([line.text.slice(Math.max(at, m.from), m.to), m.tone])
    at = Math.max(at, m.to)
  }
  if (at < line.text.length) out.push([line.text.slice(at), line.tone])
  return out
}

export function drawWorkers($: EngineInterface, e: ResolveInput, view: WorkersView, handlers: { onToggle: (track: string) => void }) {
  const { Box, Button, Text } = $.ui.resolve(e)
  return (
    <Box key="workers" flexDirection="column">
      {view.lines.map(line =>
        line.toggle === undefined && line.marks !== undefined && line.marks.length > 0 ? (
          <Box key={line.key} flexDirection="row">
            {segments(line).map(([piece, tone], i) => (
              <Text key={`${line.key}:${i}`} {...look(tone)}>
                {piece}
              </Text>
            ))}
          </Box>
        ) : line.toggle === undefined ? (
          <Text key={line.key} wrap="truncate-end" {...look(line.tone)}>
            {line.text === '' ? ' ' : line.text}
          </Text>
        ) : (
          <Button key={line.buttonKey ?? `toggle:${line.toggle}`} plain onPress={() => handlers.onToggle(line.toggle as string)}>
            <Text wrap="truncate-end" {...look(line.tone)}>
              {line.text}
            </Text>
          </Button>
        ),
      )}
    </Box>
  )
}
