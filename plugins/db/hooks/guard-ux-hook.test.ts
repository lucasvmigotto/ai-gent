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
  const reason = 'db guard:' + ' ' + 'a direct psql call with SQL that writes; use dbrun (plan/apply on a proven local database, script for anything else)'
  const toasts = world(on, { deny: 'PreToolUse:Bash hook error: ' + reason })
  const ran = await $.tool.call({ tool: 'Bash', command: "psql -c 'delete from t'" })
  expect(ran.isError).toBe(true)
  expect(ran.text).toContain('a direct psql call with SQL that writes; use dbrun (plan/apply on a proven local database, script for anything else)')
  expect(toasts.length).toBe(1)
  expect(toasts[0]).toContain('db guard refused this')
})

test('an ask still goes on to the person, and nothing is denied', async ($, on) => {
  world(on, { ask: 'db guard: psql bypasses dbrun (no classification, masking or command log); allow only if the user wants this direct call' })
  const ran = await $.tool.call({ tool: 'Bash', command: "psql -c 'select 1'" })
  expect(ran.deny).toBeUndefined()
})

test('a call the guard lets through is untouched, and nothing is shown', async ($, on) => {
  const toasts = world(on, {})
  const ran = await $.tool.call({ tool: 'Bash', command: 'ls' })
  expect(ran.isError).not.toBe(true)
  expect(toasts).toEqual([])
})

test('guardUx off registers nothing', { options: { guardUx: false } }, async ($, on) => {
  const toasts = world(on, { deny: 'db guard:' + ' ' + 'a direct psql call with SQL that writes; use dbrun (plan/apply on a proven local database, script for anything else)' })
  await $.tool.call({ tool: 'Bash', command: "psql -c 'delete from t'" })
  expect(toasts).toEqual([])
})
