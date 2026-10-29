// The repo-hygiene features' reading: everything that needs `$`. It runs only
// read-only git commands (never fetch, pull or a ref change), reads
// FETCH_HEAD's time, and relays what the update-check hook already printed.
// The wording is in hygiene-model.ts; this file feeds it.

import { atom, read, update } from 'claude-code'
import type { EngineInterface } from 'claude-code'

import type { BranchState, Hygiene } from '../types'
import { hookLines, rowText, updateLine } from './hook-lines'
import { CHAIN, branchesWithRemote, parseCounts, releasesIn } from './hygiene-model'

// Change the tag when Hygiene's fields change: it declines what an earlier
// version stored, which outlives a reload.
const hygieneAtom = atom({ plugin: 'git', key: 'hygiene' } as const, null, { shape: 'hygiene-1' })
const toastedAtom = atom({ plugin: 'git', key: 'toasted' } as const, {})

// The comparison is read again at most this often between turns.
const THROTTLE_MS = 60_000

async function git($: EngineInterface, args: readonly string[]): Promise<string | null> {
  const run = await $.process.run(['git', ...args], { timeoutMs: 10_000 }).catch(() => undefined)
  return run?.exitCode === 0 ? run.stdout.trim() : null
}

// Local long-lived branches against the remote-tracking branches already there.
async function compare($: EngineInterface): Promise<Hygiene | null> {
  const common = await git($, ['rev-parse', '--path-format=absolute', '--git-common-dir'])
  if (common === null) return null
  const refs = await git($, [
    'for-each-ref',
    '--format=%(refname)',
    ...CHAIN.map(name => `refs/heads/${name}`),
    ...CHAIN.map(name => `refs/remotes/origin/${name}`),
  ])
  const branches: BranchState[] = []
  for (const name of branchesWithRemote(refs ?? '')) {
    const counts = parseCounts((await git($, ['rev-list', '--left-right', '--count', `${name}...origin/${name}`])) ?? '')
    if (counts === null) continue
    const subjects = counts.behind > 0 ? ((await git($, ['log', '--format=%s', `${name}..origin/${name}`])) ?? '').split('\n') : []
    branches.push({ name, ...counts, releases: releasesIn(name, subjects) })
  }
  const fetched = await $.fs.stat(`${common}/FETCH_HEAD`).catch(() => undefined)
  return { branches, fetchedAt: fetched?.mtimeMs ?? null, readAt: await $.clock.now() }
}

export async function refreshHygiene($: EngineInterface): Promise<void> {
  const next = await compare($)
  const before = await read($, hygieneAtom)
  const same = JSON.stringify(before?.branches) === JSON.stringify(next?.branches) && before?.fetchedAt === next?.fetchedAt
  if (!same || before === null) await update($, hygieneAtom, () => next)
}

// After a turn: read again only if the last read is a minute old.
export async function refreshHygieneThrottled($: EngineInterface): Promise<void> {
  const before = await read($, hygieneAtom)
  if (before !== null && (await $.clock.now()) - before.readAt < THROTTLE_MS) return
  await refreshHygiene($)
}

export async function readHygiene($: EngineInterface): Promise<Hygiene | null> {
  return read($, hygieneAtom)
}

// ----------------------------------------------------------- session notices

// update-check.sh's line was only in the model's context; the person sees it
// once, as a toast. The script's own words, never a check of the mod's own.
export async function relayUpdate($: EngineInterface, message: unknown): Promise<void> {
  const line = updateLine(hookLines(rowText(message)))
  if (line === null || (await read($, toastedAtom)).update === true) return
  await update($, toastedAtom, current => ({ ...current, update: true }))
  $.ui.toast(line, { timeoutMs: 12_000 })
}

// ----------------------------------------------------------- /release-preview

export async function releasePreview($: EngineInterface): Promise<string> {
  const top = await git($, ['rev-parse', '--show-toplevel'])
  if (top === null) return 'Not a git repository.'
  const script = `${top}/scripts/release.py`
  if (!(await $.fs.exists(script).catch(() => false))) {
    return 'No scripts/release.py in this repository. devsecops:pipeline sets one up from shared/release-single.py.'
  }
  // Always --dry-run: the preview never releases.
  const run = await $.process.run(['python3', script, '--dry-run'], { cwd: top, timeoutMs: 60_000 }).catch(() => undefined)
  if (run === undefined) return 'scripts/release.py could not be started (is python3 installed?).'
  if (run.exitCode === 0) return run.stdout.trimEnd()
  if (run.exitCode === 3) return 'No release needed (nothing releasable since the last tag).'
  return `scripts/release.py --dry-run failed (exit ${run.exitCode}): ${(run.stderr || run.stdout).trim()}`
}
