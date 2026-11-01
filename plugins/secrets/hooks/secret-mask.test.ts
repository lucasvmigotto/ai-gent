import { expect, test } from 'claude-code/testing'

import { cases as shared, patterns } from './secret-data'
import { compile, looksSecret, mask, warning } from './secret-mask'

const { cases, vars } = shared

type Handlers = Parameters<Parameters<typeof test>[1]>[1]

// Credential shapes are put together here, so no full token is committed.
function expand(text: string): string {
  return text.replace(/\{(\w+)\}/g, (_, name: string) => {
    const v = vars[name as keyof typeof vars]
    return v.parts.join('') + v.fill.repeat(v.times)
  })
}

const GHP = expand('{GHP}')
const NONE: ReadonlySet<string> = new Set()

// Nothing sits beneath the plugin: these hooks stand for the engine. They
// record what reached it, and what the person was shown.
function engine(on: Handlers, fsRead: () => unknown = () => ({ value: JSON.stringify(patterns) })) {
  const seen = { prompt: [] as string[], args: [] as string[], skill: [] as string[], toasts: [] as string[], logs: [] as string[] }
  on('fs.read', fsRead as never)
  on('ui.toast', (_, e) => {
    seen.toasts.push(e.text)
    return { value: undefined }
  })
  on('ui.log', (_, e) => {
    seen.logs.push(e.text)
    return { value: undefined }
  })
  on('prompt.submit', (_, e) => {
    seen.prompt.push(e.text)
    return { text: e.text }
  })
  on('command.run', (_, e) => {
    seen.args.push(e.args)
    return { text: 'ran' }
  })
  on('skill.prompt', (_, e) => {
    seen.skill.push(e.text)
    return { text: e.text }
  })
  return seen
}

const ORIGIN = { kind: 'user' } as never

test('every shared case masks the same in the pure masker', () => {
  const rules = compile(JSON.stringify(patterns))
  for (const c of cases) {
    const masked = mask(expand(c.input), rules, 'skill' in c && c.skill ? 'skill' : 'prompt', NONE)
    expect(masked.text, c.name).toBe(expand(c.expected))
    expect(masked.counts, c.name).toEqual(c.kinds)
  }
})

test('a prompt is masked on the way down, and the person is told what, not the value', async ($, on) => {
  const seen = engine(on)
  for (const c of cases.filter(c => !('skill' in c && c.skill))) {
    seen.prompt.length = seen.toasts.length = 0
    await $.prompt.submit({ text: expand(c.input), wait: false, origin: ORIGIN })
    expect(seen.prompt, c.name).toEqual([expand(c.expected)])
    expect(seen.toasts.length, c.name).toBe(Object.keys(c.kinds).length > 0 ? 1 : 0)
    for (const secret of [GHP, expand('{PW}'), expand('{SK}'), expand('{JWT}')]) {
      expect(seen.toasts.join('\n') + seen.logs.join('\n')).not.toContain(secret)
    }
  }
})

test('the toast names the kind and where the value belongs', async ($, on) => {
  const seen = engine(on)
  await $.prompt.submit({ text: `use ${GHP}`, wait: false, origin: ORIGIN })
  expect(seen.toasts[0]).toContain('GitHub token ×1')
  expect(seen.toasts[0]).toContain('pass insert ai-gent/github/pat')
})

test('a slash command line is left to command.run, which masks its arguments', async ($, on) => {
  const seen = engine(on)
  await $.prompt.submit({ text: `/db:connect SHOP password=${expand('{PW}')}`, command: { name: 'db:connect', args: '' }, wait: false, origin: ORIGIN } as never)
  expect(seen.prompt).toEqual([`/db:connect SHOP password=${expand('{PW}')}`])

  await $.command.run({ command: 'db:connect', args: `SHOP password=${expand('{PW}')}`, origin: ORIGIN } as never)
  expect(seen.args).toEqual(['SHOP password=<redacted>'])
})

test('a hidden prompt that starts its own turn takes no rewrite; one delivered into a running turn does', async ($, on) => {
  const seen = engine(on)
  await $.prompt.submit({ text: `token ${GHP}`, isMeta: true, wait: false, origin: ORIGIN })
  await $.prompt.submit({ text: `token ${GHP}`, isMeta: true, turnId: 't1', wait: false, origin: ORIGIN })
  expect(seen.prompt).toEqual([`token ${GHP}`, 'token <token>'])
  expect(seen.logs.some(line => line.includes('not masked'))).toBe(true)
})

