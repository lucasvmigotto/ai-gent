import { expect, test } from 'claude-code/testing'

import { clampCap, listing, moreLine, moreLiner, parseBands, visible } from './band-priority'
import type { Band } from './band-priority'

const band = (id: string, priority: Band['priority'], since = 1, plugin = 'p'): Band => ({ id, plugin, priority, since })

test('hold and normal draw, info gives way', () => {
  expect(visible([band('i', 'info'), band('n', 'normal'), band('h', 'hold')], 2)).toEqual(['h', 'n'])
})

test('hold and failure draw before normal', () => {
  expect(visible([band('n', 'normal'), band('f', 'failure'), band('h', 'hold')], 2)).toEqual(['h', 'f'])
})

test('the same priority goes by activation time, then id', () => {
  expect(visible([band('late', 'normal', 9), band('early', 'normal', 1), band('mid', 'normal', 5)], 2)).toEqual(['early', 'mid'])
  expect(visible([band('b', 'normal', 1), band('a', 'normal', 1)], 1)).toEqual(['a'])
})

test('one band draws as it is, none draws nothing', () => {
  expect(visible([band('only', 'info')], 2)).toEqual(['only'])
  expect(visible([], 2)).toEqual([])
})

test('hold bands are never hidden, even above the cap', () => {
  const holds = [band('h1', 'hold', 3), band('h2', 'hold', 1), band('h3', 'hold', 2)]
  expect(visible([...holds, band('n', 'normal')], 2)).toEqual(['h2', 'h3', 'h1'])
  expect(visible([band('h', 'hold'), band('n', 'normal')], 1)).toEqual(['h'])
})

test('the cap is clamped to 1-5 and defaults to 2', () => {
  expect(clampCap(undefined)).toBe(2)
  expect(clampCap('3')).toBe(2)
  expect(clampCap(0)).toBe(1)
  expect(clampCap(9)).toBe(5)
  expect(clampCap(3)).toBe(3)
  const many = ['a', 'b', 'c', 'd', 'e', 'f'].map((id, i) => band(id, 'normal', i))
  expect(visible(many, 99)).toEqual(['a', 'b', 'c', 'd', 'e'])
})

test('the more line counts what is hidden, and one plugin owns it', () => {
  expect(moreLine(0)).toBeNull()
  expect(moreLine(1)).toBe('+1 more band (/bands)')
  expect(moreLine(2)).toBe('+2 more bands (/bands)')
  const all = [band('h', 'hold'), band('n', 'normal'), band('i', 'info', 1), band('j', 'info', 2)]
  expect(moreLiner(all, 2)).toBe('i')
  expect(moreLiner(all, 5)).toBeNull()
})

test('parseBands drops what is not a band', () => {
  expect(parseBands(undefined)).toEqual([])
  expect(parseBands('x')).toEqual([])
  const good = band('a', 'hold')
  expect(parseBands([good, { id: 1 }, null, { ...good, priority: 'urgent' }])).toEqual([good])
})

test('the listing names plugin, priority and whether it draws', () => {
  expect(listing([], 2)).toBe('No active bands.')
  const text = listing([band('hold', 'hold', 1, 'project'), band('hygiene', 'normal', 2, 'git'), band('ci', 'info', 3, 'repo')], 2)
  expect(text).toContain('hold (project, hold): shown')
  expect(text).toContain('hygiene (git, normal): shown')
  expect(text).toContain('ci (repo, info): hidden')
})
