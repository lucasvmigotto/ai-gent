import { expect, test } from 'claude-code/testing'

import { cases as shared, patterns } from './secret-data'
import { compile } from './secret-mask'
import { SCAN_LIMIT, createLimiter, redactTools, scanContent } from './output-warning'

const { vars } = shared

type Handlers = Parameters<Parameters<typeof test>[1]>[1]

// Credential shapes are put together here, so no full token is committed.
function expand(text: string): string {
  return text.replace(/\{(\w+)\}/g, (_, name: string) => {
    const v = vars[name as keyof typeof vars]
    return v.parts.join('') + v.fill.repeat(v.times)
  })
}

// One fixture per high-confidence kind, with the kind it must be counted as.
const FIXTURES: { kind: string; text: string }[] = [
  { kind: 'private-key', text: expand('{PKEY}') },
  { kind: 'github-token', text: expand('{GHP}') },
  { kind: 'github-token', text: expand('{GHPAT}') },
  { kind: 'api-key', text: expand('{SK}') },
  { kind: 'aws-key', text: expand('{AKIA}') },
  { kind: 'slack-token', text: expand('{XOX}') },
  { kind: 'url-credentials', text: `postgres://app:${expand('{PW}')}@db.example/shop` },
]
const SECRETS = FIXTURES.map(f => f.text)
const rules = compile(JSON.stringify(patterns))
const result = (text: string) => [{ type: 'tool_result', tool_use_id: 't1', content: [{ type: 'text', text }] }]
const textOf = (blocks: { content?: unknown }[]) => JSON.stringify(blocks)

// ----------------------------------------------------------------- pure parts

test('each high-confidence kind is counted, and replaced only when asked', () => {
  for (const f of FIXTURES) {
    const asked = scanContent(result(`out ${f.text} end`), rules, true)
    expect(asked.masked.counts, f.kind).toEqual({ [f.kind]: 1 })
    expect(textOf(asked.content as never), f.kind).not.toContain(f.text.slice(4, 20))
    const kept = scanContent(result(`out ${f.text} end`), rules, false)
    expect(kept.masked.counts, f.kind).toEqual({ [f.kind]: 1 })
    expect(kept.content, f.kind).toEqual(result(`out ${f.text} end`))
  }
})

test('text blocks and a plain string result are scanned; media is untouched', () => {
  const ghp = expand('{GHP}')
  const row = [
    { type: 'text', text: `a ${ghp}` },
    { type: 'tool_result', tool_use_id: 't2', content: `b ${ghp}` },
    { type: 'tool_result', tool_use_id: 't3', content: [{ type: 'image', source: { data: ghp } }] },
  ]
  const scan = scanContent(row, rules, true)
  expect(scan.masked.counts).toEqual({ 'github-token': 2 })
  expect((scan.content[0] as { text: string }).text).toBe('a <token>')
  expect((scan.content[1] as { content: string }).content).toBe('b <token>')
  expect(scan.content[2]).toBe(row[2])
  expect((scan.content[1] as { tool_use_id: string }).tool_use_id).toBe('t2')
})

test('no match returns the very same blocks, and the low-confidence kinds do not count', () => {
  const same = result('plain build output, DB_PASSWORD=<value>, Bearer authentication')
  const scan = scanContent(same, rules, true)
  expect(scan.masked.total).toBe(0)
  expect(scan.content).toBe(same)
})

test('the scan stops after 256 KiB and says so', () => {
  const ghp = expand('{GHP}')
  const long = scanContent(result(`${'x'.repeat(SCAN_LIMIT)}${ghp}`), rules, true)
  expect(long.truncated).toBe(true)
  expect(long.masked.total).toBe(0)
  const short = scanContent(result(`${ghp}${'x'.repeat(SCAN_LIMIT)}`), rules, true)
  expect(short.truncated).toBe(true)
  expect(short.masked.total).toBe(1)
  expect(scanContent(result(ghp), rules, true).truncated).toBe(false)
})

test('the redact list is read from a comma-separated option', () => {
  expect([...redactTools(undefined)]).toEqual(['Bash', 'WebFetch'])
  expect([...redactTools(' Bash , Read ,,')]).toEqual(['Bash', 'Read'])
  expect([...redactTools('')]).toEqual([])
})

