import { expect, test } from 'claude-code/testing'

import { grantLine, timeLeft } from './grant-model'

const NOW = new Date('2026-11-08T12:30:00Z')
const file = (expires: string, rest = 'branches: develop dev\nactions: push pr merge issues\n') => `expires: ${expires}\n${rest}`

test('an active grant lists the actions, branches and expiry with the time left', () => {
  expect(grantLine(file('2026-11-08T18:00:00Z'), NOW)).toBe('autonomy: push pr merge issues on develop dev, until 2026-11-08 18:00 UTC (in 5h)')
})

test('no file, an unreadable one or a malformed one is none', () => {
  expect(grantLine(null, NOW)).toBe('autonomy: none')
  expect(grantLine('', NOW)).toBe('autonomy: none')
  expect(grantLine('actions: push\n', NOW)).toBe('autonomy: none')
  expect(grantLine(file('2026-11-08 18:00'), NOW)).toBe('autonomy: none')
  expect(grantLine(file('2026-13-08T18:00:00Z'), NOW)).toBe('autonomy: none')
  expect(grantLine(file('2026-11-08T18:00:00+00:00'), NOW)).toBe('autonomy: none')
})

test('an expired file says expired, and the exact instant is expired', () => {
  expect(grantLine(file('2026-11-08T12:00:00Z'), NOW)).toBe('autonomy: expired')
  expect(grantLine(file('2026-11-08T12:30:00Z'), NOW)).toBe('autonomy: expired')
})

test('a valid file without actions lifts nothing', () => {
  expect(grantLine('expires: 2026-11-08T18:00:00Z\n', NOW)).toBe('autonomy: none lifted')
  expect(grantLine(file('2026-11-08T18:00:00Z', 'actions:\n'), NOW)).toBe('autonomy: none lifted')
  expect(grantLine(file('2026-11-08T18:00:00Z', 'actions: deploy\n'), NOW)).toBe('autonomy: none lifted')
})

test('unknown actions are ignored and branches default to develop dev', () => {
  expect(grantLine(file('2026-11-08T18:00:00Z', 'actions: deploy push tag\n'), NOW)).toBe('autonomy: push on develop dev, until 2026-11-08 18:00 UTC (in 5h)')
  expect(grantLine(file('2026-11-08T18:00:00Z', 'branches: release\nactions: pr\n'), NOW)).toContain('pr on release,')
})

test('time left: under a minute, minutes, hours, days', () => {
  expect(timeLeft(30_000)).toBe('in under 1m')
  expect(timeLeft(59 * 60_000)).toBe('in 59m')
  expect(timeLeft(60 * 60_000)).toBe('in 1h')
  expect(timeLeft(23 * 3_600_000 + 59 * 60_000)).toBe('in 23h')
  expect(timeLeft(48 * 3_600_000)).toBe('in 2d')
  expect(grantLine(file('2026-11-08T12:30:30Z'), NOW)).toContain('(in under 1m)')
  expect(grantLine(file('2026-11-08T13:12:00Z'), NOW)).toContain('(in 42m)')
  expect(grantLine(file('2026-11-11T12:30:00Z'), NOW)).toContain('(in 3d)')
})

test('it prints nothing beyond the three fields', () => {
  const out = grantLine(file('2026-11-08T18:00:00Z', 'branches: develop\nactions: push\nnote: hush\ntoken: abc\n'), NOW)
  expect(out).not.toMatch(/hush|abc|token|note/)
})
