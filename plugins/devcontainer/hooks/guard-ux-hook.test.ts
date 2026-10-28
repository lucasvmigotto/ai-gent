import { expect, test } from 'claude-code/testing'

type Handlers = Parameters<Parameters<typeof test>[1]>[1]

// Nothing sits beneath the plugin but the guard's answer, stood for here.
function world(on: Handlers, answer: { deny?: string; ask?: string }) {
  const toasts: string[] = []
  on('ui.toast', (_, e) => {
    toasts.push(e.text)
    return { value: undefined }
  })
  on('classic.PreToolUse', () => answer)
  on('tool.call', () => ({ result: {} }))
  return toasts
}

test('a deny still ends the call with the guard\'s reason, and the person is told why', async ($, on) => {
  const reason = 'containers guard:' + ' ' + "prune removes other projects' images, volumes and in-flight builds; remove by ID or by this project's label (containers.md §9a)"
  const toasts = world(on, { deny: 'PreToolUse:Bash hook error: ' + reason })
  const ran = await $.tool.call({ tool: 'Bash', command: 'podman system prune -af' })
  expect(ran.isError).toBe(true)
  expect(ran.text).toContain("prune removes other projects' images, volumes and in-flight builds; remove by ID or by this project's label (containers.md §9a)")
  expect(toasts.length).toBe(1)
  expect(toasts[0]).toContain('containers guard refused this')
})

test('a call the guard lets through is untouched, and nothing is shown', async ($, on) => {
  const toasts = world(on, {})
  const ran = await $.tool.call({ tool: 'Bash', command: 'ls' })
  expect(ran.isError).not.toBe(true)
  expect(toasts).toEqual([])
})

test('guardUx off registers nothing', { options: { guardUx: false } }, async ($, on) => {
  const toasts = world(on, { deny: 'containers guard:' + ' ' + "prune removes other projects' images, volumes and in-flight builds; remove by ID or by this project's label (containers.md §9a)" })
  await $.tool.call({ tool: 'Bash', command: 'podman system prune -af' })
  expect(toasts).toEqual([])
})
