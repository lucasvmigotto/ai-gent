import { expect, mock, test } from 'claude-code/testing'

import { hookLines, leftoversBlock, updateLine } from './hook-lines'
import { MOVES_REFS, branchesWithRemote, parseCounts, releasesIn, sentence } from './hygiene-model'
import type { BranchState } from '../types'

type Handlers = Parameters<Parameters<typeof test>[1]>[1]

// ----------------------------------------------------------------- the model

const state = (name: BranchState['name'], ahead: number, behind: number, releases: string[] = []): BranchState => ({ name, ahead, behind, releases })

test('git\'s counts, the release job\'s subjects and the branches with a remote are read', () => {
  expect(parseCounts('0\t1\n')).toEqual({ ahead: 0, behind: 1 })
  expect(parseCounts('2 3')).toEqual({ ahead: 2, behind: 3 })
  expect(parseCounts('nope')).toBeNull()

  expect(releasesIn('main', ['chore(release): 2.21.0', 'feat: x', 'chore(release): merge 2.21.0 back into dev'])).toEqual(['2.21.0'])
  expect(releasesIn('dev', ['chore(release): merge 2.21.0 back into dev', 'chore(release): 2.21.0', 'Merge pull request #1'])).toEqual(['2.21.0'])
  expect(releasesIn('develop', ['chore(release): merge 1.0.0 back into develop'])).toEqual(['1.0.0'])
  expect(releasesIn('master', ['chore(release): 1.0.0'])).toEqual(['1.0.0'])

  const refs = 'refs/heads/main\nrefs/heads/dev\nrefs/heads/feat/x\nrefs/remotes/origin/main\nrefs/remotes/origin/develop\n'
  expect(branchesWithRemote(refs)).toEqual(['main'])
})

test('the band says what git:workflow requires, and only that', () => {
  expect(sentence([state('main', 0, 0), state('dev', 0, 0)], 120, null)).toBeNull()
  expect(sentence([state('main', 0, 1, ['2.20.0'])], 120, null)).toBe(
    'main is 1 behind origin/main (release 2.20.0) — git pull --ff-only on main before the next commit',
  )
  expect(sentence([state('main', 0, 1, ['2.20.0']), state('dev', 0, 2, ['2.20.0'])], 120, null)).toBe(
    'main is 1 behind origin/main (release 2.20.0); dev is 2 behind origin/dev (release 2.20.0) — git pull --ff-only on main and dev before the next commit',
  )
  // diverged: no pull is suggested
  const diverged = sentence([state('dev', 2, 3)], 120, null) ?? ''
  expect(diverged).toBe("dev has diverged from origin/dev (2 ahead, 3 behind) — dev: see git:workflow, don't pull")
  expect(diverged).not.toContain('pull --ff-only')
  // ahead only is not the band's business
  expect(sentence([state('dev', 2, 0)], 120, null)).toBeNull()
  // the age of the last fetch, never a fetch
  expect(sentence([state('main', 0, 1)], 120, 2 * 86_400_000)).toContain('(as of the last fetch, 2 days ago)')
  expect(sentence([state('main', 0, 1)], 120, 3_600_000)).not.toContain('last fetch')
  // narrow
  expect(sentence([state('main', 0, 1), state('dev', 0, 2)], 40, null)).toBe('main ↓1 · dev ↓2 — pull --ff-only')
})

test('commands that move refs are told apart from the rest', () => {
  for (const yes of ['git pull --ff-only', 'git fetch origin', 'cd x && git merge --no-ff y', 'git -C ../c4t commit -m x', 'git switch dev', 'FOO=1 git reset --hard']) {
    expect(MOVES_REFS.test(yes), yes).toBe(true)
  }
  for (const no of ['git status', 'git log --oneline', 'echo git pull', 'ls', 'git diff']) {
    expect(MOVES_REFS.test(no), no).toBe(false)
  }
})

const UPDATE_ROW = 'SessionStart:startup hook success: [ai-gent] update available: 2.20.0 → 2.21.0 — run scripts/ai-gent-update.sh\n</system-reminder>'
const LEFTOVERS_ROW = 'ai-gent containers (report only, nothing removed):\ndisk: 12 GB free\n2 dangling images'

test('the session-start hooks\' lines are read from the stored row, not recomputed', () => {
  expect(updateLine(hookLines(UPDATE_ROW))).toBe('[ai-gent] update available: 2.20.0 → 2.21.0 — run scripts/ai-gent-update.sh')
  expect(updateLine(hookLines('SessionStart:startup hook success: something else'))).toBeNull()
  expect(leftoversBlock(hookLines(`hook success: ${LEFTOVERS_ROW}`))).toBe('ai-gent containers (report only, nothing removed): disk: 12 GB free · 2 dangling images')
  expect(leftoversBlock(hookLines('hook success: nothing'))).toBeNull()
})

