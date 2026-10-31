// What the secrets plugin's hooks module keeps in $.state (specs/024-guard-log).

// One decision of the secrets guard, verb only: never the guard's reason, a command line, an argument or a path.
export type GuardRow = {
  at: number
  guard: 'git' | 'db' | 'secrets' | 'repo' | 'containers'
  decision: 'deny' | 'ask'
  // The guard table entry's `match`, or `generic`.
  entry: string
  tool: string
  verb: string | null
  outcome: 'asked' | null
}

declare module 'claude-code' {
  interface PluginState {
    secrets: {
      // The last 200 decisions, newest last; the project plugin's /guards reads it.
      guardLog: Shaped<GuardRow[]>
    }
  }
}
