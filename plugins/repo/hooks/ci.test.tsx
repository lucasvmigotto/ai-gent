import { expect, mock, test } from 'claude-code/testing'

import type { Band } from './band-priority'

type Handlers = Parameters<Parameters<typeof test>[1]>[1]

const ROOT = '/repo'
const SHA = '3a9f1c2e0d4b5a6978877665544332211aabbccd'
const START = { cwd: ROOT, surface: 'terminal', isInteractive: true } as const
const BAND = {
  plugin: 'repo',
  component: 'AbovePrompt',
  props: { hasSurvey: false, isWorking: false, maxRows: 10, bodyColumns: 120, scroll: { offset: 0, bodyRows: 10 }, view: {} },
} as const
const PUSH = { tool: 'Bash', command: 'git push origin dev' } as const

// Recorded shapes of the two reporun answers (no network in tests).
const detect = (platform = 'github', inPass = false, installed = true, signedIn = true) =>
  JSON.stringify({ repository: { platform }, platforms: { [platform]: { token_entry: `ai-gent/${platform}/token`, token_in_pass: inPass, cli: { name: 'gh', installed, signed_in: signedIn } } } }, null, 2)

type GhRun = { databaseId: number; name: string; workflowName: string; status: string; conclusion: string; headSha?: string }
const gh = (id: number, name: string, status: string, conclusion = ''): GhRun => ({ databaseId: id, name, workflowName: name, status, conclusion, headSha: SHA })
const out = (runs: GhRun[], before = '') => ({ exitCode: 0, stdout: `${before}${JSON.stringify(runs)}\n`, stderr: '' })
const RUNNING = [gh(1, 'Docs CI', 'in_progress'), gh(2, 'check', 'completed', 'success')]
const GREEN = [gh(1, 'Docs CI', 'completed', 'success'), gh(2, 'check', 'completed', 'success')]
const RED = [gh(1, 'Docs CI', 'in_progress'), gh(2, 'check', 'completed', 'failure')]

type Reply = { exitCode: number; stdout: string; stderr: string }
type World = {
  runs: string[][]
  toasts: string[]
  detect: string
  hasWorkflows: boolean
  list: Reply[]
  others: Band[] | 'absent'
  // What the other band plugins' atoms hold, by plugin (git's falls back to `others`); 'throws' is a denied read.
  atoms: Record<string, Band[] | 'throws'>
  pushFails: boolean
  clock: ReturnType<typeof mock.clock>
}

const lists = (w: World) => w.runs.filter(a => a.includes('cli'))

// Nothing sits beneath the plugin: these hooks stand for the engine, git and reporun.
function world(on: Handlers): World {
  const w: World = { runs: [], toasts: [], detect: detect(), hasWorkflows: true, list: [out(RUNNING)], others: 'absent', atoms: {}, pushFails: false, clock: mock.clock(on, { now: 1_000_000 }) }
  on('session.start', (_, e) => ({ cwd: e.cwd }))
  on('session.cwd', () => ({ value: ROOT }))
  on('fs.exists', (_, e) => ({ value: w.hasWorkflows && e.path === `${ROOT}/.github/workflows` }))
  on('tool.call', () => (w.pushFails ? { deny: 'exit 1' } : { result: {} }))
  on('ui.toast', (_, e) => {
    w.toasts.push(e.text)
    return { value: undefined }
  })
  on('state.get', async (_, e, next) => {
    if (e.plugin === 'repo') return next(e)
    const held = w.atoms[e.plugin] ?? (e.plugin === 'git' ? w.others : 'absent')
    if (held === 'throws') return { deny: 'no such plugin' }
    return { value: held === 'absent' ? { value: undefined, version: 0 } : { value: held, version: 1 } }
  })
  on('process.run', (_, e) => {
    const argv = [...e.argv]
    w.runs.push(argv)
    const ok = (stdout: string): { value: Reply } => ({ value: { exitCode: 0, stdout, stderr: '' } })
    if (argv[0] === 'git' && argv.includes('--abbrev-ref')) return ok('dev\n')
    if (argv[0] === 'git' && argv[1] === 'rev-parse') return ok(`${SHA}\n`)
    if (argv.includes('detect')) return ok(w.detect)
    if (argv.includes('cli')) return { value: w.list.length > 1 ? (w.list.shift() as Reply) : w.list[0] }
    return { value: { exitCode: 1, stdout: '', stderr: 'unexpected' } }
  })
  on('ui.render', ($, e) => {
    const { Text } = $.ui.resolve(e)
    return <Text>beneath</Text>
  })
  return w
}