// ------------------------------------------------------------------ the band

const ROOT = '/repo'
const BAND = {
  plugin: 'git',
  component: 'AbovePrompt',
  props: { hasSurvey: false, isWorking: false, maxRows: 10, bodyColumns: 120, scroll: { offset: 0, bodyRows: 10 }, view: {} },
} as const
const START = { cwd: ROOT, surface: 'terminal', isInteractive: true } as const

type World = {
  runs: string[][]
  counts: Record<string, string>
  subjects: Record<string, string>
  toasts: string[]
  registered: string[]
  release: { exists: boolean; exit: number; stdout: string; stderr: string }
  fetchedAt: number | undefined
  clock: ReturnType<typeof mock.clock>
}

// Nothing sits beneath the plugin: these hooks stand for the engine and a
// repository at ROOT whose answers a test edits between steps.
function world(on: Handlers): World {
  const w: World = {
    runs: [],
    counts: { main: '0\t1', dev: '0\t0' },
    subjects: { main: 'chore(release): 2.20.0' },
    toasts: [],
    registered: [],
    release: { exists: true, exit: 0, stdout: 'release 2.21.0 (minor)', stderr: '' },
    fetchedAt: undefined,
    clock: mock.clock(on, { now: 10 * 86_400_000 }),
  }
  const ok = (stdout: string) => ({ value: { exitCode: 0, stdout: `${stdout}\n`, stderr: '' } })
  on('session.start', (_, e) => ({ cwd: e.cwd }))
  on('command.register', (_, e) => {
    w.registered.push(e.name)
    return { value: { command: e.name } }
  })
  on('ui.toast', (_, e) => {
    w.toasts.push(e.text)
    return { value: undefined }
  })
  on('fs.stat', (_, e) =>
    e.path.endsWith('FETCH_HEAD') && w.fetchedAt !== undefined ? { value: { kind: 'file', size: 1, mtimeMs: w.fetchedAt, isLink: false } } : { deny: 'ENOENT' },
  )
  on('fs.exists', () => ({ value: w.release.exists }))
  on('process.run', (_, e) => {
    const argv = [...e.argv]
    w.runs.push(argv)
    const line = argv.join(' ')
    if (line.includes('--git-common-dir')) return ok(`${ROOT}/.git`)
    if (line.includes('--show-toplevel')) return ok(ROOT)
    if (argv[1] === 'for-each-ref') return ok('refs/heads/main\nrefs/heads/dev\nrefs/remotes/origin/main\nrefs/remotes/origin/dev')
    const range = /(main|dev)\.\.\.origin/.exec(line)?.[1]
    if (argv[1] === 'rev-list' && range) return ok(w.counts[range] ?? '0\t0')
    const log = /(main|dev)\.\.origin/.exec(line)?.[1]
    if (argv[1] === 'log' && log) return ok(w.subjects[log] ?? '')
    if (argv[0] === 'python3') return { value: { exitCode: w.release.exit, stdout: w.release.stdout, stderr: w.release.stderr } }
    return { value: { exitCode: 1, stdout: '', stderr: 'unexpected' } }
  })
  on('tool.call', () => ({ result: {} }))
  on('ui.render', ($, e) => {
    const { Text } = $.ui.resolve(e)
    return <Text dimColor>another band</Text>
  })
  return w
}

test('the band appears after a release and says it, above whatever is beneath, on every surface', async ($, on) => {
  const w = world(on)
  await $.session.start(START)
  for (const surface of ['terminal', 'desktop'] as const) {
    const ui = await $.ui.mount({ ...BAND, surface })
    expect(await ui.find({ type: 'Text', text: /main is 1 behind origin\/main \(release 2\.20\.0\) — git pull --ff-only on main/ })).toBeDefined()
    expect(await ui.find({ type: 'Text', text: /another band/ })).toBeDefined()
    await ui.unmount()
  }
  expect(w.runs.length).toBeGreaterThan(0)
})

test('the band yields to a survey and to a running turn, and draws nothing when level', async ($, on) => {
  const w = world(on)
  await $.session.start(START)
  for (const props of [{ ...BAND.props, hasSurvey: true }, { ...BAND.props, isWorking: true }]) {
    const ui = await $.ui.mount({ ...BAND, props, surface: 'terminal' })
    expect(await ui.find({ type: 'Text', text: /behind origin/ })).toBeUndefined()
    await ui.unmount()
  }
  w.counts.main = '0\t0'
  await $.tool.call({ tool: 'Bash', command: 'git pull --ff-only' } as never)
  const ui = await $.ui.mount({ ...BAND, surface: 'terminal' })
  expect(await ui.find({ type: 'Text', text: /behind origin/ })).toBeUndefined()
  expect(await ui.find({ type: 'Text', text: /another band/ })).toBeDefined()
  await ui.unmount()
})

