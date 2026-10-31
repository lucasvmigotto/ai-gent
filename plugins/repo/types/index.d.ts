// What the repo plugin's hooks module keeps in $.state (specs/023-ci-watch-band).

// One active band, as the `bands` atom records it (hooks/band-priority.ts
// holds the same shape; this contract has to be self-contained).
export type Band = { id: string; plugin: string; priority: 'hold' | 'failure' | 'normal' | 'info'; since: number }

// One run of the pushed commit, from `gh run list --json databaseId,name,status,conclusion,workflowName,headSha`.
export type CiRun = { id: number; workflow: string; name: string; status: string; conclusion: string }

// The pushed commit and the runs last read.
export type CiRuns = { sha: string; branch: string; runs: CiRun[]; startedAt: number }

// The back-off: the phase (30 s, 60 s, 120 s), consecutive failed reads (stop at 3), the hard stop (clock ms).
export type CiTimer = { phase: number; failures: number; stopAt: number }

// `reporun detect --json`, reduced to what the band needs.
export type CiDetect = { platform: string; tokenInPass: boolean; ghSignedIn: boolean }

declare module 'claude-code' {
  interface PluginState {
    repo: {
      ciRuns: Shaped<CiRuns | null>
      // Failing run ids already toasted, so a run toasts once.
      ciToasted: number[]
      // The sha the person hid the band for.
      ciHidden: string | null
      ciTimer: CiTimer | null
      ciDetect: Shaped<CiDetect | null>
      // The bands active now, for band priority (hooks/bands.ts); other plugins may read it.
      bands: Band[]
    }
    // The git and project plugins' atoms, read-only here; absent when not installed.
    git: { bands: Band[] }
    project: { bands: Band[] }
  }
}