const ci = { type: 'Text', text: /^(CI |no runs started)/ } as const
async function band(
  $: Parameters<Parameters<typeof test>[1]>[0],
  surface: 'terminal' | 'desktop' = 'terminal',
): Promise<string | undefined> {
  const ui = await $.ui.mount({ ...BAND, surface })
  const found = await ui.find(ci)
  await ui.unmount()
  return found === undefined ? undefined : String((found as { text?: string }).text ?? '')
}

async function pushed(w: World, $: Parameters<Parameters<typeof test>[1]>[0]) {
  await $.session.start(START)
  await $.tool.call(PUSH as never)
  await w.clock.settle()
}

// ------------------------------------------------------------------ story 1

for (const surface of ['terminal', 'desktop'] as const) {
  test(`${surface}: after a push the band shows the commit, the branch and each run, and updates`, async ($, on) => {
    const w = world(on)
    w.list = [out(RUNNING), out(GREEN)]
    await pushed(w, $)
    expect(await band($, surface)).toBe('CI for 3a9f1c2 on dev: Docs CI ⏳ · check ✓')
    await w.clock.advance(29_000)
    expect(lists(w).length).toBe(1)
    await w.clock.advance(1_000)
    expect(lists(w).length).toBe(2)
    expect(await band($, surface)).toBe('CI for 3a9f1c2 on dev: Docs CI ✓ · check ✓ — more runs may follow')
    // done: no more reads, and the result clears after a short time
    await w.clock.advance(10 * 60_000)
    expect(lists(w).length).toBe(2)
    expect(await band($, surface)).toBeUndefined()
  })
}

test('the runs are found by commit and read through reporun, never the latest, never --write', async ($, on) => {
  const w = world(on)
  await pushed(w, $)
  const call = lists(w)[0]
  expect(call.slice(2)).toEqual(['cli', 'github', '--', 'run', 'list', '--commit', SHA, '--json', 'databaseId,name,status,conclusion,workflowName,headSha'])
  expect(call[1]).toMatch(/scripts\/reporun\.py$/)
})

test('the polls back off 30 s, 60 s, 120 s and stop at the hard stop', { options: { ciBandMinutes: 6 } }, async ($, on) => {
  const w = world(on)
  await pushed(w, $)
  expect(lists(w).length).toBe(1) // t=0
  await w.clock.advance(30_000)
  expect(lists(w).length).toBe(2) // t=30
  await w.clock.advance(59_000)
  expect(lists(w).length).toBe(2)
  await w.clock.advance(1_000)
  expect(lists(w).length).toBe(3) // t=90
  await w.clock.advance(120_000)
  expect(lists(w).length).toBe(4) // t=210
  await w.clock.advance(120_000)
  expect(lists(w).length).toBe(5) // t=330
  await w.clock.advance(120_000)
  expect(lists(w).length).toBe(5) // t=450 > 360: the hard stop
  expect(await band($)).toBeUndefined()
  await w.clock.advance(20 * 60_000)
  expect(lists(w).length).toBe(5)
})

test('no runs says why, and clears after the second empty check', async ($, on) => {
  const w = world(on)
  w.list = [out([])]
  await pushed(w, $)
  expect(await band($)).toBe('no runs started for 3a9f1c2 (paths, branches or a bot commit?)')
  await w.clock.advance(30_000)
  expect(lists(w).length).toBe(2)
  await w.clock.advance(10 * 60_000)
  expect(lists(w).length).toBe(2)
  expect(await band($)).toBeUndefined()
})

test('runs that start late are picked up after an empty first read', async ($, on) => {
  const w = world(on)
  w.list = [out([]), out(RUNNING)]
  await pushed(w, $)
  await w.clock.advance(30_000)
  expect(await band($)).toBe('CI for 3a9f1c2 on dev: Docs CI ⏳ · check ✓')
})

test('a failed push, a dry run, a push-like echo and other commands do nothing', async ($, on) => {
  const w = world(on)
  await $.session.start(START)
  w.pushFails = true
  await $.tool.call(PUSH as never)
  w.pushFails = false
  for (const command of ['git push --dry-run', 'echo git push', 'git status', 'git pull']) await $.tool.call({ tool: 'Bash', command } as never)
  await w.clock.settle()
  expect(w.runs).toEqual([])
  expect(await band($)).toBeUndefined()
})

test('a repository with no .github/workflows calls nothing', async ($, on) => {
  const w = world(on)
  w.hasWorkflows = false
  await pushed(w, $)
  expect(w.runs).toEqual([])
})

