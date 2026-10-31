// What the git plugin's hooks module keeps in $.state.

// One active band, as the `bands` atom records it (hooks/band-priority.ts
// holds the same shape; this contract has to be self-contained).
export type Band = { id: string; plugin: string; priority: 'hold' | 'failure' | 'normal' | 'info'; since: number }

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

// What `release.py --dry-run --json` says, reduced to what the band draws
// (specs/028-release-readiness-band). `plugins` counts targets of kind "plugin".
// The atom adds `branch`, the chain branch it was computed on (for the behind note).
export type ReleasePlan = { version: string; bump: 'major' | 'minor' | 'patch'; since: string | null; plugins: number }

// One decision of this plugin's guard, verb only: never the guard's reason, a command line, an argument or a path.
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
    git: {
      // The last 200 guard decisions, newest last; the project plugin's /guards reads it.
      guardLog: Shaped<GuardRow[]>
      hygiene: Shaped<Hygiene | null>
      // What was already shown this session, by name, so a reload repeats nothing.
      toasted: Record<string, boolean>
      // The bands active now, for band priority (hooks/bands.ts); other plugins read it.
      bands: Band[]
      // The release band's plan, or the failure line; null when nothing is releasable or the branch is not a chain branch.
      releasePlan: Shaped<(ReleasePlan & { branch: string }) | { failed: string } | null>
      // The key (HEAD, a newline, the merged tags) releasePlan was computed for.
      releasePlanFor: string | null
      // The key at which the person pressed Hide; the band returns when the key changes.
      releaseBandHidden: string | null
    }
    // The project plugin's atom, read-only here; absent when it is not installed.
    project: { bands: Band[] }
  }
}
