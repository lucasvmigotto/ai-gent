import { expect, test } from 'claude-code/testing'

import type { Next } from '../types'
import { HINT, differs, keyOf, labelOf, toastText } from './nextstage-model'

const R3: Next = { rule: 3, stage: '/project:spec', reason: '001-foo needs plan', ref: '001' }
const NONE: Next = { rule: null, stage: '/project:status', reason: 'nothing is ready to start; the full report covers QA, docs and CI' }
const R1: Next = { rule: 1, stage: 'ask the user', reason: 'an open [NEEDS CLARIFICATION] blocks plan: which login?', ref: 'specs/001-x/spec.md:4' }

test('a recommendation differs by stage or reason, and a first one always does', () => {
  expect(differs(null, R3)).toBe(true)
  expect(differs(R3, { ...R3 })).toBe(false)
  expect(differs(R3, { ...R3, rule: 2, ref: 'x' })).toBe(false)
  expect(differs(R3, { ...R3, reason: '001-foo needs tasks' })).toBe(true)
  expect(differs(R3, NONE)).toBe(true)
  expect(keyOf(R3)).toBe('/project:spec 001-foo needs plan')
})

test('the label names a pipeline artifact or a feature file, and nothing else', () => {
  expect(labelOf('/repo/specs/README.md')).toBe('specs index')
  expect(labelOf('/repo/docs/product/brief.md')).toBe('brief')
  expect(labelOf('/repo/specs/001-foo/tasks.md')).toBe('001-foo tasks')
  expect(labelOf('/repo/specs/001-foo/contracts/x.md')).toBeNull()
  expect(labelOf('/repo/docs/product/notes.md')).toBeNull()
})

test('the wording quotes the snapshot and ends as a hint', () => {
  expect(toastText(R3, ['specs index'])).toBe(`specs index written — next: /project:spec (001-foo needs plan) · ${HINT}`)
  expect(toastText(NONE, ['brief'])).toBe(`brief written — next: /project:status (${NONE.reason}) · ${HINT}`)
  expect(toastText(NONE, [])).toBe(`next: /project:status (${NONE.reason}) · ${HINT}`)
  expect(toastText(R1, ['001-x spec'])).toBe(`ask the user (${R1.reason}) — specs/001-x/spec.md:4 · ${HINT}`)
  expect(toastText(R3, ['brief', 'architecture'])).toContain('brief, architecture written')
})
