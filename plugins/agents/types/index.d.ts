// What the agents plugin's hooks module keeps in $.state.

declare module 'claude-code' {
  interface PluginState {
    agents: {
      // The budget verdict last seen, to toast only a change.
      tokenLastVerdict: string | null
      // Cards already toasted for passing their projection, once each per session.
      tokenToasted: string[]
      // When the budget was last read (the clock's ms), for the throttle.
      tokenRefreshedAt: number
    }
  }
}
