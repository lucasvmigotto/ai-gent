// The landing toast's reading (specs/019-card-cost-on-landing): everything that
// needs `$`. It looks at the cards the token status already read (and, after a
// Bash call that ran `token_report.py tasks`, reads the ledger file at once), and
// toasts each card newly `landed` with its used against projected figure. It
// spawns nothing and writes nothing; the wording is in landing-model.ts.

import { atom, read, update } from 'claude-code'
import type { EngineInterface, Register } from 'claude-code'

import { NO_WAVE, addToWave, landedNews, landingText, waveReady, waveSummary } from './landing-model'
import { readCards } from './ledger'
import type { LedgerCard } from './token-model'

const seenAtom = atom({ plugin: 'agents', key: 'landingSeen' } as const, {})
const toastedAtom = atom({ plugin: 'agents', key: 'landingToasted' } as const, [])
const firstLookAtom = atom({ plugin: 'agents', key: 'landingFirstLook' } as const, [])
const waveAtom = atom({ plugin: 'agents', key: 'landingWave' } as const, NO_WAVE)

export type LandingOptions = { cardCostToast?: boolean; cardCostHintPercent?: number }

const MAX_TOASTS = 3

// A new session (start, resume): the first read is a first look again.
export async function resetLanding($: EngineInterface): Promise<void> {
  await update($, seenAtom, () => ({}))
  await update($, toastedAtom, () => [])
  await update($, firstLookAtom, () => [])
  await update($, waveAtom, () => NO_WAVE)
}

// Looks at the cards just read: toasts the landings, then the wave if it is over.
export async function scanLanding($: EngineInterface, cards: readonly LedgerCard[], options: LandingOptions = {}): Promise<void> {
  if (options.cardCostToast === false) return
  const hint = typeof options.cardCostHintPercent === 'number' ? options.cardCostHintPercent : 25
  const found = landedNews(cards, await read($, seenAtom), await read($, toastedAtom), await read($, firstLookAtom), hint)
  await update($, seenAtom, () => found.seen)
  await update($, toastedAtom, () => found.toasted)
  await update($, firstLookAtom, () => found.firstLook)

  for (const landing of found.news.slice(0, MAX_TOASTS)) $.ui.toast(landingText(landing), { timeoutMs: 10_000 })
  const more = found.news.length - MAX_TOASTS
  if (more > 0) $.ui.toast(`+${more} more landed — /tokens`, { timeoutMs: 10_000 })

  const wave = addToWave(await read($, waveAtom), found.news)
  if (waveReady(cards, wave)) {
    $.ui.toast(waveSummary(wave), { timeoutMs: 12_000 })
    await update($, waveAtom, () => NO_WAVE)
  } else {
    await update($, waveAtom, () => wave)
  }
}

// The module's own hook: the immediate read after `tasks` (which writes the used
// figure the toast waits for; this only reads the file). The session reset is
// register.tsx's own session.start handler calling resetLanding, because an event
// takes one handler without a matcher.
export function landing(on: Parameters<Register>[0], options: LandingOptions): void {
  on('tool.call', { tool: 'Bash' }, async ($, e, next) => {
    const ran = await next(e)
    if (ran.deny === undefined && String(e.command ?? '').includes('token_report.py tasks')) {
      $.clock.after(0, () => {
        void (async () => {
          const cards = await readCards($, await $.session.cwd())
          await scanLanding($, cards, options)
        })().catch(() => undefined)
      })
    }
    return ran
  })
}
