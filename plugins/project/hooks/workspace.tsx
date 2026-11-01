// The workspace line above the prompt, drawn (specs/031-session-chrome): the model of
// workspace-model.ts with Box, Text and Button from `$.ui.resolve(e)` only. It reads no file,
// runs nothing and writes no atom while drawing; the Details press goes to a handler.

import type { EngineInterface, ResolveInput } from 'claude-code'

import { toggleGlyph } from './workspace-model'
import type { WorkspaceView } from './workspace-model'

export function drawWorkspace($: EngineInterface, e: ResolveInput, view: WorkspaceView, options: { isOpen: boolean; ascii: boolean; onToggle: () => void }) {
  const { Box, Button, Text } = $.ui.resolve(e)
  const toggle = <Button key="workspace-details" label={`${toggleGlyph(options.isOpen, options.ascii)} Details`} onPress={options.onToggle} />
  return (
    <Box flexDirection="column">
      {view.line === null ? null : (
        <Box>
          <Text wrap="truncate-end">{view.line}</Text>
          <Text> </Text>
          {view.warnings === null ? toggle : null}
        </Box>
      )}
      {view.warnings === null ? null : (
        <Box>
          <Text color="warning" wrap="truncate-end">
            {view.warnings}
          </Text>
          <Text> </Text>
          {toggle}
        </Box>
      )}
      {options.isOpen
        ? view.details.map(row => (
            <Text dimColor wrap="truncate-end">
              {`  ${row}`}
            </Text>
          ))
        : null}
    </Box>
  )
}