test('hiding the band keeps it hidden for that push and stops the polling', async ($, on) => {
  const w = world(on)
  await pushed(w, $)
  const ui = await $.ui.mount({ ...BAND, surface: 'terminal' })
  await ui.press({ key: 'hide' } as never)
  await ui.unmount()
  expect(await band($)).toBeUndefined()
  await w.clock.advance(5 * 60_000)
  expect(lists(w).length).toBe(1)
  // the next push brings it back
  await $.tool.call(PUSH as never)
  await w.clock.settle()
  expect(await band($)).toBe('CI for 3a9f1c2 on dev: Docs CI ⏳ · check ✓')
})

test('a newer push supersedes the band, and the old timer reads no more', async ($, on) => {
  const w = world(on)
  await pushed(w, $)
  await $.tool.call(PUSH as never)
  await w.clock.settle()
  expect(lists(w).length).toBe(2)
  await w.clock.advance(30_000)
  expect(lists(w).length).toBe(3) // one timer, not two
})

test('detect is read once per session, and again after a failed read', async ($, on) => {
  const w = world(on)
  await pushed(w, $)
  await $.tool.call(PUSH as never)
  await w.clock.settle()
  expect(w.runs.filter(a => a.includes('detect')).length).toBe(1)
  w.list = [{ exitCode: 1, stdout: '', stderr: 'gh: boom' }]
  await w.clock.advance(30_000)
  await $.tool.call(PUSH as never)
  await w.clock.settle()
  expect(w.runs.filter(a => a.includes('detect')).length).toBe(2)
})

// ------------------------------------------------------------------ story 2

test('a failing run toasts once, naming the workflow and the skill', async ($, on) => {
  const w = world(on)
  w.list = [out(RED), out([gh(1, 'Docs CI', 'in_progress'), gh(2, 'check', 'completed', 'failure')]), out([gh(1, 'Docs CI', 'completed', 'failure'), gh(2, 'check', 'completed', 'failure')])]
  await pushed(w, $)
  expect(w.toasts).toEqual(['check failed on 3a9f1c2 — ask /devsecops:watch why'])
  await w.clock.advance(30_000)
  expect(w.toasts.length).toBe(1)
  await w.clock.advance(60_000)
  expect(w.toasts).toEqual(['check failed on 3a9f1c2 — ask /devsecops:watch why', 'Docs CI failed on 3a9f1c2 — ask /devsecops:watch why'])
  expect(await band($)).toContain('check ✗')
})

test('timed out and startup failures toast; cancelled and skipped do not', async ($, on) => {
  const w = world(on)
  w.list = [out([gh(1, 'a', 'completed', 'timed_out'), gh(2, 'b', 'completed', 'startup_failure'), gh(3, 'c', 'completed', 'cancelled'), gh(4, 'd', 'completed', 'skipped')])]
  await pushed(w, $)
  expect(w.toasts.length).toBe(2)
  expect(await band($)).toBe('CI for 3a9f1c2 on dev: a ✗ · b ✗ · c – · d – — more runs may follow')
  expect(w.toasts.join(' ')).not.toMatch(/c failed|d failed/)
})

test('a toast carries no log text, step names, secrets or variables', async ($, on) => {
  const w = world(on)
  w.list = [{ exitCode: 0, stdout: JSON.stringify([{ ...gh(1, 'check', 'completed', 'failure'), log: 'secret=abc', steps: ['Run tests'] }]), stderr: '' }]
  await pushed(w, $)
  expect(w.toasts).toEqual(['check failed on 3a9f1c2 — ask /devsecops:watch why'])
})

// ------------------------------------------------------------------ story 3

test('gh not signed in: one toast, once per session, and nothing else runs', async ($, on) => {
  const w = world(on)
  w.detect = detect('github', false, true, false)
  await pushed(w, $)
  await $.tool.call(PUSH as never)
  await w.clock.settle()
  expect(w.toasts.length).toBe(1)
  expect(w.toasts[0]).toContain('gh auth login')
  expect(lists(w)).toEqual([])
  expect(await band($)).toBeUndefined()
})

test('gh not installed counts as not signed in', async ($, on) => {
  const w = world(on)
  w.detect = detect('github', false, false, true)
  await pushed(w, $)
  expect(lists(w)).toEqual([])
  expect(w.toasts.length).toBe(1)
})

for (const platform of ['gitlab', 'azure-devops', 'bitbucket', 'forgejo']) {
  test(`${platform}: shows nothing and spawns nothing after detect`, async ($, on) => {
    const w = world(on)
    w.detect = detect(platform)
    await pushed(w, $)
    expect(w.runs.filter(a => !a.includes('detect'))).toEqual([])
    expect(w.toasts).toEqual([])
  })
}

