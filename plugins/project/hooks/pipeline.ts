// The pipeline pane's reading and polling: everything that needs `$`. It reads
// with $.fs (and `git log -1` for staleness), writes nothing, runs no stage,
// and fetches nothing. The model and the view are in pipeline-model.ts and
// pipeline-view.ts; this file only feeds them.

import { atom, read, update } from 'claude-code'
import type { EngineInterface, Timer } from 'claude-code'

import type { ArtifactState, Feature, Marker, Snapshot } from '../types'
import { ARTIFACTS, MARKER_DIRS, README, featureState, parseReadme, recommend, scanMarkers, split } from './pipeline-model'
import type { Row } from './pipeline-model'
import { repoRoot } from './repo'

export const PANE = 'pipeline'

// Change the tag when Snapshot's fields change: it declines what an earlier
// version stored, which outlives a reload.
export const snapshotAtom = atom({ plugin: 'project', key: 'pipelineSnapshot' } as const, null, {
  shape: 'pipeline-1',
})
export const isOpenAtom = atom({ plugin: 'project', key: 'pipelineIsOpen' } as const, false)
export const expandedAtom = atom({ plugin: 'project', key: 'pipelineExpanded' } as const, {})
export const filterAtom = atom({ plugin: 'project', key: 'pipelineFilter' } as const, 'all')
// The badge's evidence, written by the poll (never read from the clock while drawing).
export const checkedAtAtom = atom({ plugin: 'project', key: 'pipelineCheckedAt' } as const, 0)
export const pollMsAtom = atom({ plugin: 'project', key: 'pipelinePollMs' } as const, 0)

// Marker scanning stops here; the pane says so.
const MAX_FILES = 300
const MAX_BYTES = 1_500_000
const MAX_ITEMS = 200

// What was read, by modification time: an idle poll lists and stats, and reads
// or spawns only what changed. A reload empties these; the next refresh refills.
const markersByFile = new Map<string, { mtimeMs: number; markers: Marker[] }>()
const dateByPath = new Map<string, { mtimeMs: number; date: string | null }>()
let readme: { mtimeMs: number; rows: Row[] } | null = null
let hasGit = true
let ticker: Timer | null = null
let intervalMs = 0

async function commitDate($: EngineInterface, root: string, path: string, mtimeMs: number): Promise<string | null> {
  const known = dateByPath.get(path)
  if (known && known.mtimeMs === mtimeMs) return known.date
  const run = await $.process
    .run(['git', '-C', root, 'log', '-1', '--format=%cs', '--', path], { timeoutMs: 5000 })
    .catch(() => undefined)
  if (run === undefined || run.exitCode !== 0) hasGit = false
  const date = run?.exitCode === 0 && run.stdout.trim() ? run.stdout.trim() : null
  dateByPath.set(path, { mtimeMs, date })
  return date
}

// Every .md under the marker folders, with its size and time, up to the caps.
async function listMarkdown($: EngineInterface, root: string, dir: string, out: { path: string; size: number; mtimeMs: number }[]): Promise<void> {
  const entries = await $.fs.list(`${root}/${dir}`).catch(() => [])
  for (const entry of [...entries].sort((a, b) => a.name.localeCompare(b.name))) {
    if (out.length >= MAX_FILES) return
    if (entry.kind === 'dir') await listMarkdown($, root, `${dir}/${entry.name}`, out)
    else if (entry.kind === 'file' && entry.name.endsWith('.md')) out.push({ path: `${dir}/${entry.name}`, size: entry.size, mtimeMs: entry.mtimeMs })
  }
}

