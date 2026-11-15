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
import type { Cell, Column, Filter, PipelineView, Row, Section, Tone } from './pipeline-view'

export type Handlers = {
  // Fill the prompt with /project:status (never submits).
  onStatus: () => void
  // Show or hide the open items.
  onMarkers: () => void
  // Filter the matrix by readiness.
  onFilter: (filter: Filter) => void
  // Open or close a row, by its key (an artifact, a feature or one kind of open item).
  onToggle: (key: string) => void
  // Fill the prompt with a command the recommendation names (never submits).
  onFill: (command: string) => void
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

  // `toggle` makes the cell the press target that opens its row: a plain
  // Button holds one cell's text, so the row itself needs no wrapper.
  const cellBox = (cell: Cell, column: Column, key: string, isHead = false, toggle?: string) => (
    <Box key={key} width={column.width} flexShrink={0} justifyContent={column.align === 'right' ? 'flex-end' : 'flex-start'}>
      {toggle ? (
        <Button key={`toggle:${toggle}`} label={words(cell)} plain dimColor={cell.tone === 'dim'} onPress={() => handlers.onToggle(toggle)} />
      ) : (
        <Text wrap="truncate-end" {...(isHead ? { bold: true, dimColor: true } : look(cell.tone))}>
          {words(cell)}
        </Text>
      )}
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
        {r.cells.map((cell, i) => cellBox(cell, section.columns[i] as Column, `${r.key}:${section.columns[i]?.key}`, false, i === 0 ? r.key : undefined))}
      </Box>
      {r.sub ? <Text dimColor wrap="truncate-end">{`${edge ? '  ' : ''}  ${r.sub}`}</Text> : null}
      {(r.detail ?? []).map((line, i) => (
        <Box key={`${r.key}:detail:${i}`} gap={1}>
          <Text>{`${edge ? '  ' : ''}    `}</Text>
          {line.map((c, j) => (
            <Text key={`${r.key}:detail:${i}:${j}`} wrap="truncate-end" {...look(c.tone)}>{c.text}</Text>
          ))}
        </Box>
      ))}
    </Box>
  )

  const by = (id: string): Section => m.sections.find(s => s.id === id) as Section
  const artifacts = by('artifacts')
  const features = by('features')
  const items = by('items')
  const badgeTone: Tone = m.header.badge === 'LIVE' ? 'good' : 'warn'
  const nextTone: Tone = m.next.badge === 'Blocking' ? 'bad' : m.next.badge === 'Suggested' ? 'info' : 'good'
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
        <Box gap={1} flexWrap="wrap">
          {m.summary.counts.flatMap((x, i) => [
            ...(i > 0 ? [<Text key={`count-sep:${x.key}`} dimColor>{sep}</Text>] : []),
            <Text key={`count:${x.key}`} {...look(x.tone)}>{`${x.value} ${x.label}`}</Text>,
          ])}
        </Box>
        {m.summary.attention ? <Text wrap="wrap" {...look(m.summary.attention.tone)}>{m.summary.attention.text}</Text> : null}
      </Box>

      {m.blockers ? (
        <Box key="blockers" flexDirection="column">
          <Box gap={2}>
            {title(`BLOCKERS ${sep} ${m.blockers.count}`)}
            <Button key="button-blockers" plain label={`${GLYPH[m.blockers.isOpen ? 'open' : 'closed'][ascii ? 1 : 0]} ${m.blockers.isOpen ? 'Hide' : 'Details'}`} onPress={() => handlers.onToggle('blockers')} />
          </Box>
          {m.blockers.rows.map(r => (
            <Box key={r.key} flexDirection="column">
              <Text wrap="truncate-end" color="error">{`${GLYPH.warn[ascii ? 1 : 0]} ${r.title} ${ascii ? '-' : '—'} ${r.reason}${r.isNext ? ' (see Next action)' : ''}`}</Text>
              {m.blockers?.isOpen ? r.detail.map((d, i) => <Text key={`${r.key}:${i}`} dimColor wrap="truncate-end">{`    ${d}`}</Text>) : null}
            </Box>
          ))}
          {m.blockers.more > 0 ? <Text dimColor>{`+${m.blockers.more} more`}</Text> : null}
        </Box>
      ) : null}

      <Box key="artifacts" flexDirection="column">
        <Box gap={2}>
          {title('Artifacts')}
          <Text dimColor>{`${m.summary.artifacts} of ${m.summary.ofArtifacts} ready`}</Text>
        </Box>
        {head(artifacts, true)}
        {artifacts.rows.map(r => row(artifacts, r, true))}
        {!m.hasGit ? <Text dimColor>Staleness needs git.</Text> : null}
        <Box key="stages" gap={m.stageArrow ? 1 : 2} flexWrap="wrap">
          <Text dimColor>Stages (an aid)</Text>
          {m.stages.flatMap((s, i) => [
            ...(m.stageArrow && i > 0 ? [<Text key={`arrow:${s.key}`} dimColor>{m.stageArrow}</Text>] : []),
            <Text key={`stage:${s.key}`} {...look(s.tone)} bold={s.state === 'current'}>{`${s.glyph} ${s.label}`}</Text>,
          ])}
          {m.stageDetail ? (
            <Button key="button-stage" plain label={`${GLYPH[m.stageDetail.isOpen ? 'open' : 'closed'][ascii ? 1 : 0]} Readiness`} onPress={() => handlers.onToggle('stage')} />
          ) : null}
        </Box>
        {m.stageDetail?.isOpen ? (
          <Box key="stage-detail" flexDirection="column">
            <Text dimColor>{`  ${m.stageDetail.label}: what the strip reads`}</Text>
            {m.stageDetail.checks.map((c, i) => (
              <Text key={`check:${i}`} wrap="truncate-end" {...look(c.tone)}>{`    ${c.glyph} ${c.text}`}</Text>
            ))}
          </Box>
        ) : null}
      </Box>

      <Box key="progress" flexDirection="column">
        <Box key="progress-head" gap={3}>
          {title('Implementation')}
          {m.progress.total > 0 ? <Text bold>{`${m.progress.percent}%`}</Text> : null}
        </Box>
        {m.progress.total === 0 ? (
          <Text dimColor>{features.empty ?? 'No features.'}</Text>
        ) : (
          <Box flexDirection="column">
            <Box key="bar">
              {m.progress.bar.map(seg => (
                <Text key={`bar:${seg.key}`} {...look(seg.tone)}>{seg.glyph.repeat(seg.length)}</Text>
              ))}
            </Box>
            <Box gap={1}>
              {m.progress.pills.flatMap((p, i) => [
                ...(i > 0 ? [<Text key={`pill-sep:${p.key}`} dimColor>{sep}</Text>] : []),
                <Text key={`pill:${p.key}`} {...look(p.tone)}>{p.text}</Text>,
              ])}
            </Box>
          </Box>
        )}
      </Box>

      <Box key="features" flexDirection="column">
        <Box key="features-head" gap={2}>
          {title('Features')}
          {m.featuresOpen ? null : (
            <Text dimColor>
              {[`${m.filter.counts.all} ${m.filter.counts.all === 1 ? 'feature' : 'features'}`, `Ready ${m.filter.counts.ready}`, `Blocked ${m.filter.counts.blocked}`, `Done ${m.filter.counts.done}`].join(` ${sep} `)}
            </Text>
          )}
          <Button
            key="button-features"
            plain
            label={`${GLYPH[m.featuresOpen ? 'open' : 'closed'][ascii ? 1 : 0]} ${m.featuresOpen ? 'Hide' : 'Show'}`}
            onPress={() => handlers.onToggle('features')}
          />
        </Box>
        {m.featuresOpen ? (
          <Box key="features-body" flexDirection="column">
        <Box gap={2}>
          {(['all', 'ready', 'blocked', 'done'] as const).map(f => (
            <Button
              key={`filter:${f}`}
              plain
              dimColor={f !== m.filter.active}
              label={`${f === m.filter.active ? `${GLYPH.implemented[ascii ? 1 : 0]} ` : ''}${f[0]?.toUpperCase()}${f.slice(1)} ${m.filter.counts[f]}`}
              onPress={() => handlers.onFilter(f)}
            />
          ))}
        </Box>
        {features.rows.length > 0 ? head(features, false) : null}
        {features.rows.map(r => row(features, r, false))}
        {features.empty ? <Text dimColor>{features.empty}</Text> : null}
        {features.more ? <Text dimColor>{features.more}</Text> : null}
        {!m.matrix.words && features.rows.length > 0 ? (
          <Text dimColor>
            {[['planned', 'Planned'], ['progress', 'In progress'], ['partial', 'Partial'], ['implemented', 'Implemented'], ['verified', 'Verified'], ['none', 'N/A']]
              .map(([key, label]) => `${GLYPH[key as 'planned'][ascii ? 1 : 0]} ${label}`)
              .join('  ')}
          </Text>
        ) : null}
          </Box>
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
              <Box width={16}>
                {(r.cells[1]?.text ?? '0') !== '0' ? (
                  <Button key={`toggle:${r.key}`} plain label={r.cells[0]?.text ?? ''} onPress={() => handlers.onToggle(`markers:${r.key.slice('marker:'.length)}`)} />
                ) : (
                  <Text>{r.cells[0]?.text}</Text>
                )}
              </Box>
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
        <Text bold {...look(m.next.badge === 'Blocking' ? 'bad' : undefined)}>{`${GLYPH[m.next.badge === 'Blocking' ? 'warn' : 'current'][ascii ? 1 : 0]} ${m.next.title}`}</Text>
        {m.next.explanation ? <Text>{`  ${m.next.explanation}`}</Text> : null}
        {m.next.full ? (
          <Box>
            <Text>{'  '}</Text>
            <Button key="button-next-details" plain label={`${GLYPH[m.next.isOpen ? 'open' : 'closed'][ascii ? 1 : 0]} ${m.next.isOpen ? 'Less' : 'Details'}`} onPress={() => handlers.onToggle('next')} />
          </Box>
        ) : null}
        {m.next.where ? <Text dimColor wrap="truncate-end">{`  ${m.next.where}`}</Text> : null}
        {!m.next.action && m.next.instruction ? <Text dimColor>{`  ${m.next.instruction}`}</Text> : null}
        {m.next.action ? (
          <Box gap={1}>
            <Text dimColor>{'  Run'}</Text>
            <Button key="button-next" label={m.next.action} onPress={() => handlers.onFill(m.next.action ?? '')} />
          </Box>
        ) : null}
        <Box gap={1}>
          <Text dimColor>{m.next.hint}</Text>
          <Button key="button-status" label={m.next.command} onPress={handlers.onStatus} />
        </Box>
      </Box>
    </Box>
  )
}
