// What the agents plugin's hooks module keeps in $.state.

declare module 'claude-code' {
  interface PluginState {
    agents: {
      // The budget verdict last seen, to toast only a change.
      tokenLastVerdict: string | null
      // Cards already toasted for passing their projection, once each per session.
      tokenToasted: string[]
      // Caps whose ETA is under tokenEtaWarnMinutes now, so a toast fires only when a cap enters it.
      tokenEtaWarned: string[]
      // When the budget was last read (the clock's ms), for the throttle.
      tokenRefreshedAt: number
      // The landing toast (specs/019-card-cost-on-landing): card id to the state last read.
      landingSeen: Record<string, string>
      // Card ids already toasted at their current landing; cleared when a card leaves `landed`.
      landingToasted: string[]
      // Card ids already `landed` at the first read of the session, never toasted.
      landingFirstLook: string[]
      // Cards landed (and over or under their projection) since the last wave toast.
      landingWave: { landed: number; over: number; under: number }
      // The card timeline pane (specs/020-card-timeline-pane): whether it is open (it polls only then).
      cardsOpen: boolean
      // Each card's state at the last read, to detect transitions.
      cardsSeen: Record<string, string>
      // The transitions the mod observed, newest last, capped at 200.
      cardsLog: { id: string; from: string; to: string; at: string }[]
      // The ledger's cards at the last read.
      cardsRows: {
        id: string
        state: string
        projectedText: string
        usedText: string
        projected: number | null
        used: number | null
        branch?: string
        worktree?: string
      }[]
      // When the mod first looked (local HH:MM).
      cardsSince: string | null
      // The last Measure: its status and, when done, each card's span (UTC ms) and the script's figures.
      cardsMeasured: {
        status: 'running' | 'failed' | 'done'
        error: string | null
        spans: { card: string; startMs: number; endMs: number; used: number; projected: number; agents: number; calls: number }[]
        at: number
      } | null
    }
  }
}