test('a skill body loses its tokens, and keeps prose and examples', async ($, on) => {
  const seen = engine(on)
  for (const c of cases.filter(c => 'skill' in c && c.skill)) {
    seen.skill.length = 0
    await $.skill.prompt({ skill: 'x', text: expand(c.input) } as never)
    expect(seen.skill, c.name).toEqual([expand(c.expected)])
  }
})

test('maskAssignments off keeps NAME=value text and still masks the rest', { options: { maskAssignments: false } }, async ($, on) => {
  const seen = engine(on)
  await $.prompt.submit({ text: `DB_PASSWORD=${expand('{PW}')} ${GHP}`, wait: false, origin: ORIGIN })
  expect(seen.prompt).toEqual([`DB_PASSWORD=${expand('{PW}')} <token>`])
})

test('secretMask off registers nothing', { options: { secretMask: false } }, async ($, on) => {
  const seen = engine(on)
  await $.prompt.submit({ text: `token ${GHP}`, wait: false, origin: ORIGIN })
  expect(seen.prompt).toEqual([`token ${GHP}`])
  expect(seen.toasts).toEqual([])
})

test('when the patterns cannot be read, a prompt that looks secret is held back and any other goes through', async ($, on) => {
  const seen = engine(on, () => ({ deny: 'ENOENT' }))
  const held = await $.prompt.submit({ text: `token ${GHP}`, wait: false, origin: ORIGIN })
  expect(held).toEqual({ drop: expect.stringContaining('was not sent') })
  const sent = await $.prompt.submit({ text: 'run the tests', wait: false, origin: ORIGIN })
  expect(sent).toEqual({ text: 'run the tests' })
  expect(seen.prompt).toEqual(['run the tests'])

  const command = await $.command.run({ command: 'x', args: `token=${GHP}`, origin: ORIGIN } as never)
  expect(command.text).toContain('was not sent')
  expect(seen.args).toEqual([])
})

test('a failed read is tried again on the next prompt', async ($, on) => {
  let reads = 0
  const seen = engine(on, () => (++reads === 1 ? { deny: 'ENOENT' } : { value: JSON.stringify(patterns) }))
  await $.prompt.submit({ text: `token ${GHP}`, wait: false, origin: ORIGIN })
  await $.prompt.submit({ text: `token ${GHP}`, wait: false, origin: ORIGIN })
  expect(seen.prompt).toEqual(['token <token>'])
})

test('looksSecret sees keys, headers and assignments, and not plain prose', () => {
  expect(looksSecret(`x ${GHP}`, 'prompt')).toBe(true)
  expect(looksSecret('db_password = abc', 'prompt')).toBe(true)
  expect(looksSecret('db_password = abc', 'skill')).toBe(false)
  expect(looksSecret('please run the build and show the diff', 'prompt')).toBe(false)
})

test('the result scope leaves out assignments and auth headers, and its pre-filter sees only key shapes', () => {
  const rules = compile(JSON.stringify(patterns))
  const text = `API_TOKEN=plainvalue and Bearer abcdefghijklmnop and ${GHP}`
  const masked = mask(text, rules, 'result', NONE)
  expect(Object.keys(masked.counts)).toEqual(['github-token'])
  expect(masked.text).toContain('API_TOKEN=plainvalue')
  expect(looksSecret(`x ${GHP}`, 'result')).toBe(true)
  expect(looksSecret('postgres://u:p@h/db', 'result')).toBe(true)
  expect(looksSecret('db_password = abc', 'result')).toBe(false)
  expect(looksSecret('plain build output', 'result')).toBe(false)
})

test('the warning names kinds and counts, says what happened, and holds no value', () => {
  const rules = compile(JSON.stringify(patterns))
  const masked = mask(`a ${GHP} b ${GHP} c postgres://app:hunter2pw@db/x`, rules, 'result', NONE)
  for (const redacted of [true, false]) {
    const text = warning(masked, 'Bash', redacted)
    expect(text).toContain('Bash')
    expect(text).toContain('GitHub token ×2')
    expect(text).toContain('URL password ×1')
    expect(text).not.toContain(GHP)
    expect(text).not.toContain(GHP.slice(0, 8))
    expect(text).not.toContain('hunter2pw')
    expect(text).not.toContain('<token>')
  }
  expect(warning(masked, 'Bash', true)).toContain('redacted in what Claude reads')
  expect(warning(masked, 'Bash', false)).toContain('not redacted')
  for (const redacted of [true, false]) expect(warning(masked, 'Bash', redacted)).toMatch(/exposed.*rotate/)
  expect(warning(masked, 'Bash', true, true)).toContain('256 KiB')
})
