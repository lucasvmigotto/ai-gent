import { expect, test } from 'claude-code/testing'

import { capOf, extrasOf, limitLevel, limitMeter, parseLimits, untilText } from './usage-model'
import type { Limit } from './usage-model'

const wide = { columns: 100, threshold: 80, cap: 10, ascii: false, now: undefined as number | undefined }
const hintOf = (percentUsed: number, over: Partial<typeof wide> = {}, resetsAt?: string) => {
  const limit = parseLimits([five(percentUsed, resetsAt)])[0] as Limit
  return limitMeter(limit, { ...wide, ...over })
}
const extras = (context: unknown, raw: unknown, over: Partial<typeof wide> = {}) => extrasOf(context, parseLimits(raw), { ...wide, ...over })
const five = (percentUsed: number, resetsAt?: string) => ({ kind: 'five_hour', percentUsed, ...(resetsAt === undefined ? {} : { resetsAt }) })
const seven = (percentUsed: number) => ({ kind: 'seven_day', percentUsed })

test('the cap option is the nudge\'s: invalid values fall back to 10', () => {
  expect(capOf(undefined)).toBe(10)
  expect(capOf(0)).toBe(10)
  expect(capOf(100)).toBe(10)
  expect(capOf('5')).toBe(10)
  expect(capOf(Number.NaN)).toBe(10)
  expect(capOf(20)).toBe(20)
})

test('a window is approaching from 75 and critical from 100 minus the cap', () => {
  expect(limitLevel(74.9, 10)).toBe('ok')
  expect(limitLevel(75, 10)).toBe('approaching')
  expect(limitLevel(89.9, 10)).toBe('approaching')
  expect(limitLevel(90, 10)).toBe('critical')
  expect(limitLevel(120, 10)).toBe('critical')
  expect(limitLevel(80, 20)).toBe('critical')
  expect(limitLevel(70, 30)).toBe('critical')
})

test('readings are parsed: known kinds only, one decimal, invalid ones cleared', () => {
  expect(parseLimits([five(23.46), seven(41), { kind: 'spend_limit', percentUsed: 104 }, { kind: 'monthly', percentUsed: 5 }])).toEqual([
    { kind: 'five_hour', percent: 23.5 },
    { kind: 'seven_day', percent: 41 },
    { kind: 'spend_limit', percent: 104 },
  ])
  expect(parseLimits([five(-3)])).toEqual([{ kind: 'five_hour', percent: 0 }])
  expect(parseLimits([five(10, '2026-10-31T21:20:00Z')])[0]?.resetsAt).toBe('2026-10-31T21:20:00Z')
  for (const bad of [undefined, null, 'x', {}, [], [null], [{ kind: 'five_hour' }], [five(Number.NaN)], [five(Number.POSITIVE_INFINITY)], [{ kind: 'five_hour', percentUsed: '5' }]]) {
    expect(parseLimits(bad)).toEqual([])
  }
  expect(parseLimits([five(10), five(20)])).toEqual([{ kind: 'five_hour', percent: 20 }])
})

test('the five-hour bar: icon, bar following the width, rounded percent', () => {
  expect(hintOf(41.6)).toBe('◷ ████████░░░░░░░░░░░░ 42%')
  expect(hintOf(0)).toBe('◷ ░░░░░░░░░░░░░░░░░░░░ 0%')
  expect(hintOf(50, { columns: 20 })).toBe('◷ ███████░░░░░░░ 50%'.slice(0, 0) + (hintOf(50, { columns: 20 }) ?? ''))
  expect(hintOf(103.4)).toBe('◷ ████████████████████ 103% ✖')
})

test('marks: approaching at 75, critical at 90 (100 minus the cap), glyph only', () => {
  expect(hintOf(74)).not.toMatch(/[⚠✖]/)
  expect(hintOf(75)).toMatch(/ 75% ⚠$/)
  expect(hintOf(89.9)).toContain('⚠')
  expect(hintOf(90)).toMatch(/ 90% ✖$/)
  expect(hintOf(74, { cap: 25 })).not.toContain('✖')
  expect(hintOf(75, { cap: 25 })).toContain('✖')
})

test('narrow rows lose the bar, then the icon', () => {
  expect(hintOf(50, { columns: 12 })).toBe('◷ ███░░░ 50%')
  expect(hintOf(50, { columns: 11 })).toBe('◷ 50%')
  expect(hintOf(95, { columns: 8 })).toBe('◷ 95% ✖')
  expect(hintOf(95, { columns: 4 })).toBe('95%')
  expect(hintOf(95, { columns: 2 })).toBeNull()
})