test('detect that does not parse is silent', async ($, on) => {
  const w = world(on)
  w.detect = 'Traceback...'
  await pushed(w, $)
  expect(lists(w)).toEqual([])
  expect(w.toasts).toEqual([])
})

test('ciBand off runs nothing', { options: { ciBand: false } }, async ($, on) => {
  const w = world(on)
  await pushed(w, $)
  expect(w.runs).toEqual([])
  expect(await band($)).toBeUndefined()
})

test('a token in pass with ciBandUsePass off is never read through reporun cli, and says why once', async ($, on) => {
  const w = world(on)
  w.detect = detect('github', true)
  await pushed(w, $)
  await $.tool.call(PUSH as never)
  await w.clock.settle()
  expect(lists(w)).toEqual([])
  expect(w.toasts.length).toBe(1)
  expect(w.toasts[0]).toContain('ciBandUsePass')
})

test('a token in pass with ciBandUsePass on is read', { options: { ciBandUsePass: true } }, async ($, on) => {
  const w = world(on)
  w.detect = detect('github', true)
  await pushed(w, $)
  expect(lists(w).length).toBe(1)
})

test('a gh warning before the array is skipped; output with no array is a failed read', async ($, on) => {
  const w = world(on)
  w.list = [out(RUNNING, "reporun: gh run list · token from the CLI's own login\n! A new release of gh is available\n")]
  await pushed(w, $)
  expect(await band($)).toBe('CI for 3a9f1c2 on dev: Docs CI ⏳ · check ✓')
})

test('three failed reads in a row stop the polling with one line, keeping the last band', async ($, on) => {
  const w = world(on)
  w.list = [out(RUNNING), { exitCode: 0, stdout: 'HTTP 502 Bad Gateway', stderr: '' }, { exitCode: 1, stdout: '', stderr: 'boom' }, { exitCode: 0, stdout: 'not json', stderr: '' }]
  await pushed(w, $)
  await w.clock.advance(30_000) // fail 1
  await w.clock.advance(60_000) // fail 2
  expect(w.toasts).toEqual([])
  await w.clock.advance(120_000) // fail 3
  expect(lists(w).length).toBe(4)
  expect(w.toasts.length).toBe(1)
  expect(w.toasts[0]).toContain('devsecops:watch')
  expect(await band($)).toBe('CI for 3a9f1c2 on dev: Docs CI ⏳ · check ✓')
  await w.clock.advance(20 * 60_000)
  expect(lists(w).length).toBe(4)
})

test('a failed read keeps the last band and backs off', async ($, on) => {
  const w = world(on)
  w.list = [out(RUNNING), { exitCode: 1, stdout: '', stderr: 'boom' }, out(GREEN)]
  await pushed(w, $)
  await w.clock.advance(30_000)
  expect(await band($)).toBe('CI for 3a9f1c2 on dev: Docs CI ⏳ · check ✓')
  await w.clock.advance(60_000)
  expect(await band($)).toContain('Docs CI ✓')
})

test('headless: nothing runs and nothing polls', async ($, on) => {
  const w = world(on)
  await $.session.start({ cwd: ROOT, surface: 'terminal', isInteractive: false } as never)
  await $.tool.call(PUSH as never)
  await w.clock.settle()
  expect(w.runs).toEqual([])
})

// ------------------------------------------------------------------ the recorder (FR-001)

test('every command spawned in a push scenario is on the read-only list', async ($, on) => {
  const w = world(on)
  w.list = [out(RED), out(GREEN)]
  await pushed(w, $)
  await w.clock.advance(60_000)
  const allowed = [
    /^git rev-parse HEAD$/,
    /^git rev-parse --abbrev-ref HEAD$/,
    /^python3 \S+\/scripts\/reporun\.py detect --json$/,
    /^python3 \S+\/scripts\/reporun\.py cli github -- run list --commit [0-9a-f]{40} --json databaseId,name,status,conclusion,workflowName,headSha$/,
  ]
  expect(w.runs.length).toBeGreaterThan(3)
  for (const argv of w.runs) {
    const line = argv.join(' ')
    expect(allowed.some(re => re.test(line)), line).toBe(true)
    expect(line).not.toMatch(/--write|--log|rerun|cancel|workflow run|secret|variable/)
  }
})

// ------------------------------------------------------------------ priority (spec 030)

const HOLD: Band = { id: 'checkpoint', plugin: 'git', priority: 'hold', since: 1 }
const OLDER: Band = { id: 'hygiene', plugin: 'git', priority: 'normal', since: 5 }

