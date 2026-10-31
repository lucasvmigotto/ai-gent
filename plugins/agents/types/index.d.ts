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
    }
  }
}