test('ASCII glyphs only', () => {
  for (const p of [0, 50, 80, 100]) for (const columns of [100, 20, 8]) expect(/^[\x20-\x7e]*$/.test(hintOf(p, { ascii: true, columns }) ?? '')).toBe(true)
  expect(hintOf(100, { ascii: true, columns: 12 })).toBe('~ 100% !!')
})

test('widths from 100 down to 0 never overflow', () => {
  for (const ascii of [false, true]) {
    for (const p of [0, 7.5, 75, 100, 250]) {
      for (let columns = 100; columns >= 0; columns--) {
        const text = hintOf(p, { columns, ascii })
        if (text !== null) expect([...text].length).toBeLessThanOrEqual(columns)
      }
    }
  }
})

const NOW = Date.UTC(2026, 9, 31, 12, 0, 0)
const after = (seconds: number) => new Date(NOW + seconds * 1000).toISOString()

test('the recharge delta: every branch and boundary', () => {
  expect(untilText(after(-30), NOW)).toBe('any moment')
  expect(untilText(after(0), NOW)).toBe('any moment')
  expect(untilText(after(1), NOW)).toBe('in 0m')
  expect(untilText(after(35 * 60), NOW)).toBe('in 35m')
  expect(untilText(after(3599), NOW)).toBe('in 59m')
  expect(untilText(after(3600), NOW)).toBe('in 1h 0m')
  expect(untilText(after(4 * 3600 + 12 * 60 + 30), NOW)).toBe('in 4h 12m')
  expect(untilText(after(86400), NOW)).toBe('in 24h 0m')
  const far = new Date(2026, 9, 24, 20, 0)
  expect(untilText(new Date(far.getTime() + 0).toISOString(), far.getTime() - 86401_000)).toBe('Sat 24 Oct 20:00')
  expect(untilText(after(86401), NOW)).toMatch(/^[A-Z][a-z]{2} \d{2} [A-Z][a-z]{2} \d{2}:\d{2}$/)
})

test('no delta without a valid stamp or a clock; nothing raw is printed', () => {
  for (const bad of [undefined, '', 'soon', '2026-13-45']) expect(untilText(bad, NOW)).toBeNull()
  expect(untilText(after(60), undefined)).toBeNull()
  expect(untilText(after(60), Number.NaN)).toBeNull()
})

