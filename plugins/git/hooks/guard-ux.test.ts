import { expect, test } from 'claude-code/testing'

import { GUARDS, MAX_ROWS, append, decorate, record, verb } from './guard-ux'
import type { Decision, GuardId, GuardRow } from './guard-ux'

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

// Credential shapes are put together here from fragments (the `vars` of
// shared/secret-cases.json), so no full token is committed.
const FRAGMENTS: Record<string, { parts: string[]; fill: string; times: number }> = {
  GHP: { parts: ['gh', 'p_'], fill: 'A', times: 36 },
  GHPAT: { parts: ['github', '_pat_'], fill: 'B', times: 30 },
  SK: { parts: ['sk', '-'], fill: 'C', times: 24 },
  AKIA: { parts: ['AK', 'IA'], fill: '7', times: 16 },
  XOX: { parts: ['xo', 'xb-'], fill: '9', times: 12 },
  JWT: { parts: ['eyJhbGci', 'OiJIUzI1NiIs'], fill: '', times: 0 },
  PKEY: { parts: ['-----BEGIN OPENSSH PRIV', 'ATE KEY-----\nb3BlbnNzaC1rZXk\n-----END OPENSSH PRIV', 'ATE KEY-----'], fill: '', times: 0 },
}
const TOKENS = Object.values(FRAGMENTS).map(v => v.parts.join('') + v.fill.repeat(v.times))

test('the verb is the first word, a path reduced to its last segment', () => {
  expect(verb('psql -c "select 1"', 'Bash')).toBe('psql')
  expect(verb('/usr/bin/psql -c x', 'Bash')).toBe('psql')
  expect(verb('  ls', 'Bash')).toBe('ls')
})

test('git, gh, pass, podman and docker also keep a second word that is a plain word', () => {
  expect(verb('git push origin main', 'Bash')).toBe('git push')
  expect(verb('gh pr merge 3', 'Bash')).toBe('gh pr')
  expect(verb('pass rm ai-gent/db/x', 'Bash')).toBe('pass rm')
  expect(verb('docker system prune -af', 'Bash')).toBe('docker system')
  expect(verb('podman volume rm x', 'Bash')).toBe('podman volume')
  expect(verb('git -C x push', 'Bash')).toBe('git')
  expect(verb('git Push', 'Bash')).toBe('git')
  expect(verb('psql push', 'Bash')).toBe('psql')
})

test('a tool with no command, or a command with no usable first word, has no verb', () => {
  expect(verb('ls', 'Read')).toBeNull()
  expect(verb(undefined, 'Bash')).toBeNull()
  expect(verb('', 'Bash')).toBeNull()
  expect(verb('FOO=bar make', 'Bash')).toBeNull()
  expect(verb('postgres://u:x@h/db', 'Bash')).toBeNull()
  expect(verb('x'.repeat(30), 'Bash')).toBeNull()
})

test('a credential shape never becomes a verb: as a first word, an argument or inside a URL', () => {
  for (const token of TOKENS) {
    for (const command of [token, `${token} push`, `git ${token}`, `git push ${token}`, `psql postgres://u:${token}@h/db`, `curl -H "Authorization: Bearer ${token}" https://h/${token}`]) {
      const found = verb(command, 'Bash') ?? ''
      expect(found.includes(token)).toBe(false)
      expect(TOKENS.some(t => found.includes(t.slice(0, 8)))).toBe(false)
    }
  }
})

const GIT_ASK = 'git:workflow guard: git push publishes; allow only if the user explicitly asked for this push'

test('a deny or an ask of the guard becomes one row: verb and entry, no text', () => {
  const ask = record('git', { ask: GIT_ASK }, { tool: 'Bash', command: 'git push origin main' }, 42)
  expect(ask).toEqual({ at: 42, guard: 'git', decision: 'ask', entry: 'git push publishes', tool: 'Bash', verb: 'git push', outcome: 'asked' })
  const deny = record('git', { deny: 'PreToolUse:Bash hook error: git:workflow guard: hooks must not be bypassed (--no-verify / -n); x' }, { tool: 'Bash', command: 'git commit -n' }, 7)
  expect(deny).toEqual({ at: 7, guard: 'git', decision: 'deny', entry: 'hooks must not be bypassed', tool: 'Bash', verb: 'git commit', outcome: null })
})

test('a reason with no entry is a generic row and keeps no reason text', () => {
  const row = record('db', { deny: 'db guard: some rule added later' }, { tool: 'Bash', command: 'psql' }, 1) as GuardRow
  expect(row.entry).toBe('generic')
  expect(JSON.stringify(row)).not.toContain('added later')
})

test('what the guard interpolated never reaches the row', () => {
  const row = record('git', { deny: 'git:workflow guard: commits directly on feature-xyz-branch are not allowed (commits directly on main)' }, { tool: 'Bash', command: 'git commit -m "wip secret-branch"' }, 1)
  expect(row?.entry).toBe('commits directly on')
  const text = JSON.stringify(row)
  expect(text).not.toContain('feature-xyz-branch')
  expect(text).not.toContain('wip')
  expect(text).not.toContain('main')
})

test('no row for another guard\'s reason, an allow, or no decision', () => {
  for (const id of IDS) {
    for (const other of IDS.filter(o => o !== id)) {
      const foreign = GUARDS[other].marker + ' ' + (GUARDS[other].entries[0]?.match ?? '')
      expect(record(id, { deny: foreign }, { tool: 'Bash', command: 'x' }, 1)).toBeNull()
      expect(record(id, { ask: foreign }, { tool: 'Bash', command: 'x' }, 1)).toBeNull()
    }
    const own = GUARDS[id].marker + ' x'
    expect(record(id, { allow: true, deny: own }, { tool: 'Bash' }, 1)).toBeNull()
    expect(record(id, {}, { tool: 'Bash' }, 1)).toBeNull()
  }
})

test('a missing command or an odd tool name leaves no verb and no stray text', () => {
  const row = record('db', { ask: 'db guard: psql bypasses dbrun (x)' }, { tool: 'Read' }, 1)
  expect(row?.verb).toBeNull()
  expect(row?.tool).toBe('Read')
  const odd = record('db', { ask: 'db guard: psql bypasses dbrun (x)' }, { tool: 'Bash psql -c "x"\nmore', command: 'psql' }, 1)
  expect(odd?.tool).toBe('unknown')
})

test('every table entry yields a row naming its own match', () => {
  for (const id of IDS) {
    for (const entry of GUARDS[id].entries) {
      expect(record(id, { ask: `${GUARDS[id].marker} ${entry.match}; x` }, { tool: 'Bash', command: 'ls' }, 1)?.entry).toBe(entry.match)
    }
  }
})

test('the ring keeps the newest rows, at most MAX_ROWS, oldest dropped first', () => {
  expect(MAX_ROWS).toBe(200)
  const row = (at: number): GuardRow => ({ at, guard: 'git', decision: 'ask', entry: 'generic', tool: 'Bash', verb: null, outcome: 'asked' })
  let rows: GuardRow[] = []
  for (let i = 0; i < 205; i++) rows = append(rows, row(i))
  expect(rows.length).toBe(200)
  expect(rows[0]?.at).toBe(5)
  expect(rows[199]?.at).toBe(204)
  const before: GuardRow[] = [row(1)]
  expect(append(before, row(2)).length).toBe(2)
  expect(before.length).toBe(1)
})
