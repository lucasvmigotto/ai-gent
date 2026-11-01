import { expect, test } from 'claude-code/testing'

import { hookLines, leftoversBlock } from './hook-lines'

type Handlers = Parameters<Parameters<typeof test>[1]>[1]

const REPORT = 'SessionStart:startup hook success: ai-gent containers (report only, nothing removed):\ndisk: 12 GB free on the container store\n2 dangling images\n</system-reminder>'

const row = (text: string, door = 'hook-context', origin: object = { kind: 'hook', event: 'SessionStart' }) =>
  ({ message: { type: 'attachment', name: 'hook_success', role: 'user', content: [{ type: 'text', text }] }, door, origin, uuid: 'u1' }) as never

// Nothing sits beneath the plugin: this hook stands for the engine's toast.
function toasts(on: Handlers): string[] {
  const shown: string[] = []
  on('ui.toast', (_, e) => {
    shown.push(e.text)
    return { value: undefined }
  })
  return shown
}

test('the report leftovers.sh printed is read from the stored row, in its own words', () => {
  expect(leftoversBlock(hookLines(REPORT))).toBe('ai-gent containers (report only, nothing removed): disk: 12 GB free on the container store · 2 dangling images')
  expect(leftoversBlock(hookLines('SessionStart:startup hook success: [ai-gent] update available'))).toBeNull()
})

test('the report is shown once as a toast; other rows and other hooks leave it alone', async ($, on) => {
  const shown = toasts(on)
  await $.session.append(row(REPORT))
  await $.session.append(row(REPORT))
  expect(shown).toEqual(['ai-gent containers (report only, nothing removed): disk: 12 GB free on the container store · 2 dangling images'])
})

test('a prompt row, or a row from another event, shows nothing', async ($, on) => {
  const shown = toasts(on)
  await $.session.append(row(REPORT, 'prompt', { kind: 'unclassified' }))
  await $.session.append(row(REPORT, 'hook-context', { kind: 'hook', event: 'PostToolUse' }))
  await $.session.append(row('SessionStart:startup hook success: nothing to report'))
  expect(shown).toEqual([])
})

test('leftoversToast off shows nothing', { options: { leftoversToast: false } }, async ($, on) => {
  const shown = toasts(on)
  await $.session.append(row(REPORT))
  expect(shown).toEqual([])
})