test('a command that moved refs makes the band read again; others do not', async ($, on) => {
  const w = world(on)
  await $.session.start(START)
  const reads = () => w.runs.filter(argv => argv[1] === 'rev-list').length
  const before = reads()
  await $.tool.call({ tool: 'Bash', command: 'git status' } as never)
  expect(reads()).toBe(before)
  w.counts.main = '0\t0'
  await $.tool.call({ tool: 'Bash', command: 'git pull --ff-only' } as never)
  expect(reads()).toBeGreaterThan(before)
  const ui = await $.ui.mount({ ...BAND, surface: 'terminal' })
  expect(await ui.find({ type: 'Text', text: /behind origin/ })).toBeUndefined()
  await ui.unmount()
})

test('the last fetch\'s age is said, and nothing ever fetches or changes anything', async ($, on) => {
  const w = world(on)
  w.fetchedAt = w.clock.now() - 3 * 86_400_000
  await $.session.start(START)
  await $.tool.call({ tool: 'Bash', command: 'git pull --ff-only' } as never)
  const ui = await $.ui.mount({ ...BAND, surface: 'terminal' })
  expect(await ui.find({ type: 'Text', text: /as of the last fetch, 3 days ago/ })).toBeDefined()
  await ui.unmount()
  const READ_ONLY = new Set(['rev-parse', 'for-each-ref', 'rev-list', 'log'])
  for (const argv of w.runs.filter(a => a[0] === 'git')) expect(READ_ONLY.has(argv[1] ?? ''), argv.join(' ')).toBe(true)
})

// ------------------------------------------------------------ session notice

const row = (text: string) =>
  ({ message: { type: 'attachment', name: 'hook_success', role: 'user', content: [{ type: 'text', text }] }, door: 'hook-context', origin: { kind: 'hook', event: 'SessionStart' }, uuid: 'u1' }) as never

test('the update notice is shown once, in the script\'s own words, and other rows are left alone', async ($, on) => {
  const w = world(on)
  await $.session.start(START)
  await $.session.append(row(UPDATE_ROW))
  await $.session.append(row(UPDATE_ROW))
  expect(w.toasts).toEqual(['[ai-gent] update available: 2.20.0 → 2.21.0 — run scripts/ai-gent-update.sh'])

  const other = { message: { type: 'user', role: 'user', content: [{ type: 'text', text: UPDATE_ROW }] }, door: 'prompt', origin: { kind: 'unclassified' }, uuid: 'u2' } as never
  await $.session.append(other)
  expect(w.toasts.length).toBe(1)
})

test('sessionToasts off shows nothing', { options: { sessionToasts: false } }, async ($, on) => {
  const w = world(on)
  await $.session.start(START)
  await $.session.append(row(UPDATE_ROW))
  expect(w.toasts).toEqual([])
})

// ------------------------------------------------------------ /release-preview

test('/release-preview prints the dry run, and only ever the dry run', async ($, on) => {
  const w = world(on)
  await $.session.start(START)
  expect(w.registered).toContain('release-preview')

  expect((await $.command.run({ command: 'release-preview', args: '' } as never)).text).toBe('release 2.21.0 (minor)')
  const ran = w.runs.filter(a => a[0] === 'python3')
  expect(ran.length).toBe(1)
  expect(ran[0]).toEqual(['python3', `${ROOT}/scripts/release.py`, '--dry-run'])

  w.release.exit = 3
  expect((await $.command.run({ command: 'release-preview', args: '' } as never)).text).toBe('No release needed (nothing releasable since the last tag).')
  w.release.exit = 2
  w.release.stderr = 'boom'
  expect((await $.command.run({ command: 'release-preview', args: '' } as never)).text).toBe('scripts/release.py --dry-run failed (exit 2): boom')
  w.release.exists = false
  expect((await $.command.run({ command: 'release-preview', args: '' } as never)).text).toContain('devsecops:pipeline')
  for (const argv of w.runs.filter(a => a[0] === 'python3')) expect(argv).toContain('--dry-run')
})

test('releasePreview off declares nothing', { options: { releasePreview: false } }, async ($, on) => {
  const w = world(on)
  await $.session.start(START)
  expect(w.registered).toEqual([])
})

test('hygieneBand off reads no branch', { options: { hygieneBand: false } }, async ($, on) => {
  const w = world(on)
  await $.session.start(START)
  expect(w.runs.filter(a => a[1] === 'rev-list')).toEqual([])
})