test('the limiter allows three toasts a minute, then one summary, never a skipped redaction', () => {
  let t = 0
  const limit = createLimiter(() => t)
  expect([1, 2, 3, 4, 5, 6].map(() => limit())).toEqual(['toast', 'toast', 'toast', 'summary', 'skip', 'skip'])
  t = 61_000
  expect(limit()).toBe('toast')
})

// ------------------------------------------------------------------- the hook

function engine(on: Handlers, fsRead: () => unknown = () => ({ value: JSON.stringify(patterns) })) {
  const seen = { rows: [] as { content: unknown[]; is: string }[], toasts: [] as string[], logs: [] as string[] }
  on('fs.read', fsRead as never)
  on('ui.toast', (_, e) => {
    seen.toasts.push(e.text)
    return { value: undefined }
  })
  on('ui.log', (_, e) => {
    seen.logs.push(e.text)
    return { value: undefined }
  })
  // The row as the session stored it is what next(e) resolves to.
  on('session.append', async (_, e, next) => {
    const stored = await next(e)
    if ('message' in stored) seen.rows.push({ content: stored.message.content, is: JSON.stringify(stored.message.content) })
    return stored
  })
  return seen
}

let n = 0
const row = (tool: string, text: string, id = `t${++n}`) =>
  ({
    message: { type: 'user', role: 'user', content: result(text).map(b => ({ ...b, tool_use_id: id })) },
    door: 'tool-result',
    origin: { kind: 'tool', tool },
    uuid: `u${n}`,
  }) as never

const everything = (seen: ReturnType<typeof engine>) => seen.toasts.join('\n') + seen.logs.join('\n')

// One test per tool and kind: the toast rate is per session, so a row each.
for (const tool of ['Bash', 'WebFetch']) {
  for (const f of FIXTURES) {
    test(`a ${tool} result loses its ${f.kind} (${f.text.length} chars), and the person is told kind, count and tool`, async ($, on) => {
      const seen = engine(on)
      await $.session.append(row(tool, `out ${f.text} end`))
      expect(seen.rows.length).toBe(1)
      for (const secret of SECRETS) expect(seen.rows[0]?.is).not.toContain(secret)
      expect(seen.rows[0]?.is).toContain('out ')
      expect(seen.toasts.length).toBe(1)
      expect(seen.toasts[0]).toContain(tool)
      expect(seen.toasts[0]).toContain('×1')
      expect(seen.toasts[0]).toContain('redacted in what Claude reads')
      expect(seen.toasts[0]).toContain('exposed')
      expect(seen.toasts[0]).not.toMatch(/erased|removed/)
      for (const secret of SECRETS) for (const at of [4, 12, secret.length - 10]) expect(everything(seen)).not.toContain(secret.slice(at, at + 6))
    })
  }
}

test('the toast for a GitHub token names its pass entry; other kinds point at secrets:pass', async ($, on) => {
  const seen = engine(on)
  await $.session.append(row('Bash', `${expand('{GHP}')} ${expand('{GHPAT}')} ${expand('{AKIA}')}`))
  expect(seen.toasts.length).toBe(1)
  expect(seen.toasts[0]).toContain('GitHub token ×2')
  expect(seen.toasts[0]).toContain('AWS key ×1')
  expect(seen.toasts[0]).toContain('pass insert ai-gent/github/pat')
  await $.session.append(row('Bash', expand('{AKIA}')))
  expect(seen.toasts[1]).toContain('secrets:pass')
  expect(seen.toasts[1]).not.toContain('pass insert')
})

test('Read, Grep and an MCP tool are warned about and left as they came', async ($, on) => {
  const seen = engine(on)
  for (const tool of ['Read', 'Grep', 'mcp__files__read']) {
    seen.rows.length = seen.toasts.length = 0
    const input = row(tool, `token ${expand('{GHP}')}`)
    await $.session.append(input)
    expect(seen.rows[0]?.content, tool).toEqual((input as { message: { content: unknown[] } }).message.content)
    expect(seen.toasts.length).toBe(1)
    expect(seen.toasts[0]).toContain('not redacted')
    expect(seen.toasts[0]).toContain(tool)
    expect(everything(seen)).not.toContain(expand('{GHP}').slice(0, 8))
  }
})

