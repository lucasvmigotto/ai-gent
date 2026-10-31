// The token status' reading: everything that needs `$`. It reads the ledger with
// $.fs and runs token_report.py through $.process.run (the script measures and
// judges; this formats what it says). It writes nothing: `tasks` keeps writing
// the ledger, tasks.md, tracker.md and the history. The wording is in
// token-model.ts; this file feeds it.

import { atom, read, update } from 'claude-code'
import type { EngineInterface } from 'claude-code'

import { etaNews, inFlight, news, parseBudget, parseLedger, statusText } from './token-model'
import type { Budget, LedgerCard } from './token-model'

const lastVerdictAtom = atom({ plugin: 'agents', key: 'tokenLastVerdict' } as const, null)
const toastedAtom = atom({ plugin: 'agents', key: 'tokenToasted' } as const, [])
const etaWarnedAtom = atom({ plugin: 'agents', key: 'tokenEtaWarned' } as const, [])
const refreshedAtAtom = atom({ plugin: 'agents', key: 'tokenRefreshedAt' } as const, 0)

const LEDGER = '.ai-gent/orchestrate-status.md'
const BUDGET = '.ai-gent/budget.yaml'
const WIDTH = 150

// The burn rate's two options (spec 018); register.tsx passes what the user set.
export type TokenOptions = { tokenBurnRate?: boolean; tokenEtaWarnMinutes?: number }

async function exists($: EngineInterface, path: string): Promise<boolean> {
  return $.fs.exists(path).catch(() => false)
}

async function readCards($: EngineInterface, cwd: string): Promise<LedgerCard[]> {
  if (!(await exists($, `${cwd}/${LEDGER}`))) return []
  return parseLedger(String(await $.fs.read(`${cwd}/${LEDGER}`).catch(() => '')))
}

// The script's own arguments for the wave: the cards in flight, or no wave at all
// (`--projected 0`, which still prints what the caps leave).
function waveArgs(flying: readonly LedgerCard[]): string[] {
  return flying.length > 0 ? [`--cards=${flying.map(card => card.id).join(',')}`] : ['--projected=0']
}

async function runBudget($: EngineInterface, cwd: string, args: readonly string[]): Promise<{ ok: boolean; stdout: string }> {
  const script = `${$.plugin.root}/scripts/token_report.py`
  const run = await $.process.run(['python3', script, 'budget', ...args], { cwd, timeoutMs: 120_000 }).catch(() => undefined)
  return { ok: run?.exitCode === 0, stdout: run?.stdout ?? '' }
}

// Reads the ledger and the budget, shows one status line, and toasts what is new.
export async function refresh($: EngineInterface, options: TokenOptions = {}): Promise<void> {
  const withRate = options.tokenBurnRate !== false
  const cwd = await $.session.cwd()
  const cards = await readCards($, cwd)
  const hasBudget = await exists($, `${cwd}/${BUDGET}`)
  if (cards.length === 0 && !hasBudget) {
    $.ui.status(undefined)
    return
  }

  const flying = inFlight(cards)
  let budget: Budget | null = null
  if (hasBudget) {
    // The cards may not be in a cards.md the script knows: fall back to the caps alone.
    const asked = await runBudget($, cwd, ['--json', ...waveArgs(flying)])
    const fallback = asked.ok || flying.length === 0 ? asked : await runBudget($, cwd, ['--json', '--projected=0'])
    budget = fallback.ok ? parseBudget(fallback.stdout) : null
  }

  const text = statusText(flying, budget, WIDTH, withRate)
  $.ui.status(text ?? undefined)
  const now = await $.clock.now()
  await update($, refreshedAtAtom, () => now)

  const found = news(cards, budget, await read($, lastVerdictAtom), await read($, toastedAtom))
  if (budget) await update($, lastVerdictAtom, () => budget.verdict)
  if (found.verdict) {
    $.ui.toast(found.verdict, { timeoutMs: 12_000 })
    // A stop may come while the person is away from a long wave.
    if (budget?.verdict === 'stop') await $.ui.notify(found.verdict, { title: 'Token budget' }).catch(() => undefined)
  }
  for (const overrun of found.overruns) $.ui.toast(overrun.text, { timeoutMs: 10_000 })
  if (found.overruns.length > 0) await update($, toastedAtom, list => [...list, ...found.overruns.map(o => o.id)])

  // The ETA toast: once per cap per crossing; the flags follow what the script just said.
  if (withRate && budget) {
    const eta = etaNews(budget, await read($, etaWarnedAtom), options.tokenEtaWarnMinutes ?? 30)
    for (const toast of eta.toasts) $.ui.toast(toast, { timeoutMs: 12_000 })
    await update($, etaWarnedAtom, () => eta.warned)
  }
}

// After a turn: read again only if the last read is older than `seconds`.
export async function refreshThrottled($: EngineInterface, seconds: number, options: TokenOptions = {}): Promise<void> {
  const last = await read($, refreshedAtAtom)
  if (last !== 0 && (await $.clock.now()) - last < seconds * 1000) return
  await refresh($, options)
}

// `/tokens`: the script's own text for the wave, and a fresh status line.
export async function tokensText($: EngineInterface, options: TokenOptions = {}): Promise<string> {
  const cwd = await $.session.cwd()
  if (!(await exists($, `${cwd}/${BUDGET}`))) {
    return 'No .ai-gent/budget.yaml here. Set session_billable and week_billable in it to track a budget (shared/token-tracking.md).'
  }
  const flying = inFlight(await readCards($, cwd))
  const asked = await runBudget($, cwd, waveArgs(flying))
  const out = asked.ok || flying.length === 0 ? asked : await runBudget($, cwd, ['--projected=0'])
  await refresh($, options).catch(() => undefined)
  return out.ok ? out.stdout.trimEnd() : 'token_report.py budget could not run (is python3 installed, and are the cards in a cards.md?).'
}
