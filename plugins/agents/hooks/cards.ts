// The card timeline pane's reading (specs/020-card-timeline-pane): everything that
// needs `$`. It reads the ledger with $.fs, polls its `mtimeMs` only while the
// pane is open, and runs `token_report.py tasks --dry-run --json` only when
// asked (Measure or --measure), one run at a time. It writes nothing. The
// logic is in cards-model.ts and the layout in cards-view.ts.

import { atom, read, update } from 'claude-code'
import type { EngineInterface, Timer } from 'claude-code'

import { appendLog, hhmm, parseMeasure, slugOf, transitions } from './cards-model'
import { text as snapshotText } from './cards-view'
import type { Snapshot } from './cards-view'
import { LEDGER, exists } from './ledger'
import { parseLedger } from './token-model'

export const PANE = 'cards'

const openAtom = atom({ plugin: 'agents', key: 'cardsOpen' } as const, false)
const seenAtom = atom({ plugin: 'agents', key: 'cardsSeen' } as const, {})
const logAtom = atom({ plugin: 'agents', key: 'cardsLog' } as const, [])
const rowsAtom = atom({ plugin: 'agents', key: 'cardsRows' } as const, [])
const sinceAtom = atom({ plugin: 'agents', key: 'cardsSince' } as const, null)
// Change the tag when the measurement's fields change: it declines what an earlier version stored.
const measuredAtom = atom({ plugin: 'agents', key: 'cardsMeasured' } as const, null, { shape: 'cards-1' })

const MEASURE_TIMEOUT_MS = 60_000
export const NO_LEDGER = `No orchestration run here: ${LEDGER} is missing.`

// What was last read, by modification time; a reload empties it and the next read refills.
let lastMtime: number | null = null
let isMeasuring = false
let ticker: Timer | null = null

// Reads the ledger when its mtime changed (or `isForced`), logs what differs from
// the states last seen. A baseline read (the pane just opened) logs nothing.
export async function refresh($: EngineInterface, isBaseline = false): Promise<void> {
  const path = `${await $.session.cwd()}/${LEDGER}`
  const stat = await $.fs.stat(path).catch(() => undefined)
  const mtime = stat?.kind === 'file' ? stat.mtimeMs : -1
  if (!isBaseline && mtime === lastMtime) return
  lastMtime = mtime
  const cards = mtime < 0 ? [] : parseLedger(String(await $.fs.read(path).catch(() => '')))
  const now = await $.clock.now()
  const found = transitions(cards, isBaseline ? {} : await read($, seenAtom))
  await update($, seenAtom, () => found.seen)
  if (found.changes.length > 0) await update($, logAtom, log => appendLog(log, found.changes, hhmm(now)))
  if ((await read($, sinceAtom)) === null) await update($, sinceAtom, () => hhmm(now))
  await update($, rowsAtom, () => cards)
}

export async function tick($: EngineInterface): Promise<void> {
  if (!(await read($, openAtom))) {
    stop()
    return
  }
  await refresh($).catch(() => undefined)
}

function startPoll($: EngineInterface, seconds: number): void {
  if (ticker !== null) return
  ticker = $.clock.every(Math.max(1, seconds) * 1000, () => void tick($))
}

export function stop(): void {
  ticker?.cancel()
  ticker = null
}

// Runs the script read-only: `tasks --dry-run --json`, never --rollup-out, no --session.
export async function measure($: EngineInterface): Promise<void> {
  if (isMeasuring) return
  isMeasuring = true
  try {
    await update($, measuredAtom, () => ({ status: 'running' as const, error: null, spans: [], at: 0 }))
    const cwd = await $.session.cwd()
    const script = `${$.plugin.root}/scripts/token_report.py`
    const run = await $.process
      .run(['python3', script, 'tasks', '--dry-run', '--json', `--project=${slugOf(cwd)}`], { cwd, timeoutMs: MEASURE_TIMEOUT_MS })
      .catch(() => undefined)
    const spans = run?.exitCode === 0 ? parseMeasure(run.stdout) : null
    const at = await $.clock.now()
    await update($, measuredAtom, () =>
      spans !== null
        ? { status: 'done' as const, error: null, spans, at }
        : {
            status: 'failed' as const,
            error: run === undefined || run.exitCode !== 0 ? `token_report.py tasks did not finish (${MEASURE_TIMEOUT_MS / 1000} s limit, or it failed)` : 'its output was not the expected JSON',
            spans: [],
            at,
          },
    )
  } finally {
    isMeasuring = false
  }
}

export async function snapshot($: EngineInterface): Promise<Snapshot> {
  return { cards: await read($, rowsAtom), log: await read($, logAtom), since: await read($, sinceAtom), measure: await read($, measuredAtom) }
}

// `/cards-pane [--measure]`: opens the pane (focused) and starts the poll, or, where
// no surface draws it, answers with the text form. Nothing opens without it.
export async function command($: EngineInterface, seconds: number, args: string): Promise<{ text: string }> {
  const cwd = await $.session.cwd()
  if (!(await exists($, `${cwd}/${LEDGER}`))) return { text: NO_LEDGER }
  const wantsMeasure = /(^|\s)--measure(\s|$)/.test(args)
  const wasOpen = await read($, openAtom)

  const placed = await $.ui.open({ id: PANE, title: 'Cards', focus: true }).catch(() => ({ isPlaced: false }))
  await refresh($, !wasOpen)
  if (placed.isPlaced) {
    await update($, openAtom, () => true)
    startPoll($, seconds)
    if (wantsMeasure) $.clock.after(0, () => void measure($).catch(() => undefined))
    return { text: 'Cards pane opened.' }
  }
  await update($, openAtom, () => false)
  stop()
  if (wantsMeasure) await measure($)
  return { text: snapshotText(await snapshot($)) }
}

export async function close($: EngineInterface): Promise<void> {
  stop()
  await update($, openAtom, () => false)
}

// At the session's start: forget what was read, and poll again if the pane was open before a reload.
export async function resume($: EngineInterface, seconds: number): Promise<void> {
  // A session's start is a fresh look: nothing read before is remembered.
  stop()
  lastMtime = null
  isMeasuring = false
  if (await read($, openAtom)) startPoll($, seconds)
}
