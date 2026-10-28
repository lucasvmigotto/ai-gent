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
  const reason = 'git:workflow guard:' + ' ' + 'hooks must not be bypassed (--no-verify / -n); fix what the hook reports and try again'
  const toasts = world(on, { deny: 'PreToolUse:Bash hook error: ' + reason })
  const ran = await $.tool.call({ tool: 'Bash', command: 'git commit --no-verify -m x' })
  expect(ran.isError).toBe(true)
  expect(ran.text).toContain('hooks must not be bypassed (--no-verify / -n); fix what the hook reports and try again')
  expect(toasts.length).toBe(1)
  expect(toasts[0]).toContain('git:workflow guard refused this')
})

test('an ask still goes on to the person, and nothing is denied', async ($, on) => {
  world(on, { ask: 'git:workflow guard: git push publishes; allow only if the user explicitly asked for this push' })
  const ran = await $.tool.call({ tool: 'Bash', command: 'git push origin dev' })
  expect(ran.deny).toBeUndefined()
})

test('a call the guard lets through is untouched, and nothing is shown', async ($, on) => {
  const toasts = world(on, {})
  const ran = await $.tool.call({ tool: 'Bash', command: 'ls' })
  expect(ran.isError).not.toBe(true)
  expect(toasts).toEqual([])
})

test('guardUx off registers nothing', { options: { guardUx: false } }, async ($, on) => {
  const toasts = world(on, { deny: 'git:workflow guard:' + ' ' + 'hooks must not be bypassed (--no-verify / -n); fix what the hook reports and try again' })
  await $.tool.call({ tool: 'Bash', command: 'git commit --no-verify -m x' })
  expect(toasts).toEqual([])
})
