// The workspace line above the prompt, drawn (specs/031-session-chrome): the model of
// workspace-model.ts with Box, Text and Button from `$.ui.resolve(e)` only. It reads no file,
// runs nothing and writes no atom while drawing; the Details press goes to a handler.

import type { EngineInterface, ResolveInput } from 'claude-code'

import { toggleGlyph } from './workspace-model'
import type { WorkspaceView } from './workspace-model'

export function drawWorkspace($: EngineInterface, e: ResolveInput, view: WorkspaceView, options: { isOpen: boolean; ascii: boolean; onToggle: () => void }) {
  const { Box, Button, Text } = $.ui.resolve(e)
  const toggle = <Button key="workspace-details" label={`${toggleGlyph(options.isOpen, options.ascii)} Details`} onPress={options.onToggle} />
  // The model and effort sit at the right of the row that holds the Details button.
  const tail = (isHere: boolean) =>
    isHere && view.tail !== null ? [<Box key="workspace-fill" flexGrow={1} />, <Text key="workspace-model" dimColor>{view.tail}</Text>] : null
  return (
    <Box flexDirection="column">
      {view.line === null && view.warnings === null ? (
        <Box>
          {toggle}
          {tail(true)}
        </Box>
      ) : null}
      {view.line === null ? null : (
        <Box>
          <Text wrap="truncate-end">{view.line}</Text>
          <Text> </Text>
          {view.warnings === null ? toggle : null}
          {tail(view.warnings === null)}
        </Box>
      )}
      {view.warnings === null ? null : (
        <Box>
          <Text color="warning" wrap="truncate-end">
            {view.warnings}
          </Text>
          <Text> </Text>
          {toggle}
          {tail(true)}
        </Box>
      )}
      {options.isOpen
        ? (view.details.length === 0 ? ['Nothing to report'] : view.details).map(row => (
            <Text dimColor wrap="truncate-end">
              {`  ${row}`}
            </Text>
          ))
        : null}
    </Box>
  )
}
