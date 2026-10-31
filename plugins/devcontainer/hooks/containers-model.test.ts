import { expect, test } from 'claude-code/testing'

import { NARROW, TRIGGER, bandText, commandText, findingLine, POINTER, priorityOf, readReport, summaryOf } from './containers-model'

const FULL = '{"checked":true,"project":"shop","findings":[{"kind":"disk","free_gb":14,"floor_gb":20,"store":"/s"},{"kind":"dangling","count":2},{"kind":"volumes","count":1},{"kind":"buildah","count":21}]}'

test('the JSON is read by its keys, in the script\'s order, and nothing else', () => {
  const report = readReport(FULL)
  expect(report).toEqual({
    checked: true,
    reason: null,
    findings: [
      { kind: 'disk', free_gb: 14, floor_gb: 20 },
      { kind: 'dangling', count: 2 },
      { kind: 'volumes', count: 1 },
      { kind: 'buildah', count: 21 },
    ],
  })
  expect(readReport('{"checked":true,"project":"shop","findings":[]}')).toEqual({ checked: true, reason: null, findings: [] })
  expect(readReport('{"checked":false,"reason":"no-engine"}')).toEqual({ checked: false, reason: 'no-engine', findings: [] })
  expect(readReport('{"checked":false}')).toEqual({ checked: false, reason: null, findings: [] })
})

test('output that is not the report reads as null', () => {
  for (const bad of ['', 'ai-gent containers (report only):\n2 dangling', '[]', '{"findings":[]}', '{"checked":true}', '{"checked":true,"findings":[1,{"count":2}]}']) {
    expect(readReport(bad), bad).toBeNull()
  }
})

test('an unknown kind is kept, with its count when it has one', () => {
  const report = readReport('{"checked":true,"findings":[{"kind":"networks","count":3},{"kind":"mystery"}]}')
  expect(report?.findings).toEqual([{ kind: 'networks', count: 3 }, { kind: 'mystery' }])
  expect(bandText(report?.findings ?? [], 120)).toBe('containers: networks 3 · mystery — /containers')
})

test('the band follows the script\'s order and wording', () => {
  const findings = readReport(FULL)?.findings ?? []
  expect(bandText([findings[1], findings[2]] as never, 120)).toBe('containers: 2 dangling images · 1 volume of a gone worktree — /containers')
  expect(bandText(findings, 200)).toBe('containers: ⚠ disk 14 GB free, floor 20 · 2 dangling images · 1 volume of a gone worktree · 21 buildah (any project) — /containers')
  expect(bandText([{ kind: 'dangling', count: 1 }, { kind: 'volumes', count: 3 }], 120)).toBe('containers: 1 dangling image · 3 volumes of a gone worktree — /containers')
  expect(bandText([], 120)).toBeNull()
})

test('a narrow terminal gets the short form', () => {
  const findings = readReport(FULL)?.findings ?? []
  expect(NARROW).toBe(60)
  expect(bandText(findings, 59)).toBe('containers ⚠ 4 — /containers')
  expect(bandText(findings, 60)).toContain('disk 14 GB free')
  expect(bandText([], 40)).toBeNull()
})

test('a disk below the floor is normal, leftovers alone are info', () => {
  expect(priorityOf([{ kind: 'disk', free_gb: 1, floor_gb: 20 }])).toBe('normal')
  expect(priorityOf([{ kind: 'buildah', count: 2 }])).toBe('info')
})

test('/containers lists each finding, then the script\'s text and the pointer', () => {
  const findings = readReport(FULL)?.findings ?? []
  expect(findingLine(findings[0] as never)).toBe('⚠ disk 14 GB free, floor 20')
  const text = commandText({ checked: true, reason: null, findings }, 'ai-gent containers (report only, nothing removed):\ndisk: 14 GB free')
  expect(text).toContain('21 buildah (any project)')
  expect(text).toContain('ai-gent containers (report only, nothing removed):')
  expect(text.endsWith(POINTER)).toBe(true)
  expect(POINTER).toContain('wt.py resources')
  expect(POINTER).toContain('§9a')
})

test('/containers says clean, not checked and unreadable apart', () => {
  expect(commandText({ checked: true, reason: null, findings: [] }, '')).toBe('No leftovers or disk problem reported for this project.')
  expect(commandText({ checked: false, reason: 'no-engine', findings: [] }, '')).toContain('Nothing was checked (no-engine)')
  expect(commandText({ checked: false, reason: 'not-a-checkout', findings: [] }, '')).toContain('not-a-checkout')
  expect(commandText({ checked: false, reason: 'disabled', findings: [] }, '')).toContain('AI_GENT_NO_LEFTOVERS_CHECK')
  expect(commandText(null, '', 'unreadable')).toBe('The report could not be read: leftovers.sh --json did not print JSON.')
  expect(commandText(null, '', 'timeout')).toBe('The report timed out: leftovers.sh did not answer in 10 seconds.')
  expect(commandText(null, '', 'missing')).toBe('The report is not available: hooks/leftovers.sh is missing from the plugin.')
})

test('nothing the mod prints teaches a prune', () => {
  const findings = readReport(FULL)?.findings ?? []
  const all = [
    bandText(findings, 200),
    bandText(findings, 40),
    commandText({ checked: true, reason: null, findings }, 'disk: 14 GB free'),
    commandText({ checked: true, reason: null, findings: [] }, ''),
    commandText({ checked: false, reason: 'disabled', findings: [] }, ''),
    commandText(null, '', 'timeout'),
    commandText(null, '', 'unreadable'),
    commandText(null, '', 'missing'),
    POINTER,
  ]
  for (const text of all) expect(text ?? '').not.toMatch(/prune/i)
})

test('only container work triggers a read', () => {
  for (const yes of [
    'podman build -t x .',
    'docker run --rm x',
    'buildah bud .',
    'podman rm abc',
    'docker rmi x',
    'podman volume rm v',
    'docker compose up',
    'sudo podman build .',
    'FOO=1 docker run x',
    'python3 plugins/agents/scripts/wt.py finish x',
    './wt.py add feat',
    'wt.py remove x',
  ]) {
    expect(TRIGGER.test(yes), yes).toBe(true)
  }
  for (const no of ['ls', 'git status', 'podman ps', 'docker images', 'echo podman build', 'wt.py list', 'cat docker run']) {
    expect(TRIGGER.test(no), no).toBe(false)
  }
})

test('the workspace summary: leftovers as one counted warning, never for a low disk or nothing', () => {
  expect(summaryOf([])).toBeUndefined()
  expect(summaryOf([{ kind: 'disk', free_gb: 14, floor_gb: 20 }, { kind: 'dangling', count: 2 }])).toBeUndefined()
  expect(summaryOf([{ kind: 'dangling', count: 2 }, { kind: 'volumes', count: 1 }])).toEqual({ text: '3 container leftovers', tone: 'warn' })
  expect(summaryOf([{ kind: 'mystery' }])).toEqual({ text: '1 container leftover', tone: 'warn' })
})
