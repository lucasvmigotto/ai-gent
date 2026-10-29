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
  const reason = 'repo guard:' + ' ' + "this user's issues, work items, pull requests and comments carry no sign-off or mention of the agent that wrote them; remove it and run the command again"
  const toasts = world(on, { deny: 'PreToolUse:Bash hook error: ' + reason })
  const ran = await $.tool.call({ tool: 'Bash', command: "reporun github --write -- pr comment 1 --body 'Co-Authored-By: x'" })
  expect(ran.isError).toBe(true)
  expect(ran.text).toContain("this user's issues, work items, pull requests and comments carry no sign-off or mention of the agent that wrote them; remove it and run the command again")
  expect(toasts.length).toBe(1)
  expect(toasts[0]).toContain('repo guard refused this')
})

test('an ask still goes on to the person, and nothing is denied', async ($, on) => {
  world(on, { ask: 'repo guard: reporun --write changes an issue, work item or pull request on the platform; confirm this exact change' })
  const ran = await $.tool.call({ tool: 'Bash', command: 'reporun github --write -- pr merge 1' })
  expect(ran.deny).toBeUndefined()
})

test('a call the guard lets through is untouched, and nothing is shown', async ($, on) => {
  const toasts = world(on, {})
  const ran = await $.tool.call({ tool: 'Bash', command: 'ls' })
  expect(ran.isError).not.toBe(true)
  expect(toasts).toEqual([])
})

test('guardUx off registers nothing', { options: { guardUx: false } }, async ($, on) => {
  const toasts = world(on, { deny: 'repo guard:' + ' ' + "this user's issues, work items, pull requests and comments carry no sign-off or mention of the agent that wrote them; remove it and run the command again" })
  await $.tool.call({ tool: 'Bash', command: "reporun github --write -- pr comment 1 --body 'Co-Authored-By: x'" })
  expect(toasts).toEqual([])
})
