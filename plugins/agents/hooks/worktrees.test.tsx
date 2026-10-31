import { expect, mock, test } from 'claude-code/testing'

import { GH_PR_LIST, WT_LIST, WT_LIST_NO_BASE } from './worktrees-fixtures'
import { MAX_ROWS, checksOf, joinPrs, markOf, parseGh, parseWt, text as boardText, view } from './worktrees-model'
import type { Board, Row } from './worktrees-model'

type Handlers = Parameters<Parameters<typeof test>[1]>[1]

const ROOT = '/repo'
const START = { cwd: ROOT, surface: 'terminal', isInteractive: true } as const
const PANE = {
  plugin: 'agents',
  component: 'Pane',
  requestId: 'worktrees',
  props: { title: 'Worktrees', isFocused: false, bodyColumns: 100, placement: 'dock', scroll: { offset: 0, bodyRows: 40 }, view: {} },
} as const
const GH = { usable: true, platform: 'github' }

// ----------------------------------------------------------------- the model

const rows = (): Row[] => parseWt(WT_LIST) ?? []
const joined = (): Row[] => joinPrs(rows(), parseGh(GH_PR_LIST) ?? {})
const board = (extra: Partial<Board> = {}): Board => ({ rows: joined(), error: null, gh: GH, ...extra })
const lines = (b: Board, columns = 100) => view(b, columns).map(l => l.text).join('\n')

test('wt.py list --json is read by exactly its keys, in its order, nulls kept', () => {
  expect(rows().map(r => r.branch)).toEqual(['feat/us1', 'feat/us2', 'feat/us3', 'feat/us4', '(detached)'])
  expect(rows()[0]).toEqual({ branch: 'feat/us1', path: '.worktrees/feat-us1', dirty: true, ahead: 3, behind: 0 })
  expect(rows()[4]).toMatchObject({ ahead: null, behind: null })
  expect(parseWt(WT_LIST_NO_BASE)?.[0]).toMatchObject({ ahead: null, behind: null })
  expect(parseWt('[]')).toEqual([])
  expect(parseWt('not json')).toBeNull()
  expect(parseWt('{"a": 1}')).toBeNull()
  expect(parseWt('[{"branch": "x"}]')).toBeNull() // a key missing: the script changed
})

test('the check rollup is failing, then pending, then passing, else none', () => {
  const run = (conclusion: string, status = 'COMPLETED') => ({ __typename: 'CheckRun', status, conclusion })
  const ok = run('SUCCESS')
  for (const c of ['FAILURE', 'TIMED_OUT', 'CANCELLED', 'ACTION_REQUIRED', 'STARTUP_FAILURE']) expect(checksOf([ok, run(c)])).toBe('failing')
  expect(checksOf([{ __typename: 'StatusContext', state: 'FAILURE' }])).toBe('failing')
  expect(checksOf([{ __typename: 'StatusContext', state: 'ERROR' }])).toBe('failing')
  expect(checksOf([ok, run('', 'IN_PROGRESS')])).toBe('pending')
  expect(checksOf([ok, run('', 'QUEUED')])).toBe('pending')
  expect(checksOf([{ state: 'PENDING' }])).toBe('pending')
  expect(checksOf([{ state: 'EXPECTED' }])).toBe('pending')
  expect(checksOf([run('FAILURE'), run('', 'IN_PROGRESS')])).toBe('failing') // first match wins
  expect(checksOf([ok, run('NEUTRAL'), run('SKIPPED'), { state: 'SUCCESS' }])).toBe('passing')
  expect(checksOf([])).toBe('none')
  expect(checksOf(undefined)).toBe('none')
})

