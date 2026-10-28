// What the project plugin's hooks module keeps in $.state.

// The hand-off file as the band shows it (hooks/checkpoint.ts reads it).
export type Checkpoint = {
  path: string
  holds: string[]
  rules: string[]
  // The "Updated" line's timestamp, as written.
  updated: string | null
  // A commit is newer than the file (checkpoint.py show warns the same).
  isStale: boolean
  mtimeMs: number
}

declare module 'claude-code' {
  interface PluginState {
    project: {
      checkpoint: Shaped<Checkpoint | null>
      isBandHidden: boolean
    }
  }
}
