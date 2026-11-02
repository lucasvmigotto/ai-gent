// The workers pane drawn: the lines of workers-model.ts with Box, Text and Button from
// `$.ui.resolve(e)` only. It reads no file, runs nothing and writes no atom while drawing;
// a press goes to a handler. Colour never stands alone: every row has a glyph and the state word.

import type { EngineInterface, ResolveInput } from 'claude-code'

import type { Tone, WorkersView } from './workers-model'

const COLOR: Partial<Record<Tone, string>> = { good: 'success', warn: 'warning', bad: 'error', info: 'suggestion' }

function look(tone: Tone): { color?: string; dimColor?: boolean; bold?: boolean } {
  if (tone === 'plain') return {}
  if (tone === 'dim') return { dimColor: true }
  if (tone === 'bold') return { bold: true }
  return { color: COLOR[tone] }
}

export function drawWorkers($: EngineInterface, e: ResolveInput, view: WorkersView, handlers: { onToggle: (track: string) => void }) {
  const { Box, Button, Text } = $.ui.resolve(e)
  return (
    <Box key="workers" flexDirection="column">
      {view.lines.map(line =>
        line.toggle === undefined ? (
          <Text key={line.key} wrap="truncate-end" {...look(line.tone)}>
            {line.text}
          </Text>
        ) : (
          <Button key={`toggle:${line.toggle}`} label={line.text} plain dimColor={line.tone === 'dim'} onPress={() => handlers.onToggle(line.toggle as string)} />
        ),
      )}
    </Box>
  )
}