test('a pull request joins its row by headRefName; an open one wins over an older closed one', () => {
  const prs = parseGh(GH_PR_LIST)
  expect(Object.keys(prs ?? {}).sort()).toEqual(['feat/old', 'feat/us1', 'feat/us2', 'feat/us3'])
  expect(prs?.['feat/us1']).toEqual({ number: 41, state: 'OPEN', isDraft: false, checks: 'failing' })
  expect(prs?.['feat/us2']).toEqual({ number: 42, state: 'OPEN', isDraft: true, checks: 'pending' })
  expect(prs?.['feat/us3']).toEqual({ number: 43, state: 'MERGED', isDraft: false, checks: 'passing' })
  const out = joined()
  expect(out[0]?.pr?.number).toBe(41)
  expect(out[3]?.pr).toBeNull() // no pull request
  expect(out[4]?.pr).toBeUndefined() // detached: not looked up
  expect(parseGh('not json')).toBeNull()
  expect(parseGh('{}')).toBeNull()
  expect(parseGh('[]')).toEqual({})
})

test('the marks: failing, removable on a merged pull request and a clean tree, nothing of its own', () => {
  const [us1, us2, us3, us4] = joined() as [Row, Row, Row, Row]
  expect(markOf(us1)).toBe('failing')
  expect(markOf(us2)).toBeNull()
  expect(markOf(us3)).toBe('removable') // ahead 4: a rebase merge keeps it above 0
  expect(markOf({ ...us3, dirty: true })).toBeNull() // merged but not clean
  expect(markOf(us4)).toBe('nothing of its own') // ahead 0, clean, no pull request
  expect(markOf({ ...us4, pr: { number: 9, state: 'MERGED', isDraft: false, checks: 'none' } })).toBe('removable')
  expect(markOf({ ...us4, ahead: null, behind: null })).toBeNull()
  expect(markOf({ ...us4, dirty: true })).toBeNull()
  expect(markOf({ ...us3, ahead: 0 })).toBe('removable')
})

