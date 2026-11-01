// The agents plugin's Claude Code mod, two features that only make visible what
// the skills already record:
//  - the token status (specs/014-token-status): a status line with the cards in
//    flight, their projection against what they used, and what the budget leaves;
//    a toast when the wave stops fitting or a card passes its projection, and one
//    when a card lands with what it cost; /tokens for the full text.
//    token_report.py stays the only thing that measures or judges a budget.
//  - the card timeline pane (specs/020-card-timeline-pane): /cards-pane, a pane
//    with the ledger's cards, the state changes seen and, on Measure, each
//    card's span. It has its own option and does not depend on the token status.
//  - the worktree board (specs/022-worktree-board): /worktree-board, a pane with one
//    row per .worktrees/<branch> (ahead and behind, uncommitted changes and, with gh
//    signed in on GitHub, the pull request and its checks). wt.py stays the only
//    thing that reads the worktrees. It has its own options too.
// The mod writes nothing.

import type { Register } from 'claude-code'

import { PANE, close, command, measure, resume, snapshot } from './cards'
import { view } from './cards-view'
import type { Tone } from './cards-view'
import { landing, resetLanding } from './landing'
import { refresh, refreshThrottled, tokensText } from './tokens'
import type { TokenOptions } from './tokens'
import { PANE as BOARD, close as closeBoard, command as boardCommand, intervals, resume as resumeBoard, snapshot as boardSnapshot, worktreeBash } from './worktrees'
import type { BoardOptions, Intervals } from './worktrees'
import { view as boardView } from './worktrees-model'

type Options = { tokenStatus?: boolean; tokenRefreshSeconds?: number; cardsPane?: boolean; cardsPollSeconds?: number } & TokenOptions & BoardOptions

const COLOR: Partial<Record<Tone, string>> = { good: 'success', warn: 'warning', bad: 'error' }

// Each feature composes on its own option: tokenStatus off turns 014, 018 and 019
// off and leaves the panes; cardsPane off leaves the token status and the board;
// worktreeBoard off leaves everything else. An event takes
// one handler without a matcher, so session.start is registered once and each
// feature's step has its own `.catch`: a failing one never takes the other down.
// The board's own handlers, composed ahead of the cards' early return below so that
// the cards pane off does not take it with it.
function worktreeBoard(on: Parameters<Register>[0], every: Intervals): void {
  worktreeBash(on)

  on('command.run', { command: 'worktree-board' }, async $ => boardCommand($, every))

  on('ui.close', { id: BOARD }, async ($, e, next) => {
    const closed = await next(e)
    await closeBoard($)
    return closed
  }).catch(($, e, next) => next(e))

  on('ui.render', { component: 'Pane', requestId: BOARD }, async ($, e) => {
    const { Box, Text } = $.ui.resolve(e)
    const lines = boardView(await boardSnapshot($), e.props.bodyColumns)
    return (
      <Box flexDirection="column">
        {lines.map(line => (
          <Text key={line.key} bold={line.tone === 'bold'} dimColor={line.tone === 'dim'} color={COLOR[line.tone]}>
            {line.text}
          </Text>
        ))}
      </Box>
    )
  })
}

export const register: Register = (on, options: Options) => {
  const hasTokens = options.tokenStatus !== false
  const hasCards = options.cardsPane !== false
  const hasBoard = options.worktreeBoard !== false
  const every = intervals(options)
  const seconds = options.tokenRefreshSeconds ?? 120
  const pollSeconds = options.cardsPollSeconds ?? 5
  // The burn rate and its ETA toast (specs/018-token-burn-rate) ride the same refresh.
  const rate: TokenOptions = {
    tokenBurnRate: options.tokenBurnRate,
    tokenEtaWarnMinutes: options.tokenEtaWarnMinutes,
    cardCostToast: options.cardCostToast,
    cardCostHintPercent: options.cardCostHintPercent,
  }
  // The card cost on landing (specs/019-card-cost-on-landing) rides the same ledger reader.
  const isLanding = options.cardCostToast !== false

  if (hasTokens && isLanding) landing(on, rate)
  if (hasBoard) worktreeBoard(on, every)

  on('session.start', async ($, e, next) => {
    // A new session's first ledger read is a first look: nothing already landed is replayed.
    if (hasTokens && isLanding) await resetLanding($).catch(() => undefined)
    const started = await next(e)
    if (hasTokens) {
      await $.command.register({ name: 'tokens', description: 'Show the token budget for the cards in flight (token_report.py budget)' }).catch(() => undefined)
      // Detached: the budget reads a week of transcripts and must not hold the session's start.
      $.clock.after(0, () => void refresh($, rate).catch(() => undefined))
    }
    if (hasCards) {
      await $.command
        .register({ name: 'cards-pane', description: "Show the orchestrated cards, the state changes seen and, on Measure, each card's span" })
        .catch(() => undefined)
      await resume($, pollSeconds).catch(() => undefined)
    }
    if (hasBoard) {
      await $.command.register({ name: 'worktree-board', description: 'Show the worktrees under .worktrees/ with ahead/behind, uncommitted changes and, with gh signed in, the pull request and checks' }).catch(() => undefined)
      await resumeBoard($, every).catch(() => undefined)
    }
    return started
  })

  if (hasTokens) {
    on('turn.complete', async ($, e, next) => {
      $.clock.after(0, () => void refreshThrottled($, seconds, rate).catch(() => undefined))
      return next(e)
    })

    on('command.run', { command: 'tokens' }, async $ => ({ text: await tokensText($, rate) }))
  }

  if (!hasCards) return

  on('command.run', { command: 'cards-pane' }, async ($, e) => command($, pollSeconds, e.args))

  on('ui.close', async ($, e, next) => {
    const closed = await next(e)
    if (e.id === PANE) await close($)
    return closed
  }).catch(($, e, next) => next(e))

  on('ui.render', { component: 'Pane', requestId: PANE }, async ($, e) => {
    const { Box, Button, Text } = $.ui.resolve(e)
    const lines = view(await snapshot($), e.props.bodyColumns)
    return (
      <Box flexDirection="column">
        {lines.map(line => (
          <Box key={line.key}>
            <Text bold={line.tone === 'bold'} dimColor={line.tone === 'dim'} color={COLOR[line.tone]}>
              {line.text}
            </Text>
            {line.button ? <Button key={`button-${line.button.id}`} label={line.button.label} onPress={() => measure($)} /> : null}
          </Box>
        ))}
      </Box>
    )
  })
}
