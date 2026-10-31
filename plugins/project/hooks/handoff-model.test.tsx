import { expect, test } from 'claude-code/testing'

import { capToast, compactToast, contextToast, fmtTokens, isContextHigh, lowCaps, parseCaps, standing } from './handoff-model'
import type { Standing } from './handoff-model'

const NOW = 1_800_000_000_000
const FRESH = { isStale: false, mtimeMs: NOW - 3_600_000 }
const WITH = (extra: Partial<Standing>): Standing => ({
  checkpoint: { isStale: true, mtimeMs: NOW - 3_600_000 },
  commits: 3,
  isDirty: false,
  now: NOW,
  ...extra,
})

test('the context test is at or above the threshold, and a missing figure never fires', () => {
  expect(isContextHigh(82, 80)).toBe(true)
  expect(isContextHigh(80, 80)).toBe(true)
  expect(isContextHigh(79, 80)).toBe(false)
  expect(isContextHigh(65, 60)).toBe(true)
  expect(isContextHigh(undefined, 80)).toBe(false)
})

test('caps are read from budget --json, and anything else is not a budget', () => {
  const out = JSON.stringify({ verdict: 'proceed', caps: [{ name: 'session_billable', cap: 4_000_000, used: 3_700_000, left: 300_000 }, { name: 'bad' }], cards: [], batches: [] })
  expect(parseCaps(out)).toEqual([{ name: 'session_billable', cap: 4_000_000, used: 3_700_000, left: 300_000 }])
  expect(parseCaps(JSON.stringify({ verdict: 'proceed', caps: [] }))).toEqual([])
  expect(parseCaps('not json')).toBeNull()
  expect(parseCaps('{"verdict":"x"}')).toBeNull()
})

test('a cap is low when under the share left, the tightest first, and an empty cap is skipped', () => {
  const caps = [
    { name: 'week_billable', cap: 60_000_000, used: 54_600_000, left: 5_400_000 },
    { name: 'session_billable', cap: 4_000_000, used: 3_840_000, left: 160_000 },
    { name: 'roomy', cap: 10, used: 1, left: 9 },
    { name: 'zero', cap: 0, used: 0, left: 0 },
  ]
  expect(lowCaps(caps, 10).map(c => c.name)).toEqual(['session_billable', 'week_billable'])
  expect(lowCaps(caps, 5).map(c => c.name)).toEqual(['session_billable'])
  expect(lowCaps([{ name: 'a', cap: 100, used: 80, left: 20 }], 25).map(c => c.name)).toEqual(['a'])
  expect(lowCaps([{ name: 'a', cap: 100, used: 90, left: 10 }], 10)).toEqual([])
})

test('figures print short', () => {
  expect([fmtTokens(4_000_000), fmtTokens(280_000), fmtTokens(1_250_000), fmtTokens(900)]).toEqual(['4M', '280k', '1.3M', '900'])
})

test('the standing names commits, the tree, the age, or that nothing is needed', () => {
  expect(standing(WITH({}))).toEqual({ isCurrent: false, text: 'The checkpoint is 3 commits older than HEAD' })
  expect(standing(WITH({ commits: 1 })).text).toBe('The checkpoint is 1 commit older than HEAD')
  expect(standing(WITH({ isDirty: true })).text).toBe('The checkpoint is 3 commits older than HEAD, and the working tree has uncommitted changes')
  expect(standing(WITH({ commits: 0, checkpoint: FRESH, isDirty: true })).text).toBe('The working tree has uncommitted changes since the checkpoint')
  expect(standing(WITH({ commits: 0 })).text).toBe('The checkpoint is older than the last commit')
  expect(standing(WITH({ commits: 0, checkpoint: FRESH }))).toEqual({ isCurrent: true, text: 'The checkpoint is current' })
  expect(standing(WITH({ commits: null, isDirty: null, checkpoint: FRESH })).text).toBe('The checkpoint was written 1 h ago')
  expect(standing(WITH({ checkpoint: null })).text).toBe('There is no checkpoint')
})

test('the context toast follows the story 1 wording, and says nothing more is needed when current', () => {
  expect(contextToast(82, WITH({}))).toBe('Context 82% full. The checkpoint is 3 commits older than HEAD — run /project:checkpoint before the conversation is compacted.')
  expect(contextToast(82, WITH({ checkpoint: null }))).toBe('Context 82% full. There is no checkpoint — run /project:checkpoint before the conversation is compacted.')
  expect(contextToast(82, WITH({ commits: 0, checkpoint: FRESH }))).toBe('Context 82% full. The checkpoint is current; nothing more is needed.')
})

test('the compaction toast has no percent', () => {
  const text = compactToast(WITH({}))
  expect(text).toBe('The conversation is about to be compacted. The checkpoint is 3 commits older than HEAD — run /project:checkpoint first: a summary loses what it keeps.')
  expect(text).not.toContain('%')
  expect(compactToast(WITH({ commits: 0, checkpoint: FRESH }))).toBe('The conversation is about to be compacted. The checkpoint is current; nothing more is needed.')
})

test('the cap toast names the cap and what is left', () => {
  const cap = { name: 'session_billable', cap: 4_000_000, used: 3_720_000, left: 280_000 }
  expect(capToast(cap, WITH({}))).toBe('session_billable cap nearly spent (280k of 4M left). The checkpoint is 3 commits older than HEAD — run /project:checkpoint.')
  expect(capToast(cap, WITH({ commits: 0, checkpoint: FRESH }))).toBe('session_billable cap nearly spent (280k of 4M left). The checkpoint is current; nothing more is needed.')
})
