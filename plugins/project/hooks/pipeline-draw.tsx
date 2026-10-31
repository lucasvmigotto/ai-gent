// The pipeline pane's dashboard: the structured model of pipeline-view.ts drawn
// with Box, Text and Button from `$.ui.resolve(e)` only (spec 029). It reads no
// file, runs nothing and writes no atom while drawing; a press goes to a handler.
//
// Safe choices for what only an interactive session can show: the progress bar
// is a run of block characters (no empty Box with a background), a pill is
// coloured, bold text with its word (no background), and each Button holds one
// cell. Colour never stands alone: every coloured run has a glyph or a word.

import type { EngineInterface, ResolveInput } from 'claude-code'

import { GLYPH } from './pipeline-view'
import type { Cell, Column, PipelineView, Row, Section, Tone } from './pipeline-view'

export type Handlers = {
  // Fill the prompt with /project:status (never submits).
  onStatus: () => void
  // Show or hide the open items.
  onMarkers: () => void
}

const COLOR: Partial<Record<Tone, string>> = { good: 'success', warn: 'warning', bad: 'error', info: 'suggestion' }

// A tone as Text props: a theme key, dim or bold; never a raw colour.
function look(tone: Tone | undefined): { color?: string; dimColor?: boolean; bold?: boolean } {
  if (tone === undefined || tone === 'plain') return {}
  if (tone === 'dim') return { dimColor: true }
  if (tone === 'bold') return { bold: true }
  return { color: COLOR[tone] }
}

const words = (cell: Cell): string => (cell.glyph ? `${cell.glyph} ${cell.text}` : cell.text)