async function readMarkers($: EngineInterface, root: string): Promise<Snapshot['markers']> {
  const files: { path: string; size: number; mtimeMs: number }[] = []
  for (const dir of MARKER_DIRS) await listMarkdown($, root, dir, files)
  const live = new Set<string>()
  let bytes = 0
  let isTruncated = files.length >= MAX_FILES
  const all: Marker[] = []
  for (const file of files) {
    bytes += file.size
    if (bytes > MAX_BYTES) {
      isTruncated = true
      break
    }
    live.add(file.path)
    let entry = markersByFile.get(file.path)
    if (!entry || entry.mtimeMs !== file.mtimeMs) {
      const text = await $.fs.read(`${root}/${file.path}`).catch(() => '')
      entry = { mtimeMs: file.mtimeMs, markers: scanMarkers(file.path, String(text)) }
      markersByFile.set(file.path, entry)
    }
    all.push(...entry.markers)
  }
  for (const path of markersByFile.keys()) if (!live.has(path)) markersByFile.delete(path)
  const counts = { gap: 0, clarification: 0, assumption: 0 }
  for (const m of all) counts[m.kind] += 1
  const rank = { clarification: 0, gap: 1, assumption: 2 }
  const items = [...all].sort((a, b) => rank[a.kind] - rank[b.kind]).slice(0, MAX_ITEMS)
  return { counts, items, isTruncated }
}

async function readFeatures($: EngineInterface, root: string, rows: readonly Row[]): Promise<Feature[]> {
  const split_ = split(rows)
  const dirs = new Map<string, string>()
  for (const entry of await $.fs.list(`${root}/specs`).catch(() => [])) {
    const number = /^(\d{3})-/.exec(entry.name)?.[1]
    if (entry.kind === 'dir' && number && !dirs.has(number)) dirs.set(number, entry.name)
  }
  const features: Feature[] = []
  for (const row of rows) {
    const dir = dirs.get(row.number)
    const names = dir ? new Set((await $.fs.list(`${root}/specs/${dir}`).catch(() => [])).map(e => e.name)) : new Set<string>()
    features.push({
      number: row.number,
      name: row.name,
      priority: row.priority,
      depends: row.depends,
      sides: row.sides,
      state: featureState(row.sides, row.notes),
      bucket: split_.done.includes(row.number) ? 'done' : split_.blocked.includes(row.number) ? 'blocked' : 'ready',
      waiting: split_.waiting[row.number] ?? [],
      files: { spec: names.has('spec.md'), plan: names.has('plan.md'), tasks: names.has('tasks.md') },
    })
  }
  return features
}

export async function compute($: EngineInterface): Promise<Snapshot> {
  const root = await repoRoot($)
  hasGit = true

  const stats = new Map<string, number>()
  for (const spec of ARTIFACTS) {
    const stat = await $.fs.stat(`${root}/${spec.path}`).catch(() => undefined)
    if (stat?.kind === 'file') stats.set(spec.path, stat.mtimeMs)
  }
  const dates = new Map<string, string | null>()
  for (const [path, mtimeMs] of stats) dates.set(path, await commitDate($, root, path, mtimeMs))

  const artifacts: ArtifactState[] = ARTIFACTS.map(spec => {
    const isPresent = stats.has(spec.path)
    const own = dates.get(spec.path) ?? null
    const newer = spec.after
      .map(path => ({ path, date: dates.get(path) ?? null }))
      .filter((up): up is { path: string; date: string } => own !== null && up.date !== null && up.date > own)
      .sort((a, b) => b.date.localeCompare(a.date))[0]
    return {
      path: spec.path,
      label: spec.label,
      stage: spec.stage,
      isPresent,
      commitDate: own,
      staleSince: isPresent && newer ? newer : null,
      isProblem: !isPresent && spec.neededBy.some(path => stats.has(path)),
    }
  })

  let rows: Row[] = []
  const readmeTime = stats.get(README)
  if (readmeTime !== undefined) {
    if (readme?.mtimeMs !== readmeTime) {
      readme = { mtimeMs: readmeTime, rows: parseReadme(String(await $.fs.read(`${root}/${README}`).catch(() => ''))) }
    }
    rows = readme.rows
  } else {
    readme = null
  }
  const features = await readFeatures($, root, rows)
  const markers = await readMarkers($, root)

  return {
    artifacts,
    features,
    hasReadme: readmeTime !== undefined,
    markers,
    next: recommend(artifacts, features, markers.items),
    hasGit,
    computedAt: await $.clock.now(),
  }
}

