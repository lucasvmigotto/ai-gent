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
  const reason = 'secrets guard:' + ' ' + 'that shows a secret value from pass; the agent sees names only (pass ls) — the runner that needs a secret reads it'
  const toasts = world(on, { deny: 'PreToolUse:Bash hook error: ' + reason })
  const ran = await $.tool.call({ tool: 'Bash', command: 'pass show ai-gent/db/x' })
  expect(ran.isError).toBe(true)
  expect(ran.text).toContain('that shows a secret value from pass; the agent sees names only (pass ls) — the runner that needs a secret reads it')
  expect(toasts.length).toBe(1)
  expect(toasts[0]).toContain('secrets guard refused this')
})

test('an ask still goes on to the person, and nothing is denied', async ($, on) => {
  world(on, { ask: 'secrets guard: pass rm changes the password store; the user decides' })
  const ran = await $.tool.call({ tool: 'Bash', command: 'pass rm ai-gent/db/x' })
  expect(ran.deny).toBeUndefined()
})

test('a call the guard lets through is untouched, and nothing is shown', async ($, on) => {
  const toasts = world(on, {})
  const ran = await $.tool.call({ tool: 'Bash', command: 'ls' })
  expect(ran.isError).not.toBe(true)
  expect(toasts).toEqual([])
})

test('guardUx off registers nothing', { options: { guardUx: false } }, async ($, on) => {
  const toasts = world(on, { deny: 'secrets guard:' + ' ' + 'that shows a secret value from pass; the agent sees names only (pass ls) — the runner that needs a secret reads it' })
  await $.tool.call({ tool: 'Bash', command: 'pass show ai-gent/db/x' })
  expect(toasts).toEqual([])
})
