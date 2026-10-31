// The next-stage toast (spec 027): after a pipeline artifact is written, or a
// [NEEDS CLARIFICATION] is added, toast the stage the pipeline pane recommends,
// once per turn and only when the recommendation changed. The recommendation is
// `compute($).next` from pipeline.ts, never a second set of rules. It reads
// with $.fs (through compute), writes nothing and runs no stage. The wording is
// in nextstage-model.ts; this file feeds it.

import { atom, read, update } from 'claude-code'
import type { EngineInterface } from 'claude-code'

import type { Snapshot } from '../types'
import { differs, keyOf, labelOf, toastText } from './nextstage-model'
import { compute, readIsOpen, readSnapshot } from './pipeline'
import { ARTIFACTS } from './pipeline-model'
import { repoRoot } from './repo'

// Change the tag when Next's fields change: it declines what an earlier version
// stored, which outlives a reload.
const lastAtom = atom({ plugin: 'project', key: 'nextStageLast' } as const, null, { shape: 'nextstage-1' })
const shownAtom = atom({ plugin: 'project', key: 'nextStageShown' } as const, [] as string[])
const mtimesAtom = atom({ plugin: 'project', key: 'nextStageMtimes' } as const, {} as Record<string, number>)

export type NextStageOptions = { nextStageToast?: boolean }

const isOn = (options: NextStageOptions): boolean => options.nextStageToast !== false

// What the turn's Edit/Write calls touched, until the turn ends.
let pending: { labels: string[]; at: number } | null = null
// The seed in flight, so a turn that ends before it lands waits for it.
let seeding: Promise<void> | null = null

const isWatched = (path: string): boolean => path.includes('/docs/product/') || path.includes('/specs/')

// The stats of the artifacts, by path; absent ones are left out.
async function statArtifacts($: EngineInterface): Promise<Record<string, number>> {
  const root = await repoRoot($)
  const out: Record<string, number> = {}
  for (const spec of ARTIFACTS) {
    const stat = await $.fs.stat(`${root}/${spec.path}`).catch(() => undefined)
    if (stat?.kind === 'file') out[spec.path] = stat.mtimeMs
  }
  return out
}

// At session start: record the recommendation and the artifact times the first
// change is judged against, detached and without a toast.
export function seedNextStage($: EngineInterface, options: NextStageOptions): void {
  if (!isOn(options)) return
  pending = null
  const run = async (): Promise<void> => {
    const times = await statArtifacts($)
    const snapshot = await compute($)
    await update($, lastAtom, () => snapshot.next)
    await update($, mtimesAtom, () => times)
  }
  seeding = run()
    .catch(() => undefined)
    .finally(() => {
      seeding = null
    })
}

// Before an Edit or Write on a watched path runs: let the seed finish, so it
// reads the tree as it was, and say whether the file is new.
export async function beforeWrite($: EngineInterface, options: NextStageOptions, path: string, isWrite: boolean): Promise<boolean> {
  if (!isOn(options) || !isWatched(path)) return false
  if (seeding !== null) await seeding
  return isWrite ? !(await $.fs.exists(path).catch(() => true)) : false
}

// An Edit or Write on a watched path completed: remember it for the turn's end.
// `isNew` says the file did not exist before the call.
export async function noteWrite($: EngineInterface, options: NextStageOptions, path: string, isNew: boolean): Promise<void> {
  if (!isOn(options) || !isWatched(path)) return
  const label = isNew ? labelOf(path) : null
  pending = {
    labels: [...(pending?.labels ?? []), ...(label !== null ? [label] : [])],
    at: await $.clock.now(),
  }
}

// At turn end: when a watched file was written (by a tool, or by anything else,
// seen by its time) judge the snapshot's `next` against the last one and toast
// it once if it is new.
export async function nextStageAtTurnEnd($: EngineInterface, options: NextStageOptions): Promise<void> {
  if (!isOn(options)) return
  if (seeding !== null) await seeding
  const touched = pending
  pending = null

  const known = await read($, mtimesAtom)
  const times = await statArtifacts($)
  await update($, mtimesAtom, () => times)
  const last = await read($, lastAtom)
  const labels = new Set(touched?.labels ?? [])
  let isChanged = touched !== null
  if (last !== null) {
    for (const spec of ARTIFACTS) {
      if (times[spec.path] === known[spec.path]) continue
      isChanged = true
      if (known[spec.path] === undefined) labels.add(spec.label)
    }
  }
  if (!isChanged) return

  // The open pane's snapshot, when it was taken after the write.
  let snapshot: Snapshot | null = null
  if (touched !== null && (await readIsOpen($))) {
    const open = await readSnapshot($)
    if (open !== null && open.computedAt > touched.at) snapshot = open
  }
  snapshot ??= await compute($)

  const { next } = snapshot
  await update($, lastAtom, () => next)
  if (!differs(last, next)) return
  const shown = await read($, shownAtom)
  if (shown.includes(keyOf(next))) return
  await update($, shownAtom, list => [...list, keyOf(next)])
  $.ui.toast(toastText(next, [...labels]), { timeoutMs: 15_000 })
}
