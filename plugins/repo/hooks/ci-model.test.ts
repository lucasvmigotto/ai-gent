import { expect, test } from 'claude-code/testing'

import { bandText, delayMs, gate, isDone, isFailed, isPush, markOf, newFailures, parseDetect, parseRuns, priorityOf, runListArgv, toastText } from './ci-model'
import type { CiRun } from '../types'

const run = (id: number, workflow: string, status: string, conclusion = ''): CiRun => ({ id, workflow, name: workflow, status, conclusion })

// A real `gh run list --json databaseId,name,status,conclusion,workflowName,headSha` answer, recorded (shortened).
const GH = JSON.stringify([
  { conclusion: '', databaseId: 111, headSha: '3a9f1c2e0d', name: 'Docs CI', status: 'in_progress', workflowName: 'Docs CI' },
  { conclusion: 'success', databaseId: 112, headSha: '3a9f1c2e0d', name: 'check', status: 'completed', workflowName: 'check' },
])

// ------------------------------------------------------------ the push

test('a push that can change the remote is told from the rest', () => {
  for (const yes of ['git push', 'git push origin dev', 'git -C ../x push -u origin feat/a', 'cd x && git push', 'FOO=1 git push --force-with-lease', 'git add . ; git push']) {
    expect(isPush(yes), yes).toBe(true)
  }
  for (const no of ['git push --dry-run', 'git push -n origin dev', 'git status', 'echo git push', 'git pull', 'git log --oneline', 'gitpush']) {
    expect(isPush(no), no).toBe(false)
  }
})

// ------------------------------------------------------------ run list

test('the array is taken from the first line that starts with [ (stderr is merged into stdout)', () => {
  const merged = `reporun: gh run list --commit 3a9f1c2 · token from the CLI's own login\n! A new release of gh is available: 2.60 -> 2.62\n${GH}\n`
  expect(parseRuns(merged)?.map(r => r.id)).toEqual([111, 112])
  expect(parseRuns(GH)?.[0]).toEqual({ id: 111, workflow: 'Docs CI', name: 'Docs CI', status: 'in_progress', conclusion: '' })
  // pretty-printed, over many lines
  expect(parseRuns(`warning\n${JSON.stringify(JSON.parse(GH), null, 2)}\n`)?.length).toBe(2)
  // a bracket in a warning is skipped when it does not parse
  expect(parseRuns(`[warn] something\n${GH}`)?.length).toBe(2)
})

test('an empty array is no runs; anything unparsable is a failed read', () => {
  expect(parseRuns('reporun: x\n[]\n')).toEqual([])
  for (const bad of ['', 'no json here', 'gh: HTTP 401: Bad credentials', '[1, 2', '{"a":1}', '[{"databaseId":"x"}]', '[null]']) {
    expect(parseRuns(bad), bad).toBeNull()
  }
})

test('the call lists the commit\'s runs by sha, read only, with exactly the pinned fields', () => {
  expect(runListArgv('/plugin', '3a9f1c2e0d')).toEqual([
    'python3', '/plugin/scripts/reporun.py', 'cli', 'github', '--', 'run', 'list', '--commit', '3a9f1c2e0d', '--json', 'databaseId,name,status,conclusion,workflowName,headSha',
  ])
  expect(runListArgv('/plugin', 'x').join(' ')).not.toContain('--write')
})

// ------------------------------------------------------------ states

test('marks: success, failure-like, running and neutral', () => {
  expect(markOf(run(1, 'a', 'completed', 'success'))).toBe('✓')
  for (const c of ['failure', 'timed_out', 'startup_failure']) expect(markOf(run(1, 'a', 'completed', c)), c).toBe('✗')
  for (const s of ['queued', 'in_progress', 'waiting', 'pending', 'requested']) expect(markOf(run(1, 'a', s)), s).toBe('⏳')
  for (const c of ['cancelled', 'skipped', 'neutral', 'stale', 'action_required']) expect(markOf(run(1, 'a', 'completed', c)), c).toBe('–')
})

test('failed runs, done runs and the band\'s priority', () => {
  const ok = run(1, 'a', 'completed', 'success')
  const bad = run(2, 'b', 'completed', 'failure')
  const going = run(3, 'c', 'queued')
  expect([isFailed(ok), isFailed(bad), isFailed(going), isFailed(run(4, 'd', 'completed', 'cancelled'))]).toEqual([false, true, false, false])
  expect([isDone([ok, bad]), isDone([ok, going]), isDone([])]).toEqual([true, false, true])
  expect(priorityOf([ok, going])).toBe('normal')
  expect(priorityOf([ok, bad, going])).toBe('failure')
  expect(priorityOf([run(4, 'd', 'completed', 'cancelled')])).toBe('normal')
})

