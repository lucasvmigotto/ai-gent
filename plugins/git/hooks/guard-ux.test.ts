import { expect, test } from 'claude-code/testing'

import { GUARDS, decorate } from './guard-ux'
import type { Decision, GuardId } from './guard-ux'

// The wording of every guard's reasons is checked against guard.sh by
// scripts/consistency.py; here the table's behaviour: it explains, and it
// never changes a decision (the same tests run in every guard plugin's copy).

const IDS = Object.keys(GUARDS) as GuardId[]

test('a deny keeps the guard\'s exact text and gains a line of context and a notice', () => {
  const reason = 'PreToolUse:Bash hook error: git:workflow guard: hooks must not be bypassed (--no-verify / -n); fix what the hook reports and try again'
  const { result, notice } = decorate('git', { deny: reason })
  expect(result.deny).toBe(reason)
  expect(result.ask).toBeUndefined()
  expect(result.allow).toBeUndefined()
  expect(result.additionalContext).toEqual(['git:workflow guard: instead, fix what the hook reports, then commit again.'])
  expect(notice).toBe('git:workflow guard refused this. Hooks are never bypassed (--no-verify, -n). Instead: fix what the hook reports, then commit again. (git:workflow)')
})

test('an ask stays an ask: a fuller question over the guard\'s reason, verbatim', () => {
  const reason = 'git:workflow guard: git push publishes; allow only if the user explicitly asked for this push'
  const { result, notice } = decorate('git', { ask: reason })
  expect(result.ask).toBe(`This publishes commits to the remote. Allow only if you asked for this push.\n\n${reason}`)
  expect(result.deny).toBeUndefined()
  expect(result.allow).toBeUndefined()
  expect(notice).toBeNull()
})

test('a reason with no entry is shown as the guard wrote it, and an ask is left alone', () => {
  const deny = decorate('db', { deny: 'db guard: some rule added later' })
  expect(deny.result).toEqual({ deny: 'db guard: some rule added later' })
  expect(deny.notice).toBe('db guard refused this. some rule added later (db:connect)')
  const ask = decorate('db', { ask: 'db guard: some question added later' })
  expect(ask).toEqual({ result: { ask: 'db guard: some question added later' }, notice: null })
})

test('a guard decorates only its own reasons', () => {
  for (const id of IDS) {
    for (const other of IDS.filter(o => o !== id)) {
      const foreign = GUARDS[other].marker + ' ' + (GUARDS[other].entries[0]?.match ?? '')
      for (const decision of [{ deny: foreign }, { ask: foreign }] as Decision[]) {
        expect(decorate(id, decision)).toEqual({ result: decision, notice: null })
      }
    }
  }
})

test('no decision is ever created or changed into another', () => {
  for (const id of IDS) {
    const guard = GUARDS[id]
    for (const entry of guard.entries) {
      const reason = `${guard.marker} ${entry.match}; x`
      const denied = decorate(id, { deny: reason }).result
      expect(denied.deny).toBe(reason)
      expect(denied.ask ?? denied.allow).toBeUndefined()
      const asked = decorate(id, { ask: reason }).result
      expect(asked.ask).toContain(reason)
      expect(asked.deny ?? asked.allow).toBeUndefined()
    }
    // allow, or no decision at all, passes untouched
    expect(decorate(id, { allow: true })).toEqual({ result: { allow: true }, notice: null })
    expect(decorate(id, {})).toEqual({ result: {}, notice: null })
    expect(decorate(id, { deny: 'something else entirely' }).result).toEqual({ deny: 'something else entirely' })
  }
})

test('context already on a result is kept', () => {
  const reason = `${GUARDS.secrets.marker} the agent never writes a credential; ask`
  const { result } = decorate('secrets', { deny: reason, additionalContext: ['earlier'] })
  expect(result.additionalContext).toEqual(['earlier', 'secrets guard: instead, ask the user to run `pass insert <entry>` in their own terminal.'])
})
