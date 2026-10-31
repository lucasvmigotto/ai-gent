// What the devcontainer plugin's hooks module keeps in $.state.

// One active band, as the `bands` atom records it (hooks/band-priority.ts
// holds the same shape; this contract has to be self-contained).
export type Priority = 'hold' | 'failure' | 'normal' | 'info'
export type Band = { id: string; plugin: string; priority: Priority; since: number }

// One element of `leftovers.sh --json`'s `findings` (specs/025-container-disk-band).
// `kind` is disk, dangling, volumes or buildah, or any string a newer script adds.
export type Finding = { kind: string; count?: number; free_gb?: number; floor_gb?: number }

// Why the last read gave no report: it timed out, did not print JSON, or the script is missing.
export type ReportProblem = 'timeout' | 'unreadable' | 'missing'

// What the band draws: the last report, when it was read, and whether that read worked.
export type ContainersState = {
  findings: Finding[]
  checked: boolean
  reason: string | null
  readAt: number
  ok: boolean
  problem: ReportProblem | null
}

declare module 'claude-code' {
  interface PluginState {
    devcontainer: {
      // What was already shown this session, by name, so a reload repeats nothing.
      toasted: Record<string, boolean>
      'containers.state': Shaped<ContainersState | null>
      // The bands active now, for band priority (hooks/bands.ts); other plugins may read it.
      bands: Band[]
    }
    // The other band plugins' atoms, read-only here; each is absent when its plugin is not installed.
    project: { bands: Band[] }
    git: { bands: Band[] }
    repo: { bands: Band[] }
  }
}
