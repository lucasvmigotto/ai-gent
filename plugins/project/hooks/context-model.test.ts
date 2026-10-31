import { expect, test } from 'claude-code/testing'

import { bar, levelOf, meterOf, parsePercent, thresholdOf } from './context-model'

const wide = { columns: 100, threshold: 80, ascii: false }

test('the threshold is the nudge\'s: invalid values fall back to 80', () => {
  expect(thresholdOf(undefined)).toBe(80)
  expect(thresholdOf(Number.NaN)).toBe(80)
  expect(thresholdOf(0)).toBe(80)
  expect(thresholdOf(101)).toBe(80)
  expect(thresholdOf('70')).toBe(80)
  expect(thresholdOf(70)).toBe(70)
})

test('green below three quarters of the threshold, amber from there, red from the threshold', () => {
  expect(levelOf(59, 80)).toBe('ok')
  expect(levelOf(60, 80)).toBe('approaching')
  expect(levelOf(79, 80)).toBe('approaching')
  expect(levelOf(80, 80)).toBe('critical')
  expect(levelOf(100, 80)).toBe('critical')
  expect(levelOf(45, 60)).toBe('approaching')
})

test('the percent is clamped and rounded; anything that is not a number is none', () => {
  expect(parsePercent(42.6)).toBe(43)
  expect(parsePercent(-5)).toBe(0)
  expect(parsePercent(250)).toBe(100)
  for (const bad of [undefined, null, Number.NaN, Number.POSITIVE_INFINITY, '50', {}]) expect(parsePercent(bad)).toBeNull()
})

test('a normal figure reads label, bar, used and remaining', () => {
  const meter = meterOf(43, wide)
  expect(meter?.level).toBe('ok')
  expect(meter?.text).toBe(`Context ${bar(43, 20, false)} 43% used · 57% remaining`)
})

test('approaching and critical add a mark and a word, never colour alone', () => {
  expect(meterOf(70, wide)?.text).toContain('⚠ approaching')
  expect(meterOf(85, wide)?.text).toContain('✖ critical')
  expect(meterOf(85, { ...wide, ascii: true })?.text).toMatch(/!! critical$/)
  expect(meterOf(85, { ...wide, ascii: true })?.text).not.toMatch(/[^\x20-\x7e]/)
})

test('0 and 100 are safe', () => {
  expect(meterOf(0, wide)?.text).toContain('0% used · 100% remaining')
  expect(meterOf(0, wide)?.text).toContain('░'.repeat(20))
  expect(meterOf(100, wide)?.text).toContain('█'.repeat(20))
  expect(meterOf(100, wide)?.text).toContain('0% remaining')
  expect(meterOf(140, wide)?.percent).toBe(100)
})

test('missing, NaN and invalid usage show nothing, never a made-up figure', () => {
  for (const bad of [undefined, null, Number.NaN, Number.POSITIVE_INFINITY, 'x']) expect(meterOf(bad, wide)).toBeNull()
})

test('the line never passes its width, from wide down to nothing', () => {
  for (const columns of [100, 70, 50, 40, 30, 24, 16, 12, 9, 6, 3, 0]) {
    for (const percent of [0, 30, 65, 85, 100]) {
      const meter = meterOf(percent, { columns, threshold: 80, ascii: false })
      if (meter !== null) expect([...meter.text].length).toBeLessThanOrEqual(columns)
    }
  }
  expect(meterOf(50, { ...wide, columns: 12 })?.text).toBe('ctx 50%')
  expect(meterOf(50, { ...wide, columns: 3 })).toBeNull()
})

test('the bar follows the width, and drops out when narrow', () => {
  expect(meterOf(50, { ...wide, columns: 45 })?.text).toMatch(/█{6,}/)
  expect(meterOf(50, { ...wide, columns: 20 })?.text).not.toMatch(/█/)
})