test('a failure toasts once per run id, and neutral ends never toast', () => {
  const runs = [run(1, 'a', 'completed', 'failure'), run(2, 'b', 'completed', 'cancelled'), run(3, 'c', 'completed', 'timed_out'), run(4, 'd', 'in_progress')]
  expect(newFailures(runs, []).map(r => r.id)).toEqual([1, 3])
  expect(newFailures(runs, [1]).map(r => r.id)).toEqual([3])
  expect(newFailures(runs, [1, 3])).toEqual([])
})

// ------------------------------------------------------------ wording

test('the band names the commit, the branch and each run with its mark', () => {
  const ci = { sha: '3a9f1c2e0d', branch: 'dev', startedAt: 0, runs: parseRuns(GH) ?? [] }
  expect(bandText(ci, 120)).toBe('CI for 3a9f1c2 on dev: Docs CI ⏳ · check ✓')
  const done = { ...ci, runs: [run(1, 'Docs CI', 'completed', 'success'), run(2, 'check', 'completed', 'failure')] }
  expect(bandText(done, 120)).toBe('CI for 3a9f1c2 on dev: Docs CI ✓ · check ✗ — more runs may follow')
  // narrow: the note goes first
  expect(bandText(done, 40)).toBe('CI 3a9f1c2: Docs CI ✓ · check ✗')
})

test('no runs says why it may be so, never that CI is broken', () => {
  const text = bandText({ sha: '3a9f1c2e0d', branch: 'dev', startedAt: 0, runs: [] }, 120)
  expect(text).toBe('no runs started for 3a9f1c2 (paths, branches or a bot commit?)')
  expect(text).not.toMatch(/broken/i)
})

test('the toast names the workflow and the skill, and nothing from the logs', () => {
  expect(toastText(run(1, 'check', 'completed', 'failure'), '3a9f1c2e0d')).toBe('check failed on 3a9f1c2 — ask /devsecops:watch why')
  expect(toastText({ ...run(1, '', 'completed', 'timed_out'), name: 'deploy' }, '3a9f1c2e0d')).toBe('deploy failed on 3a9f1c2 — ask /devsecops:watch why')
})

// ------------------------------------------------------------ back-off

test('the back-off is the poll interval, doubled twice, then held', () => {
  expect([0, 1, 2, 3, 9].map(p => delayMs(p, 30))).toEqual([30_000, 60_000, 120_000, 120_000, 120_000])
  expect(delayMs(0, 10)).toBe(10_000)
  expect(delayMs(0, Number.NaN)).toBe(30_000)
})

// ------------------------------------------------------------ detect

const detect = (platform: string, inPass: boolean, installed: boolean, signedIn: boolean) =>
  JSON.stringify({ repository: { platform }, platforms: { [platform]: { token_entry: 'ai-gent/github/token', token_in_pass: inPass, cli: { name: 'gh', installed, signed_in: signedIn } } } })

test('detect is reduced to the platform, the pass flag and the sign-in', () => {
  expect(parseDetect(detect('github', false, true, true))).toEqual({ platform: 'github', tokenInPass: false, ghSignedIn: true })
  expect(parseDetect(detect('github', true, true, false))).toEqual({ platform: 'github', tokenInPass: true, ghSignedIn: false })
  expect(parseDetect(detect('github', false, false, true))?.ghSignedIn).toBe(false)
  expect(parseDetect(detect('gitlab', false, true, true))?.platform).toBe('gitlab')
  expect(parseDetect(JSON.stringify({ repository: {}, platforms: {} }))).toEqual({ platform: '', tokenInPass: false, ghSignedIn: false })
  for (const bad of ['', 'nope', '[]', '{"platforms":1}']) expect(parseDetect(bad), bad).toBeNull()
})

test('the gate reads only on GitHub, signed in, and never decrypts pass unless allowed', () => {
  const d = (platform: string, tokenInPass: boolean, ghSignedIn: boolean) => ({ platform, tokenInPass, ghSignedIn })
  expect(gate(d('github', false, true), false)).toBe('read')
  expect(gate(d('github', true, true), false)).toBe('pass')
  expect(gate(d('github', true, true), true)).toBe('read')
  expect(gate(d('github', false, false), false)).toBe('signin')
  expect(gate(d('github', true, false), true)).toBe('signin')
  for (const p of ['gitlab', 'azure-devops', 'bitbucket', 'forgejo', '']) expect(gate(d(p, false, true), true), p).toBe('unsupported')
  expect(gate(null, false)).toBe('unsupported')
})
