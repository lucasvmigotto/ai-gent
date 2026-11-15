// The workers pane's reading and polling (spec 041): everything that needs `$`. It reads
// `.ai-gent/delegate/<track>/status.json`, `.ai-gent/delegation.log` and the cap line of
// `.ai-gent/parallel.yaml` with $.fs, writes nothing but its own atoms and runs nothing.
// It never reads a track's launch, event, stderr or brief files. The model is workers-model.ts.

import { atom, read, update } from 'claude-code'
import type { EngineInterface, Timer } from 'claude-code'

import type { WorkerLogLine, WorkerStatus, WorkersSnapshot } from '../types'
import { MAX_STATUS_BYTES, agentRow, parseCap, parseLog, parseStatus } from './workers-model'
import { mainRoot } from './repo'

export const PANE = 'workers'

const MAX_TRACKS = 100
const MAX_LOG_BYTES = 2_000_000

// Change the tag when WorkersSnapshot's fields change: it declines what an earlier version stored.
export const snapshotAtom = atom({ plugin: 'project', key: 'workersSnapshot' } as const, null, { shape: 'workers-3' })
export const isOpenAtom = atom({ plugin: 'project', key: 'workersIsOpen' } as const, false)
export const expandedAtom = atom({ plugin: 'project', key: 'workersExpanded' } as const, {})
export const pollMsAtom = atom({ plugin: 'project', key: 'workersPollMs' } as const, 0)

// Read by modification time: an idle poll lists and stats, and reads only what changed.
const statusByTrack = new Map<string, { mtimeMs: number; status: WorkerStatus | null }>()
let capCache: { mtimeMs: number; cap: number | null } | null = null
let logCache: { mtimeMs: number; lines: WorkerLogLine[] } | null = null
let ticker: Timer | null = null
// Set by register.tsx from the workersShowDescription option (default off, FR-017).
let showDescription = false
export const configure = (options: { showDescription: boolean }): void => {
  showDescription = options.showDescription
}
// The API gives a subagent no clock: when this poll first saw it, and when its status last changed (epoch seconds).
const agentSeen = new Map<string, { first: number; changed: number; status: string }>()
let intervalMs = 0

export async function compute($: EngineInterface): Promise<WorkersSnapshot> {
  const root = await mainRoot($)
  const base = `${root}/.ai-gent`
  const workers: WorkerStatus[] = []
  let skipped = 0
  const live = new Set<string>()
  const entries = await $.fs.list(`${base}/delegate`).catch(() => [])
  for (const entry of [...entries].sort((a, b) => a.name.localeCompare(b.name)).slice(0, MAX_TRACKS)) {
    if (entry.kind !== 'dir') continue
    const path = `${base}/delegate/${entry.name}/status.json`
    const stat = await $.fs.stat(path).catch(() => undefined)
    if (stat?.kind !== 'file') continue
    live.add(entry.name)
    let known = statusByTrack.get(entry.name)
    if (!known || known.mtimeMs !== stat.mtimeMs) {
      const text = stat.size > MAX_STATUS_BYTES ? '' : String(await $.fs.read(path).catch(() => ''))
      const status = parseStatus(text)
      // A file that names another track is not this track's.
      known = { mtimeMs: stat.mtimeMs, status: status !== null && status.track === entry.name ? status : null }
      statusByTrack.set(entry.name, known)
    }
    if (known.status === null) skipped += 1
    else workers.push(known.status)
  }
  for (const name of statusByTrack.keys()) if (!live.has(name)) statusByTrack.delete(name)

  // Kind 1: the session's own subagents, read from the API; kinds 2 and 3 are the status files above.
  const nowS = Math.floor((await $.clock.now()) / 1000)
  const agents = await $.agent.list().catch(() => [])
  const present = new Set<string>()
  for (const info of agents.slice(0, MAX_TRACKS)) {
    present.add(info.id)
    const known = agentSeen.get(info.id)
    const seen = known === undefined ? { first: nowS, changed: nowS, status: info.status } : known.status === info.status ? known : { ...known, changed: nowS, status: info.status }
    agentSeen.set(info.id, seen)
    workers.push(agentRow(info, seen, showDescription))
  }
  for (const id of agentSeen.keys()) if (!present.has(id)) agentSeen.delete(id)

  const logStat = await $.fs.stat(`${base}/delegation.log`).catch(() => undefined)
  if (logStat?.kind !== 'file') logCache = null
  else if (logCache?.mtimeMs !== logStat.mtimeMs) {
    const text = logStat.size > MAX_LOG_BYTES ? '' : String(await $.fs.read(`${base}/delegation.log`).catch(() => ''))
    logCache = { mtimeMs: logStat.mtimeMs, lines: parseLog(text) }
  }

  const capStat = await $.fs.stat(`${base}/parallel.yaml`).catch(() => undefined)
  if (capStat?.kind !== 'file') capCache = null
  else if (capCache?.mtimeMs !== capStat.mtimeMs) {
    const yaml = capStat.size > MAX_STATUS_BYTES ? null : await $.fs.read(`${base}/parallel.yaml`).catch(() => null)
    capCache = { mtimeMs: capStat.mtimeMs, cap: yaml === null ? null : parseCap(String(yaml)) }
  }
  return {
    workers,
    log: logCache?.lines ?? [],
    cap: capCache?.cap ?? null,
    skipped,
    computedAt: await $.clock.now(),
  }
}

// Reads the files and records the snapshot when anything but the time changed.
export async function refresh($: EngineInterface): Promise<WorkersSnapshot> {
  const snapshot = await compute($)
  await update($, pollMsAtom, () => (ticker === null ? 0 : intervalMs))
  const before = await read($, snapshotAtom)
  const same = before !== null && JSON.stringify({ ...before, computedAt: 0 }) === JSON.stringify({ ...snapshot, computedAt: 0 })
  // The clock still advances an elapsed time, so a snapshot with live workers is recorded each check.
  const isLive = snapshot.workers.some(w => w.state === 'running' || w.state === 'stalled')
  if (!same || isLive) await update($, snapshotAtom, () => snapshot)
  return snapshot
}

export async function tick($: EngineInterface): Promise<void> {
  if (!(await read($, isOpenAtom))) {
    await stop($)
    return
  }
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

// Opens the pane (or focuses it) and starts reading. False where no surface docks it.
export async function open($: EngineInterface, seconds: number): Promise<boolean> {
  const placed = await $.ui.open({ id: PANE, title: 'Workers', focus: true })
  await update($, isOpenAtom, () => true)
  start($, seconds)
  await refresh($).catch(() => undefined)
  return placed.isPlaced
}

export async function close($: EngineInterface): Promise<void> {
  await stop($)
  await update($, isOpenAtom, () => false)
}

// A row's button: open or close its detail. It never runs anything.
export async function toggle($: EngineInterface, track: string): Promise<void> {
  await update($, expandedAtom, current => ({ ...current, [track]: current[track] !== true }))
}

// Reads for register.tsx (an atom is read only in the file that declares it).
export function readSnapshot($: EngineInterface): Promise<WorkersSnapshot | null> {
  return read($, snapshotAtom)
}

export function readExpanded($: EngineInterface): Promise<Record<string, boolean>> {
  return read($, expandedAtom)
}