test('a normal CI band yields to an older normal band under the cap', async ($, on) => {
  const w = world(on)
  w.others = [HOLD, OLDER]
  await pushed(w, $)
  expect(await band($)).toBeUndefined()
})

test('a failing CI band outranks a normal one and draws', async ($, on) => {
  const w = world(on)
  w.others = [HOLD, OLDER]
  w.list = [out(RED)]
  await pushed(w, $)
  expect(await band($)).toContain('check ✗')
})

test('it draws when there is room, and the hidden count is left to the plugin that owns the line', async ($, on) => {
  const w = world(on)
  w.others = [HOLD]
  await pushed(w, $)
  expect(await band($)).toBe('CI for 3a9f1c2 on dev: Docs CI ⏳ · check ✓')
})

// ------------------------------------------------------------------ cross-plugin cap (spec 030)

const THEIRS: Record<string, Band> = {
  project: { id: 'holds', plugin: 'project', priority: 'hold', since: 1 },
  git: { id: 'hygiene', plugin: 'git', priority: 'hold', since: 1 },
  devcontainer: { id: 'containers', plugin: 'devcontainer', priority: 'hold', since: 1 },
}
const MORE = { type: 'Text', text: /more band/ } as const
const moreText = async ($: Parameters<Parameters<typeof test>[1]>[0]): Promise<string | undefined> => {
  const ui = await $.ui.mount({ ...BAND, surface: 'terminal' })
  const found = await ui.find(MORE)
  await ui.unmount()
  return found === undefined ? undefined : String((found as { text?: string }).text ?? '')
}

for (const other of Object.keys(THEIRS)) {
  test(`a band from ${other} counts toward the cap: at a cap of one the CI band gives way and says so`, { options: { bandCap: 1 } }, async ($, on) => {
    const w = world(on)
    w.atoms = { [other]: [THEIRS[other] as Band] }
    await pushed(w, $)
    expect(await band($)).toBeUndefined()
    expect(await moreText($)).toBe('+1 more band (/bands)')
  })
}

test('a plugin that is absent or whose read throws adds no band, and the others still count', { options: { bandCap: 1 } }, async ($, on) => {
  const w = world(on)
  w.atoms = { project: 'throws', git: THEIRS.git ? [THEIRS.git] : [] }
  await pushed(w, $)
  expect(await band($)).toBeUndefined()
  expect(await moreText($)).toBe('+1 more band (/bands)')
})

test('with every other plugin absent the CI band draws at a cap of one', { options: { bandCap: 1 } }, async ($, on) => {
  const w = world(on)
  await pushed(w, $)
  expect(await band($)).toContain('CI for 3a9f1c2')
  expect(await moreText($)).toBeUndefined()
})

test('exactly one plugin draws the more-line: this one when its band is the first hidden, no one else otherwise', { options: { bandCap: 1 } }, async ($, on) => {
  const w = world(on)
  // a hold shows; this normal band (since 1_000_000) hides first, ahead of devcontainer's later info band
  w.atoms = { project: [THEIRS.project as Band], devcontainer: [{ id: 'containers', plugin: 'devcontainer', priority: 'info', since: 2e6 }] }
  await pushed(w, $)
  expect(await moreText($)).toBe('+2 more bands (/bands)')
})

test('a band that is hidden after another plugin\'s older one leaves the line to that plugin', { options: { bandCap: 1 } }, async ($, on) => {
  const w = world(on)
  w.atoms = { project: [THEIRS.project as Band], devcontainer: [{ id: 'containers', plugin: 'devcontainer', priority: 'normal', since: 5 }] }
  await pushed(w, $)
  expect(await moreText($)).toBeUndefined()
})

// ------------------------------------------------------------------ independent options (FR-007)

test('guardUx off leaves the CI band working', { options: { guardUx: false } }, async ($, on) => {
  const w = world(on)
  await pushed(w, $)
  expect(await band($)).toBe('CI for 3a9f1c2 on dev: Docs CI ⏳ · check ✓')
})

test('ciBand off leaves the guard presentation working', { options: { ciBand: false } }, async ($, on) => {
  const w = world(on)
  on('classic.PreToolUse', () => ({ deny: 'PreToolUse:Bash hook error: repo guard: hooks must not be bypassed' }))
  const ran = await $.tool.call({ tool: 'Bash', command: 'git push --no-verify' } as never)
  expect(ran.isError).toBe(true)
  expect(w.toasts.some(t => t.includes('repo guard refused this'))).toBe(true)
})