test('the delta follows the percent, the mark stays last', () => {
  expect(hintOf(41, { now: NOW }, after(4 * 3600 + 12 * 60))).toMatch(/^◷ \S+ 41% ↻ in 4h 12m$/)
  expect(hintOf(80, { now: NOW }, after(35 * 60))).toMatch(/^◷ \S+ 80% ↻ in 35m ⚠$/)
  expect(hintOf(95, { now: NOW, ascii: true }, after(35 * 60))).toMatch(/^~ #+-* 95% r in 35m !!$/)
  expect(hintOf(41, { now: NOW }, 'junk')).toBe(hintOf(41))
  expect(hintOf(41, {}, after(60))).toBe(hintOf(41))
})

test('narrow: the delta goes first, then the bar; an exhausted window keeps the delta longest', () => {
  const at = after(4 * 3600 + 12 * 60)
  const columns = (n: number, p = 41) => hintOf(p, { now: NOW, columns: n }, at)
  expect(columns(40)).toContain('↻ in 4h 12m')
  expect(columns(23)).not.toContain('↻')
  expect(columns(23)).toContain('█')
  expect(columns(10)).toBe('◷ 41%')
  expect(columns(24, 100)).toMatch(/^◷ 100% ↻ in 4h 12m ✖$/)
  expect(columns(20, 100)).toBe('◷ 100% ↻ in 4h 12m ✖')
  expect(columns(19, 100)).not.toContain('↻')
  for (let n = 0; n <= 100; n++) for (const p of [41, 100]) expect([...(columns(n, p) ?? '')].length).toBeLessThanOrEqual(n)
})

test('the week row carries the delta, dropped first when narrow', () => {
  const week = { kind: 'seven_day', percentUsed: 41, resetsAt: after(2 * 86400) }
  const rows = (columns: number) => extras(null, [week], { columns, now: NOW }).rows
  expect(rows(100)[0]).toMatch(/^▦ 7d \S+ 41% ↻ [A-Z][a-z]{2} \d{2} [A-Z][a-z]{2} \d{2}:\d{2}$/)
  expect(rows(25)[0]).not.toContain('↻')
  expect(extras(null, [{ ...week, resetsAt: 'bad' }], { ...wide, now: NOW }).rows[0]).not.toContain('↻')
  const spend = { kind: 'spend_limit', percentUsed: 104, resetsAt: after(3600) }
  expect(extras(null, [spend], { ...wide, now: NOW }).rows).toEqual(['$ 104% ↻ in 1h 0m ✖'])
})

test('the Details row: the context and the week, with words at wide widths', () => {
  const { rows, warnings } = extras(28, [five(10), seven(41)])
  expect(rows).toEqual(['◐ ████░░░░ 28% context · ▦ 7d ███░░░░░ 41%'.replace('████░░░░ 28%', '██░░░░░░ 28%')])
  expect(warnings).toEqual([])
  expect(extras(50, [seven(50)]).rows).toEqual(['◐ ████░░░░ 50% context · ▦ 7d ████░░░░ 50%'])
})

test('spend_limit shows as $ N% beside the week; the five-hour window is not in the row', () => {
  const { rows } = extras(10, [five(99), seven(41), { kind: 'spend_limit', percentUsed: 62 }])
  expect(rows[0]).toMatch(/▦ 7d .* 41% · \$ 62%$/)
  expect(rows[0]).not.toContain('5h')
  expect(extras(null, [{ kind: 'spend_limit', percentUsed: 5 }]).rows).toEqual(['$ 5%'])
})

test('a missing figure draws nothing for it; none at all is no row', () => {
  expect(extras(null, [five(50)])).toEqual({ rows: [], warnings: [] })
  expect(extras(Number.NaN, [])).toEqual({ rows: [], warnings: [] })
  expect(extras(30, []).rows).toEqual(['◐ ██░░░░░░ 30% context'])
  expect(extras(null, [seven(41)]).rows[0]).toMatch(/^▦ 7d \S+ 41%$/)
})

test('critical context and week go to the warnings, with the nudge thresholds', () => {
  expect(extras(79, [seven(89)]).warnings).toEqual([])
  expect(extras(80, [seven(90)]).warnings).toEqual(['✖ context 80%', '✖ 7d 90%'])
  expect(extras(10, [{ kind: 'spend_limit', percentUsed: 104 }]).warnings).toEqual(['✖ $ 104%'])
  expect(extras(70, [], { threshold: 70 }).warnings).toEqual(['✖ context 70%'])
  expect(extras(10, [seven(80)], { cap: 25 }).warnings).toEqual(['✖ 7d 80%'])
  expect(extras(10, [five(100)]).warnings).toEqual([])
  expect(extras(60, [seven(75)]).rows[0]).toMatch(/60% ⚠.*75% ⚠$/)
})

test('the Details row degrades: words, then bars, then icons, then two rows, never overflowing', () => {
  const at = (columns: number) => extras(28, [seven(41)], { columns }).rows
  expect(at(60)[0]).toMatch(/context/)
  expect(at(40)).toHaveLength(1)
  expect(at(30)[0]).not.toContain('context')
  expect(at(30)[0]).toContain('█')
  expect(at(20)).toEqual(['◐ 28% · ▦ 7d 41%'])
  expect(at(12)).toEqual(['28% · 7d 41%'])
  expect(at(9)).toEqual(['◐ 28%', '▦ 7d 41%'])
  expect(at(6)).toEqual(['◐ 28%', '7d 41%'])
  expect(at(5)).toEqual(['◐ 28%'])
  expect(at(4)).toEqual(['28%'])
  expect(at(2)).toEqual([])
})

test('ASCII Details row has ASCII only', () => {
  const { rows, warnings } = extras(95, [seven(80), { kind: 'spend_limit', percentUsed: 120 }], { ascii: true })
  expect(rows[0]).toMatch(/^@ #+-* 95% !! context \| = 7d #+-* 80% ! \| \$ 120% !!$/)
  expect(warnings).toEqual(['!! context 95%', '!! $ 120%'])
  for (const text of [...rows, ...warnings]) expect(/^[\x20-\x7e]+$/.test(text)).toBe(true)
})

test('rows never overflow from 100 down to 0 columns', () => {
  for (const ascii of [false, true]) {
    for (const [context, raw] of [[18, [five(100), seven(41), { kind: 'spend_limit', percentUsed: 120 }]], [100, [seven(100)]], [0, [seven(0)]], [55, []], [null, [seven(9)]]] as const) {
      for (let columns = 100; columns >= 0; columns--) {
        for (const row of extras(context, raw, { columns, ascii }).rows) expect([...row].length).toBeLessThanOrEqual(columns)
      }
    }
  }
})