test('wide rows carry counts, the dirty dot, the pull request and its checks, and the hint once for a removable row', () => {
  const out = lines(board())
  expect(out).toContain('5 worktrees · 1 uncommitted · 1 failing · 1 removable')
  expect(out).toMatch(/feat\/us1\s+\+3\/-0\s+●\s+#41 open\s+checks failing\s+failing/)
  expect(out).toMatch(/feat\/us2\s+\+2\/-1\s+#42 draft\s+checks pending/)
  expect(out).toMatch(/feat\/us3\s+\+4\/-0\s+#43 merged\s+checks passing\s+removable: wt\.py remove feat\/us3/)
  expect(out).toMatch(/feat\/us4\s+\+0\/-2\s+no PR\s+nothing of its own/)
  expect(out.match(/wt\.py remove/g)).toHaveLength(1)
  expect(out).not.toContain('wt.py remove feat/us1')
})

test('null counts read ? and one line says no dev branch was found, once', () => {
  const out = lines(board({ rows: [...rows().slice(0, 1), ...(parseWt(WT_LIST_NO_BASE) ?? []).map(r => ({ ...r, branch: 'feat/b' })), ...rows().slice(4)] }))
  expect(out).toMatch(/feat\/b\s+\?/)
  expect(out).toMatch(/\(detached\)\s+\?/)
  expect(out.match(/no dev branch found/g)).toHaveLength(1)
  expect(lines(board({ rows: rows().slice(0, 4) }))).not.toContain('no dev branch found')
})

test('without gh the pull request columns are omitted and one line says gh auth login would add them', () => {
  const out = lines(board({ rows: rows(), gh: { usable: false, platform: 'github' } }))
  expect(out).not.toMatch(/#41|no PR/)
  expect(out).toContain('gh auth login')
  expect(out).not.toContain('removable') // no hint without gh
  expect(out).toMatch(/feat\/us4\s+\+0\/-2\s+nothing of its own/)
})

test('another platform says its pull requests are not read yet', () => {
  const out = lines(board({ rows: rows(), gh: { usable: false, platform: 'gitlab' } }))
  expect(out).toContain('GitLab')
  expect(out).toContain('not read yet')
  expect(out).not.toContain('gh auth login')
})

test('a pull request still being read shows an ellipsis, and a row not looked up a dash', () => {
  const out = lines(board({ rows: rows().map(r => ({ ...r })) }))
  expect(out).toMatch(/feat\/us1\s+\+3\/-0\s+●\s+…/)
  expect(lines(board())).toMatch(/\(detached\)\s+\?\s+—/)
})

test('the narrow form is branch +a/-b and a dot', () => {
  const out = lines(board(), 40)
  expect(out).toContain('feat/us1 +3/-0 ●')
  expect(out).toContain('feat/us2 +2/-1')
  expect(out).not.toContain('#41')
  expect(out).not.toContain('checks')
})

test('a failed wt.py says so with the first stderr line, over the last rows', () => {
  const out = lines(board({ error: 'fatal: not a git repository' }))
  expect(out).toContain('wt.py failed: fatal: not a git repository')
  expect(out).toContain('feat/us1')
})

test('no rows says there are no worktrees', () => {
  expect(lines(board({ rows: [] }))).toContain('no worktrees under .worktrees/')
})

test('more than the cap shows +N more', () => {
  const many = Array.from({ length: MAX_ROWS + 5 }, (_, i) => ({ branch: `feat/b${i}`, path: `.worktrees/b${i}`, dirty: false, ahead: 1, behind: 0 }))
  const out = lines(board({ rows: many, gh: { usable: false, platform: 'github' } }))
  expect(out).toContain('+5 more')
  expect(out).toContain(`feat/b${MAX_ROWS - 1}`)
  expect(out).not.toContain(`feat/b${MAX_ROWS} `)
  expect(out).toContain(`${MAX_ROWS + 5} worktrees`)
})

test('the text form lists every row, wide, for a surface that cannot dock the pane', () => {
  const out = boardText(board())
  expect(out).toContain('5 worktrees · 1 uncommitted · 1 failing · 1 removable')
  expect(out).toMatch(/feat\/us3\s+\+4\/-0\s+#43 merged/)
  expect(boardText(board({ rows: [] }))).toBe('no worktrees under .worktrees/')
})

// ------------------------------------------------------------------ the flow

type World = {
  runs: string[][]
  wt: { exit: number; stdout: string; stderr: string; reject: boolean; gate: Promise<void> | null }
  gh: { exit: number; prs: string; prsExit: number }
  remote: { exit: number; url: string }
  opened: number
  placed: boolean
  registered: string[]
  toasts: string[]
  clock: ReturnType<typeof mock.clock>
}

// Nothing sits beneath the plugin: these hooks stand for the engine, a repository
// at ROOT, wt.py, git and gh, edited by the test between steps. Nothing touches the network.
function world(on: Handlers): World {
  const w: World = {
    runs: [],
    wt: { exit: 0, stdout: WT_LIST, stderr: '', reject: false, gate: null },
    gh: { exit: 0, prs: GH_PR_LIST, prsExit: 0 },
    remote: { exit: 0, url: 'git@github.com:acme/shop.git\n' },
    opened: 0,
    placed: true,
    registered: [],
    toasts: [],
    clock: mock.clock(on, { now: 1_800_000_000_000 }),
  }
  on('session.start', (_, e) => ({ cwd: e.cwd }))
  on('session.cwd', () => ({ value: ROOT }))
  on('turn.complete', (_, e) => ({ text: e.text }))
  on('command.register', (_, e) => {
    w.registered.push(e.name)
    return { value: { command: e.name } }
  })
  on('ui.open', () => {
    w.opened += 1
    return { value: { isPlaced: w.placed } }
  })
  on('ui.close', () => undefined)
  on('ui.toast', (_, e) => {
    w.toasts.push(e.text)
    return { value: undefined }
  })
  on('ui.status', () => ({ value: undefined }))
  on('fs.exists', () => ({ value: false }))
  on('fs.stat', () => ({ deny: 'ENOENT' }))
  on('fs.read', () => ({ deny: 'ENOENT' }))
  on('process.run', async (_, e) => {
    w.runs.push([...e.argv])
    const [bin, a, b] = e.argv
    if (bin === 'python3' && e.argv.includes('list')) {
      if (w.wt.gate) await w.wt.gate
      if (w.wt.reject) return { deny: 'timed out' }
      return { value: { exitCode: w.wt.exit, stdout: w.wt.stdout, stderr: w.wt.stderr } }
    }
    if (bin === 'git' && a === 'remote') return { value: { exitCode: w.remote.exit, stdout: w.remote.url, stderr: '' } }
    if (bin === 'gh' && a === 'auth') return { value: { exitCode: w.gh.exit, stdout: '', stderr: '' } }
    if (bin === 'gh' && a === 'pr' && b === 'list') return { value: { exitCode: w.gh.prsExit, stdout: w.gh.prs, stderr: '' } }
    return { deny: `unexpected ${e.argv.join(' ')}` }
  })
  on('tool.call', () => ({ result: {} }))
  return w
}

const open = () => ({ command: 'worktree-board', args: '' }) as never
const wtRuns = (w: World) => w.runs.filter(r => r[0] === 'python3' && r.includes('list'))
const ghRuns = (w: World) => w.runs.filter(r => r[0] === 'gh' && r[1] === 'pr')
const authRuns = (w: World) => w.runs.filter(r => r[0] === 'gh' && r[1] === 'auth')
const remoteRuns = (w: World) => w.runs.filter(r => r[0] === 'git')
const GH_ARGV = ['gh', 'pr', 'list', '--state', 'all', '--limit', '100', '--json', 'number,headRefName,state,isDraft,statusCheckRollup']

test('/worktree-board opens a pane listing every row on terminal and desktop, and never opens by itself', async ($, on) => {
  const w = world(on)
  await $.session.start(START)
  expect(w.registered).toContain('worktree-board')
  expect(w.opened).toBe(0)
  expect(w.runs).toEqual([]) // nothing spawned before the command
  expect((await $.command.run(open())).text).toBe('Worktree board opened.')
  expect(w.opened).toBe(1)
  for (const surface of ['terminal', 'desktop'] as const) {
    const ui = await $.ui.mount({ ...PANE, surface })
    expect(await ui.find({ type: 'Text', text: /5 worktrees · 1 uncommitted · 1 failing · 1 removable/ })).toBeDefined()
    expect(await ui.find({ type: 'Text', text: /feat\/us1\s+\+3\/-0\s+●\s+#41 open\s+checks failing/ })).toBeDefined()
    expect(await ui.find({ type: 'Text', text: /feat\/us3.*#43 merged.*removable: wt\.py remove feat\/us3/ })).toBeDefined()
    expect(await ui.find({ type: 'Text', text: /feat\/us4.*no PR.*nothing of its own/ })).toBeDefined()
    await ui.unmount()
  }
  // The spawns are the four the spec names, with gh once.
  expect(wtRuns(w)).toHaveLength(1)
  expect(wtRuns(w)[0]?.slice(2)).toEqual(['list', '--json'])
  expect(remoteRuns(w)).toEqual([['git', 'remote', 'get-url', 'origin']])
  expect(authRuns(w)).toEqual([['gh', 'auth', 'status']])
  expect(ghRuns(w)).toEqual([GH_ARGV])
})

test('with no worktrees the command says so and opens nothing', async ($, on) => {
  const w = world(on)
  w.wt.stdout = '[]'
  await $.session.start(START)
  expect((await $.command.run(open())).text).toBe('no worktrees under .worktrees/')
  expect(w.opened).toBe(0)
  await w.clock.advance(600_000)
  expect(w.runs.filter(r => r[0] === 'python3')).toHaveLength(1) // not left polling
})

test('null counts are drawn as ? with the no dev branch line', async ($, on) => {
  const w = world(on)
  w.wt.stdout = WT_LIST_NO_BASE
  w.gh.prs = '[]'
  await $.session.start(START)
  await $.command.run(open())
  const ui = await $.ui.mount({ ...PANE, surface: 'terminal' })
  expect(await ui.find({ type: 'Text', text: /feat\/us1\s+\?\s+no PR/ })).toBeDefined()
  expect(await ui.find({ type: 'Text', text: /no dev branch found/ })).toBeDefined()
  await ui.unmount()
})

test('gh not signed in: no gh pr list, local columns, and the gh auth login line', async ($, on) => {
  const w = world(on)
  w.gh.exit = 1
  await $.session.start(START)
  await $.command.run(open())
  const ui = await $.ui.mount({ ...PANE, surface: 'terminal' })
  expect(await ui.find({ type: 'Text', text: /gh auth login/ })).toBeDefined()
  expect(await ui.find({ type: 'Text', text: /feat\/us1\s+\+3\/-0\s+●/ })).toBeDefined()
  expect(await ui.find({ type: 'Text', text: /#41/ })).toBeUndefined()
  expect(await ui.find({ type: 'Text', text: /wt\.py remove/ })).toBeUndefined()
  await ui.unmount()
  await w.clock.advance(600_000)
  expect(ghRuns(w)).toHaveLength(0)
})

test('a remote that is not GitHub reads no pull requests and does not even ask gh', async ($, on) => {
  const w = world(on)
  w.remote.url = 'https://gitlab.com/acme/shop.git\n'
  await $.session.start(START)
  await $.command.run(open())
  const ui = await $.ui.mount({ ...PANE, surface: 'terminal' })
  expect(await ui.find({ type: 'Text', text: /GitLab.*not read yet/ })).toBeDefined()
  await ui.unmount()
  expect(authRuns(w)).toHaveLength(0)
  expect(ghRuns(w)).toHaveLength(0)
})

test('a failing gh pr list keeps the rows and tries again at the next interval', async ($, on) => {
  const w = world(on)
  w.gh.prsExit = 1
  w.gh.prs = ''
  await $.session.start(START)
  await $.command.run(open())
  w.gh.prsExit = 0
  w.gh.prs = GH_PR_LIST
  await w.clock.advance(120_000)
  const ui = await $.ui.mount({ ...PANE, surface: 'terminal' })
  expect(await ui.find({ type: 'Text', text: /#41 open/ })).toBeDefined()
  await ui.unmount()
})

test('a Bash call that runs wt.py, git commit or git merge refreshes the local rows at once, gh not', async ($, on) => {
  const w = world(on)
  await $.session.start(START)
  await $.command.run(open())
  w.runs.length = 0
  await $.tool.call({ tool: 'Bash', command: 'ls -la' } as never)
  await w.clock.settle()
  expect(w.runs).toEqual([])
  for (const command of ['python3 plugins/agents/scripts/wt.py add feat/x', 'git commit -m "feat: x"', 'git merge --no-ff feat/x', 'git -C .worktrees/x commit -m y']) {
    w.runs.length = 0
    await $.tool.call({ tool: 'Bash', command } as never)
    await w.clock.settle()
    expect(wtRuns(w)).toHaveLength(1)
    expect(w.runs.filter(r => r[0] !== 'python3')).toEqual([])
  }
})

test('the Bash refresh redraws the rows and never overlaps a run in flight', async ($, on) => {
  const w = world(on)
  await $.session.start(START)
  await $.command.run(open())
  const ui = await $.ui.mount({ ...PANE, surface: 'terminal' })
  w.runs.length = 0
  w.wt.stdout = WT_LIST.replace('"ahead": 3', '"ahead": 5')
  let release = () => {}
  w.wt.gate = new Promise<void>(resolve => (release = resolve))
  await $.tool.call({ tool: 'Bash', command: 'git commit -m a' } as never)
  await $.tool.call({ tool: 'Bash', command: 'git commit -m b' } as never)
  await w.clock.settle()
  expect(wtRuns(w)).toHaveLength(1)
  w.wt.gate = null
  release()
  await w.clock.settle()
  expect(await ui.find({ type: 'Text', text: /feat\/us1\s+\+5\/-0/ })).toBeDefined()
  await ui.unmount()
})

test('a Bash refresh counts as the local run: the next tick inside the interval is skipped', async ($, on) => {
  const w = world(on)
  await $.session.start(START)
  await $.command.run(open())
  await w.clock.advance(10_000)
  w.runs.length = 0
  await $.tool.call({ tool: 'Bash', command: 'git commit -m a' } as never)
  await w.clock.settle()
  expect(wtRuns(w)).toHaveLength(1)
  await w.clock.advance(5_000) // the tick at 15 s: 5 s after the refresh
  expect(wtRuns(w)).toHaveLength(1)
  await w.clock.advance(15_000)
  expect(wtRuns(w)).toHaveLength(2)
})

test('the local interval runs wt.py again and the remote interval runs gh', async ($, on) => {
  const w = world(on)
  await $.session.start(START)
  await $.command.run(open())
  w.runs.length = 0
  await w.clock.advance(15_000)
  expect(wtRuns(w)).toHaveLength(1)
  expect(ghRuns(w)).toHaveLength(0)
  await w.clock.advance(15_000)
  expect(wtRuns(w)).toHaveLength(2)
  await w.clock.advance(90_000) // 120 s after the opening
  expect(ghRuns(w)).toHaveLength(1)
  expect(authRuns(w)).toHaveLength(0) // gh auth status and the remote are read once, not per refresh
  expect(remoteRuns(w)).toHaveLength(0)
})

test('open for ten minutes with nothing changing, gh pr list runs six times at most', async ($, on) => {
  const w = world(on)
  await $.session.start(START)
  await $.command.run(open())
  await w.clock.advance(600_000)
  expect(ghRuns(w)).toHaveLength(6)
})

test('the intervals are options', { options: { worktreeBoardSeconds: 5, worktreeBoardRemoteSeconds: 30 } }, async ($, on) => {
  const w = world(on)
  await $.session.start(START)
  await $.command.run(open())
  w.runs.length = 0
  await w.clock.advance(30_000)
  expect(wtRuns(w)).toHaveLength(6)
  expect(ghRuns(w)).toHaveLength(1)
})

// The harness cannot raise `ui.close`, so a pane that cannot be placed stands for a closed one:
// the command then closes the board's state and stops the polls.
test('a closed board spawns nothing', async ($, on) => {
  const w = world(on)
  await $.session.start(START)
  await $.command.run(open())
  w.placed = false
  await $.command.run(open())
  w.runs.length = 0
  await w.clock.advance(600_000)
  await $.tool.call({ tool: 'Bash', command: 'git commit -m a' } as never)
  await w.clock.settle()
  expect(w.runs).toEqual([])
})

test('wt.py failing keeps the last rows and says why with the first stderr line', async ($, on) => {
  const w = world(on)
  await $.session.start(START)
  await $.command.run(open())
  w.wt.exit = 1
  w.wt.stdout = ''
  w.wt.stderr = 'fatal: not a git repository\nsecond line\n'
  await w.clock.advance(15_000)
  const ui = await $.ui.mount({ ...PANE, surface: 'terminal' })
  expect(await ui.find({ type: 'Text', text: /wt\.py failed: fatal: not a git repository$/ })).toBeDefined()
  expect(await ui.find({ type: 'Text', text: /second line/ })).toBeUndefined()
  expect(await ui.find({ type: 'Text', text: /feat\/us1\s+\+3\/-0/ })).toBeDefined()
  w.wt.exit = 0
  w.wt.stdout = WT_LIST
  await w.clock.advance(15_000)
  expect(await ui.find({ type: 'Text', text: /wt\.py failed/ })).toBeUndefined()
  await ui.unmount()
})

test('wt.py timing out is reported the same way', async ($, on) => {
  const w = world(on)
  await $.session.start(START)
  await $.command.run(open())
  w.wt.reject = true
  await w.clock.advance(15_000)
  const ui = await $.ui.mount({ ...PANE, surface: 'terminal' })
  expect(await ui.find({ type: 'Text', text: /wt\.py failed/ })).toBeDefined()
  expect(await ui.find({ type: 'Text', text: /feat\/us1/ })).toBeDefined()
  await ui.unmount()
})

test('a reload keeps polling the open board and does not reread gh auth', async ($, on) => {
  const w = world(on)
  await $.session.start(START)
  await $.command.run(open())
  await $.session.start(START)
  await w.clock.settle()
  w.runs.length = 0
  await w.clock.advance(15_000)
  expect(wtRuns(w)).toHaveLength(1) // one poll, not two
  expect(authRuns(w)).toHaveLength(0)
})

test('where no pane can open the command prints the text form and leaves nothing polling', async ($, on) => {
  const w = world(on)
  w.placed = false
  await $.session.start({ ...START, surface: 'mobile' })
  const plain = (await $.command.run(open())).text ?? ''
  expect(plain).toContain('5 worktrees · 1 uncommitted · 1 failing · 1 removable')
  expect(plain).toMatch(/feat\/us3\s+\+4\/-0\s+#43 merged/)
  const count = w.runs.length
  await w.clock.advance(600_000)
  expect(w.runs).toHaveLength(count)
})

test('the calls work with a Bash guard installed: they are process.run events, not Bash tool calls', async ($, on) => {
  const w = world(on)
  on('tool.call', { tool: 'Bash' }, () => ({ deny: 'guard' }))
  await $.session.start(START)
  expect((await $.command.run(open())).text).toBe('Worktree board opened.')
  expect(wtRuns(w)).toHaveLength(1)
  expect(ghRuns(w)).toHaveLength(1)
})

test('the board never writes: no tool, write or spawn beyond the four reads', async ($, on) => {
  const w = world(on)
  await $.session.start(START)
  await $.command.run(open())
  await w.clock.advance(300_000)
  for (const argv of w.runs) {
    const ok = (argv[0] === 'python3' && argv.slice(-2).join(' ') === 'list --json') || (argv[0] === 'git' && argv.join(' ') === 'git remote get-url origin') || argv.join(' ') === 'gh auth status' || (argv[0] === 'gh' && argv[1] === 'pr' && argv[2] === 'list')
    expect(ok).toBe(true)
  }
})

test('worktreeBoard off registers nothing and leaves the token status and the card pane', { options: { worktreeBoard: false } }, async ($, on) => {
  const w = world(on)
  await $.session.start(START)
  expect(w.registered).not.toContain('worktree-board')
  expect(w.registered).toEqual(expect.arrayContaining(['tokens', 'cards-pane']))
})

test('tokenStatus off does not turn the board off', { options: { tokenStatus: false } }, async ($, on) => {
  const w = world(on)
  await $.session.start(START)
  expect(w.registered).toContain('worktree-board')
  expect(w.registered).not.toContain('tokens')
  expect((await $.command.run(open())).text).toBe('Worktree board opened.')
})

test('the card pane off does not turn the board off', { options: { cardsPane: false } }, async ($, on) => {
  const w = world(on)
  await $.session.start(START)
  expect(w.registered).toContain('worktree-board')
  expect(w.registered).not.toContain('cards-pane')
})

test('the board registers beside the token status and the card pane', async ($, on) => {
  const w = world(on)
  await $.session.start(START)
  expect(w.registered).toEqual(expect.arrayContaining(['tokens', 'cards-pane', 'worktree-board']))
})
