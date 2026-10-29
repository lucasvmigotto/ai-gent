// What the git plugin's hooks module keeps in $.state.

export type ChainBranch = 'main' | 'master' | 'dev' | 'develop'

// One long-lived branch compared with its remote-tracking branch.
export type BranchState = {
  name: ChainBranch
  // Commits only the local branch has, and only the remote-tracking branch has.
  ahead: number
  behind: number
  // Release versions among the commits it is behind by, newest first.
  releases: string[]
}

export type Hygiene = {
  branches: BranchState[]
  // FETCH_HEAD's modification time, null if the repository never fetched.
  fetchedAt: number | null
  readAt: number
}

declare module 'claude-code' {
  interface PluginState {
    git: {
      hygiene: Shaped<Hygiene | null>
      // What was already shown this session, by name, so a reload repeats nothing.
      toasted: Record<string, boolean>
    }
  }
}
