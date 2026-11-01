import { expect, mock, test } from 'claude-code/testing'

import type { GuardRow } from './guard-ux'

type Handlers = Parameters<Parameters<typeof test>[1]>[1]

// What `$.state` holds for the guardLog atom.
let kept: { value: unknown; version: number } = { value: undefined, version: 0 }

// Nothing sits beneath the plugin but the guard's answer, stood for here.
function world(on: Handlers, answer: { deny?: string; ask?: string }) {
  const toasts: string[] = []
  on('ui.toast', (_, e) => {
    toasts.push(e.text)
    return { value: undefined }
  })
  on('classic.PreToolUse', () => answer)
  on('tool.call', () => ({ result: {} }))
  mock.clock(on, { now: 1000 })
  kept = { value: undefined, version: 0 }
  on('state.get', (_, e, next) => (e.key !== 'guardLog' ? next(e) : { value: { value: kept.value, version: kept.version } }))
  on('state.set', (_, e, next) => {
    if (e.key !== 'guardLog') return next(e)
    if (e.ifVersion !== undefined && e.ifVersion !== kept.version) return { value: { isSet: false, version: kept.version } }
    kept = { value: e.value, version: kept.version + 1 }
    return { value: { isSet: true, version: kept.version } }
  })
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

// ---- the guard log (specs/024-guard-log)

const logged = (): GuardRow[] => {
  const value = kept.value as { value?: GuardRow[] } | GuardRow[] | undefined
  return (Array.isArray(value) ? value : value?.value) ?? []
}

// Credential shapes are put together here from fragments (the `vars` of
// shared/secret-cases.json), so no full token is committed.
const FRAGMENTS = [
  { parts: ['gh', 'p_'], fill: 'A', times: 36 },
  { parts: ['github', '_pat_'], fill: 'B', times: 30 },
  { parts: ['sk', '-'], fill: 'C', times: 24 },
  { parts: ['AK', 'IA'], fill: '7', times: 16 },
  { parts: ['xo', 'xb-'], fill: '9', times: 12 },
  { parts: ['eyJhbGci', 'OiJIUzI1NiIs'], fill: '', times: 0 },
  { parts: ['-----BEGIN OPENSSH PRIV', 'ATE KEY-----\nb3BlbnNzaC1rZXk\n-----END OPENSSH PRIV', 'ATE KEY-----'], fill: '', times: 0 },
]
const TOKENS = FRAGMENTS.map(v => v.parts.join('') + v.fill.repeat(v.times))

const MARKER = "db guard:"
const DENY_ENTRY = "with SQL that writes"
const ASK_ENTRY = "bypasses dbrun"
const DENY_CMD = "psql -c 'delete from t'"
const ASK_CMD = "psql -c 'select 1'"
const FOREIGN = "git:workflow guard: git push publishes"
const DENY = (extra = '') => `${MARKER} ${DENY_ENTRY}${extra}; try something else`
const ASK = (extra = '') => `${MARKER} ${ASK_ENTRY}${extra}; allow only if asked`

test('a deny and an ask each leave one row: entry and verb, nothing else', async ($, on) => {
  const answer: { deny?: string; ask?: string } = { deny: 'PreToolUse:Bash hook error: ' + DENY() }
  world(on, answer)
  await $.tool.call({ tool: 'Bash', command: DENY_CMD })
  delete answer.deny
  answer.ask = ASK()
  await $.tool.call({ tool: 'Bash', command: ASK_CMD })
  const rows = logged()
  expect(rows.map(r => [r.guard, r.decision, r.entry, r.tool, r.verb])).toEqual([
    ["db", 'deny', DENY_ENTRY, 'Bash', "psql"],
    ["db", 'ask', ASK_ENTRY, 'Bash', "psql"],
  ])
  expect(rows[1]?.outcome).toBe('asked')
})

test('a credential as a first word, an argument or inside a URL never reaches a row', async ($, on) => {
  world(on, { ask: ASK() })
  for (const token of TOKENS) {
    for (const command of [token, `${token} push`, `${ASK_CMD} ${token}`, `psql postgres://u:${token}@h/db`, `curl -H "Authorization: Bearer ${token}" https://h/${token}`]) {
      await $.tool.call({ tool: 'Bash', command })
    }
  }
  const rows = logged()
  expect(rows.length).toBe(TOKENS.length * 5)
  const text = JSON.stringify(rows)
  for (const token of TOKENS) {
    expect(text.includes(token)).toBe(false)
    expect(text.includes(token.slice(0, 10))).toBe(false)
  }
})

test('what the guard interpolated into its reason never reaches a row', async ($, on) => {
  world(on, { deny: 'PreToolUse:Bash hook error: ' + DENY(' on sentinel-branch-zq') })
  await $.tool.call({ tool: 'Bash', command: `${DENY_CMD} sentinel-arg-zq` })
  const text = JSON.stringify(logged())
  expect(text).not.toContain('sentinel')
  expect(text).toContain(DENY_ENTRY)
})

test('another guard\'s reason leaves no row', async ($, on) => {
  const answer: { deny?: string; ask?: string } = { ask: FOREIGN }
  world(on, answer)
  await $.tool.call({ tool: 'Bash', command: ASK_CMD })
  delete answer.ask
  answer.deny = 'PreToolUse:Bash hook error: ' + FOREIGN
  await $.tool.call({ tool: 'Bash', command: DENY_CMD })
  expect(logged()).toEqual([])
})

test('a call the guard lets through leaves no row', async ($, on) => {
  world(on, {})
  await $.tool.call({ tool: 'Bash', command: 'ls' })
  expect(logged()).toEqual([])
})

test('guardLog off keeps no row, and the decision and the words are unchanged', { options: { guardLog: false } }, async ($, on) => {
  const toasts = world(on, { deny: 'PreToolUse:Bash hook error: ' + DENY() })
  const ran = await $.tool.call({ tool: 'Bash', command: DENY_CMD })
  expect(ran.isError).toBe(true)
  expect(toasts.length).toBe(1)
  expect(logged()).toEqual([])
})

test('guardUx off still records, and explains nothing', { options: { guardUx: false } }, async ($, on) => {
  const toasts = world(on, { deny: 'PreToolUse:Bash hook error: ' + DENY() })
  const ran = await $.tool.call({ tool: 'Bash', command: DENY_CMD })
  expect(ran.isError).toBe(true)
  expect(toasts).toEqual([])
  expect(logged().length).toBe(1)
})

test('the log keeps the newest 200 rows', async ($, on) => {
  world(on, { ask: ASK() })
  for (let i = 0; i < 203; i++) await $.tool.call({ tool: 'Bash', command: ASK_CMD })
  expect(logged().length).toBe(200)
})