test('a row with no tool name is looked up by its tool_use_id from the tool.call that made it', async ($, on) => {
  const seen = engine(on)
  on('tool.call', () => ({ result: {} }))
  await $.tool.call({ tool: 'Bash', command: 'cat .env', tool_use_id: 'call-1' } as never)
  await $.session.append(row('unknown', `x ${expand('{GHP}')}`, 'call-1'))
  expect(seen.rows[0]?.is).not.toContain(expand('{GHP}'))
  expect(seen.toasts[0]).toContain('Bash')
  await $.session.append(row('unknown', `x ${expand('{GHP}')}`, 'call-none'))
  expect(seen.rows[1]?.is).toContain(expand('{GHP}'))
  expect(seen.toasts[1]).toContain('unknown')
})

test('silent and untouched: no match, assignments and headers, other doors', async ($, on) => {
  const seen = engine(on)
  const plain = row('Bash', 'ok DB_PASSWORD=<value> and Basic authentication')
  await $.session.append(plain)
  expect(seen.rows[0]?.content).toEqual((plain as { message: { content: unknown[] } }).message.content)
  const prompt = { ...(row('Bash', expand('{GHP}')) as object), door: 'prompt' } as never
  await $.session.append(prompt)
  expect(seen.rows[1]?.is).toContain(expand('{GHP}'))
  expect(seen.toasts).toEqual([])
})

test('six rows are all redacted, with at most three toasts and one summary', async ($, on) => {
  const seen = engine(on)
  for (let i = 0; i < 6; i++) await $.session.append(row('Bash', `x ${expand('{GHP}')}`))
  expect(seen.rows.length).toBe(6)
  for (const r of seen.rows) expect(r.is).not.toContain(expand('{GHP}'))
  expect(seen.toasts.length).toBe(4)
  expect(seen.toasts[3]).toContain('more results')
  expect(everything(seen)).not.toContain(expand('{GHP}').slice(0, 8))
})

test('a large result says it scanned the first 256 KiB', async ($, on) => {
  const seen = engine(on)
  await $.session.append(row('Bash', `${expand('{GHP}')}${'x'.repeat(SCAN_LIMIT)}`))
  expect(seen.toasts[0]).toContain('256 KiB')
})

test('when the scan fails the row is stored as received and the toast is loud', async ($, on) => {
  const seen = engine(on, () => ({ deny: 'ENOENT' }))
  await $.session.append(row('Bash', `x ${expand('{GHP}')}`))
  expect(seen.rows.length).toBe(1)
  expect(seen.rows[0]?.is).toContain(expand('{GHP}'))
  expect(seen.toasts.length).toBe(1)
  expect(seen.toasts[0]).toContain('could not check or redact')
  expect(seen.toasts[0]).toContain('exposed')
  expect(everything(seen)).not.toContain(expand('{GHP}').slice(0, 8))
})

test('secretOutputWarning off leaves rows alone and shows nothing', { options: { secretOutputWarning: false } }, async ($, on) => {
  const seen = engine(on)
  await $.session.append(row('Bash', `x ${expand('{GHP}')}`))
  expect(seen.rows[0]?.is).toContain(expand('{GHP}'))
  expect(seen.toasts).toEqual([])
})

test('secretOutputRedact off is warn-only', { options: { secretOutputRedact: false } }, async ($, on) => {
  const seen = engine(on)
  await $.session.append(row('Bash', `x ${expand('{GHP}')}`))
  expect(seen.rows[0]?.is).toContain(expand('{GHP}'))
  expect(seen.toasts[0]).toContain('not redacted')
})

test('secretOutputTools picks the tools that are redacted', { options: { secretOutputTools: 'Read' } }, async ($, on) => {
  const seen = engine(on)
  await $.session.append(row('Read', `x ${expand('{GHP}')}`))
  await $.session.append(row('Bash', `x ${expand('{GHP}')}`))
  expect(seen.rows[0]?.is).not.toContain(expand('{GHP}'))
  expect(seen.rows[1]?.is).toContain(expand('{GHP}'))
})

test('the output warning works with secretMask off', { options: { secretMask: false } }, async ($, on) => {
  const seen = engine(on)
  await $.session.append(row('Bash', `x ${expand('{GHP}')}`))
  expect(seen.rows[0]?.is).not.toContain(expand('{GHP}'))
})