export function drawDashboard($: EngineInterface, e: ResolveInput, m: PipelineView, handlers: Handlers) {
  const { Box, Button, Text } = $.ui.resolve(e)
  const ascii = m.ascii
  const sep = GLYPH.sep[ascii ? 1 : 0]
  const gap = m.mode === 'full' ? 2 : 1

  const title = (text: string) => <Text bold>{text}</Text>

  const cellBox = (cell: Cell, column: Column, key: string, isHead = false) => (
    <Box key={key} width={column.width} flexShrink={0} justifyContent={column.align === 'right' ? 'flex-end' : 'flex-start'}>
      <Text wrap="truncate-end" {...(isHead ? { bold: true, dimColor: true } : look(cell.tone))}>
        {words(cell)}
      </Text>
    </Box>
  )

  const head = (section: Section, edge: boolean) => (
    <Box key={`${section.id}:head`} gap={gap}>
      {edge ? <Box width={1} /> : null}
      {section.columns.map(c => cellBox({ text: c.label }, c, `${section.id}:head:${c.key}`, true))}
    </Box>
  )

  const row = (section: Section, r: Row, edge: boolean) => (
    <Box key={r.key} flexDirection="column">
      <Box gap={gap}>
        {edge ? (
          <Box width={1} flexShrink={0}>
            <Text {...look(r.edge)}>{GLYPH.edge[ascii ? 1 : 0]}</Text>
          </Box>
        ) : null}
        {r.cells.map((cell, i) => cellBox(cell, section.columns[i] as Column, `${r.key}:${section.columns[i]?.key}`))}
      </Box>
      {r.sub ? <Text dimColor>{`${edge ? '  ' : ''}${r.sub}`}</Text> : null}
    </Box>
  )

  const by = (id: string): Section => m.sections.find(s => s.id === id) as Section
  const artifacts = by('artifacts')
  const features = by('features')
  const items = by('items')
  const badgeTone: Tone = m.header.badge === 'LIVE' ? 'good' : 'warn'
  const nextTone: Tone = m.next.badge === 'Needs attention' ? 'warn' : m.next.badge === 'Suggested' ? 'info' : 'good'
  const isOpen = items.rows.some(r => r.isOpen)

  return (
    <Box key="pipeline" flexDirection="column" gap={1}>
      <Box key="header" justifyContent="space-between">
        {title(m.header.title)}
        <Box gap={1}>
          <Text bold {...look(badgeTone)}>{` ${m.header.badge} `}</Text>
          <Text dimColor>{m.header.caption}</Text>
        </Box>
      </Box>

      <Box key="summary" flexDirection="column">
        {m.mode === 'full' ? (
          <Box flexDirection="column">
            <Box gap={4}>
              {m.summary.metrics.filter(x => x.isLarge).map(x => (
                <Box key={`metric:${x.key}`} flexDirection="column">
                  <Text bold size="title" {...look(x.tone)}>{String(x.value)}</Text>
                  <Text dimColor>{x.label}</Text>
                </Box>
              ))}
            </Box>
            <Text dimColor>{m.summary.metrics.filter(x => !x.isLarge).map(x => `${x.value} ${x.label}`).join(` ${sep} `)}</Text>
          </Box>
        ) : (
          <Text>{m.summary.line}</Text>
        )}
      </Box>

      <Box key="artifacts" flexDirection="column">
        {title('Artifacts')}
        {head(artifacts, true)}
        {artifacts.rows.map(r => row(artifacts, r, true))}
        {!m.hasGit ? <Text dimColor>Staleness needs git.</Text> : null}
        <Box key="stages" gap={2} flexWrap="wrap">
          <Text dimColor>Stages (an aid)</Text>
          {m.stages.map(s => (
            <Text key={`stage:${s.key}`} {...look(s.tone)} bold={s.state === 'current'}>{`${s.glyph} ${s.label}`}</Text>
          ))}
        </Box>
      </Box>

      <Box key="progress" flexDirection="column">
        {title('Implementation')}
        {m.progress.total === 0 ? (
          <Text dimColor>{features.empty ?? 'No features.'}</Text>
        ) : (
          <Box flexDirection="column">
            <Box key="bar">
              {m.progress.bar.map(seg => (
                <Text key={`bar:${seg.key}`} {...look(seg.tone)}>{seg.glyph.repeat(seg.length)}</Text>
              ))}
            </Box>
            <Box gap={2}>
              {m.progress.pills.map(p => (
                <Text key={`pill:${p.key}`} {...look(p.tone)}>{p.text}</Text>
              ))}
              <Text dimColor>{`of ${m.progress.total}`}</Text>
            </Box>
          </Box>
        )}
      </Box>

      <Box key="features" flexDirection="column">
        {title('Features')}
        <Box gap={2}>
          {(['all', 'ready', 'blocked', 'done'] as const).map(f => (
            <Text key={`filter:${f}`} bold={f === m.filter.active} dimColor={f !== m.filter.active}>
              {`${f === m.filter.active ? `${GLYPH.implemented[ascii ? 1 : 0]} ` : ''}${f[0]?.toUpperCase()}${f.slice(1)} ${m.filter.counts[f]}`}
            </Text>
          ))}
        </Box>
        {features.rows.length > 0 ? head(features, false) : null}
        {features.rows.map(r => row(features, r, false))}
        {features.empty ? <Text dimColor>{features.empty}</Text> : null}
        {features.more ? <Text dimColor>{features.more}</Text> : null}
        {m.mode === 'condensed' && features.rows.length > 0 ? (
          <Text dimColor>
            {[['planned', 'Planned'], ['progress', 'In progress'], ['partial', 'Partial'], ['implemented', 'Implemented'], ['verified', 'Verified'], ['none', 'N/A']]
              .map(([key, label]) => `${GLYPH[key as 'planned'][ascii ? 1 : 0]} ${label}`)
              .join('  ')}
          </Text>
        ) : null}
      </Box>

      <Box key="items" flexDirection="column">
        <Box gap={1}>
          {title('Open items')}
          {items.rows.some(r => (r.cells[1]?.text ?? '0') !== '0') ? (
            <Button key="button-markers" label={isOpen ? 'Hide' : 'Show'} onPress={handlers.onMarkers} />
          ) : null}
        </Box>
        {items.rows.map(r => (
          <Box key={r.key} flexDirection="column">
            <Box gap={1}>
              <Box width={16}><Text>{r.cells[0]?.text}</Text></Box>
              <Text {...look(r.cells[1]?.tone)}>{r.cells[1]?.text}</Text>
            </Box>
            {(r.detail ?? []).map((line, i) => (
              <Text key={`${r.key}:${i}`} wrap="truncate-end" dimColor>{`  ${line.map(c => c.text).join(' ')}`}</Text>
            ))}
          </Box>
        ))}
      </Box>

      <Box key="next" flexDirection="column">
        <Box justifyContent="space-between">
          {title('Next action')}
          <Text bold {...look(nextTone)}>{` ${m.next.badge} `}</Text>
        </Box>
        <Text>{m.next.text}</Text>
        {m.next.ref ? <Text dimColor>{m.next.ref}</Text> : null}
        <Box gap={1}>
          <Text dimColor>{m.next.hint}</Text>
          <Button key="button-status" label={m.next.command} onPress={handlers.onStatus} />
        </Box>
      </Box>
    </Box>
  )
}