// Reads the files and, when anything but the time changed, redraws the pane.
export async function refresh($: EngineInterface): Promise<Snapshot> {
  const snapshot = await compute($)
  // A check ran: the badge's evidence. It advances even when nothing changed.
  await update($, checkedAtAtom, () => snapshot.computedAt)
  await update($, pollMsAtom, () => (ticker === null ? 0 : intervalMs))
  const before = await read($, snapshotAtom)
  const same = before !== null && JSON.stringify({ ...before, computedAt: 0 }) === JSON.stringify({ ...snapshot, computedAt: 0 })
  if (!same) await update($, snapshotAtom, () => snapshot)
  return snapshot
}

export async function tick($: EngineInterface): Promise<void> {
  if (!(await read($, isOpenAtom))) {
    await stop($)
    return
  }
  // A check that fails is not a check: the badge reads STALE until one succeeds.
  await refresh($).catch(() => update($, pollMsAtom, () => 0))
}

export function start($: EngineInterface, seconds: number): void {
  if (ticker !== null) return
  intervalMs = Math.max(1, seconds) * 1000
  ticker = $.clock.every(intervalMs, () => void tick($))
}

export async function stop($: EngineInterface): Promise<void> {
  ticker?.cancel()
  ticker = null
  await update($, pollMsAtom, () => 0)
}

// Opens the pane (or focuses it) and starts reading. `isPlaced` says whether a
// surface draws it; headless or too narrow, the command prints the text.
export async function open($: EngineInterface, seconds: number, isAsked: boolean): Promise<boolean> {
  const placed = await $.ui.open({ id: PANE, title: 'Pipeline', ...(isAsked ? { focus: true as const } : {}) })
  await update($, isOpenAtom, () => true)
  start($, seconds)
  await refresh($).catch(() => undefined)
  return placed.isPlaced
}

export async function close($: EngineInterface): Promise<void> {
  await stop($)
  await update($, isOpenAtom, () => false)
}

// A pane button: show or hide the markers, put /project:status (or the
// command the recommendation names) in the prompt, set the matrix filter or
// open a row. It never submits: the person decides to run anything.
export async function press($: EngineInterface, id: 'markers' | 'status' | 'fill' | 'filter' | 'toggle', arg = ''): Promise<void> {
  if (id === 'status') {
    await $.prompt.fill({ text: '/project:status' })
    return
  }
  if (id === 'fill') {
    if (arg.startsWith('/')) await $.prompt.fill({ text: arg })
    return
  }
  if (id === 'filter') {
    if (arg === 'all' || arg === 'ready' || arg === 'blocked' || arg === 'done') await update($, filterAtom, () => arg)
    return
  }
  if (id === 'toggle') {
    if (arg !== '') await update($, expandedAtom, current => ({ ...current, [arg]: current[arg] !== true }))
    return
  }
  await update($, expandedAtom, current => ({ ...current, markers: current.markers !== true }))
}

// At the session's start (and after a reload): declare /pipeline, and open the
// pane by itself where a pipeline exists and a surface can dock it, or where it
// was open before the reload.
export async function startPipeline($: EngineInterface, seconds: number, canDock: boolean): Promise<void> {
  await $.command.register({ name: 'pipeline-pane', description: 'Show the pipeline: artifacts, features, open items and the next stage' })
  const hasPipeline = await $.fs.exists(`${await repoRoot($)}/${README}`).catch(() => false)
  if (canDock && ((await read($, isOpenAtom)) || hasPipeline)) await open($, seconds, false)
}

// Reads for register.tsx, which cannot read these atoms itself (the state
// library's scan wants the atom declared in the file that reads it).
export function readSnapshot($: EngineInterface): Promise<Snapshot | null> {
  return read($, snapshotAtom)
}

export function readExpanded($: EngineInterface): Promise<Record<string, boolean>> {
  return read($, expandedAtom)
}

export function readIsOpen($: EngineInterface): Promise<boolean> {
  return read($, isOpenAtom)
}

// What the dashboard needs besides the snapshot, read here because an atom is
// read only in the file that declares it.
export async function readPaneState($: EngineInterface): Promise<{ filter: string; checkedAt: number; pollMs: number }> {
  return { filter: await read($, filterAtom), checkedAt: await read($, checkedAtAtom), pollMs: await read($, pollMsAtom) }
}
