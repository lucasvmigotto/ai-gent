import { expect, test } from 'claude-code/testing'

import { JSON_KEYS, MOVES_RELEASE, TARGET_KEYS, bandText, failureText, isReleaseBranch, keyOf, parsePlan } from './release-model'

const target = (kind: string, label: string) => ({ kind, label, from: '1.0.0', to: '1.1.0', bump: 'minor' })
const json = (over: Record<string, unknown> = {}) =>
  JSON.stringify({
    version: '2.23.0',
    bump: 'minor',
    since: '2.22.0',
    targets: [target('plugin', 'git'), target('plugin', 'repo'), target('plugin', 'agents'), target('manifest', 'release.json')],
    changelog: 'generated from commits',
    ...over,
  })

test('the keys are the ones both scripts print', () => {
  expect([...JSON_KEYS]).toEqual(['version', 'bump', 'since', 'targets', 'changelog'])
  expect([...TARGET_KEYS]).toEqual(['kind', 'label', 'from', 'to', 'bump'])
})

test('the plan is read from the JSON, counting only plugin targets', () => {
  expect(parsePlan(json())).toEqual({ version: '2.23.0', bump: 'minor', since: '2.22.0', plugins: 3 })
  expect(parsePlan(json({ since: null, bump: 'patch', targets: [target('plugin', 'git')] }))).toEqual({ version: '2.23.0', bump: 'patch', since: null, plugins: 1 })
  expect(parsePlan(json({ targets: [target('source', 'VERSION')] }))).toEqual({ version: '2.23.0', bump: 'minor', since: '2.22.0', plugins: 0 })
  expect(parsePlan(`${json()}\n`)).not.toBeNull()
})

test('anything but the expected JSON is no plan', () => {
  for (const bad of ['', 'release 2.23.0 (minor)', '[]', '{}', json({ version: 3 }), json({ bump: 'huge' }), json({ since: 4 }), json({ targets: 'x' }), json({ targets: [null] }), json({ targets: [{ kind: 'plugin' }] })]) {
    expect(parsePlan(bad), bad).toBeNull()
  }
})

test('the band says the version, the bump, the base tag and the plugin count', () => {
  const plan = { version: '2.23.0', bump: 'minor' as const, since: '2.22.0', plugins: 3 }
  expect(bandText(plan, 120, false)).toBe('next release 2.23.0 (minor, since 2.22.0) · 3 plugins bump — /release-preview for the plan')
  expect(bandText({ ...plan, plugins: 1 }, 120, false)).toBe('next release 2.23.0 (minor, since 2.22.0) · 1 plugin bump — /release-preview for the plan')
  expect(bandText({ ...plan, plugins: 0, since: null }, 120, false)).toBe('next release 2.23.0 (minor, first release) — /release-preview for the plan')
  expect(bandText({ ...plan, plugins: 0, version: '1.4.0', since: '1.3.2' }, 120, false)).toBe('next release 1.4.0 (minor, since 1.3.2) — /release-preview for the plan')
  expect(bandText(plan, 120, true)).toBe('next release 2.23.0 (minor, since 2.22.0) · 3 plugins bump (local branch is behind origin) — /release-preview for the plan')
  expect(bandText(plan, 59, false)).toBe('next 2.23.0 (minor)')
  expect(bandText(plan, 60, false)).toContain('next release')
})

test('a failure is one dim line with the first stderr line', () => {
  expect(failureText(1, 'release.py: error: no git\nmore')).toBe('release preview failed (exit 1): release.py: error: no git')
  expect(failureText(2, 'release.py: error: unrecognized arguments: --json')).toBe(
    'release preview failed (exit 2): release.py: error: unrecognized arguments: --json — re-run devsecops:pipeline to refresh scripts/release.py',
  )
  expect(failureText(null, '')).toBe('release preview failed: scripts/release.py could not be started')
  expect(failureText(0, '')).toBe('release preview failed: scripts/release.py printed no plan')
})

test('only a chain branch is previewed', () => {
  for (const yes of ['main', 'master', 'dev', 'develop']) expect(isReleaseBranch(yes), yes).toBe(true)
  for (const no of ['feat/x', 'HEAD', 'dev2', '']) expect(isReleaseBranch(no), no).toBe(false)
})

test('the key is HEAD, a newline and the merged tags', () => {
  expect(keyOf('abc', '1.0.0\n1.1.0')).toBe('abc\n1.0.0\n1.1.0')
  expect(keyOf('abc', '')).toBe('abc\n')
})

test('commands that can change HEAD or the tags are told apart from the rest', () => {
  for (const yes of ['git commit -m x', 'git tag 1.0.0', 'git -C ../x tag -d 1.0.0', 'cd x && git merge dev', 'git pull --ff-only', 'git switch dev']) {
    expect(MOVES_RELEASE.test(yes), yes).toBe(true)
  }
  for (const no of ['git status', 'echo git tag', 'git log --oneline', 'git push']) expect(MOVES_RELEASE.test(no), no).toBe(false)
})
