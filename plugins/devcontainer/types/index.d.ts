// What the devcontainer plugin's hooks module keeps in $.state.

// One active band, as the `bands` atom records it (hooks/band-priority.ts
// holds the same shape; this contract has to be self-contained).
export type Priority = 'hold' | 'failure' | 'normal' | 'info'
export type Band = { id: string; plugin: string; priority: Priority; since: number; summary?: { text: string; tone: 'ok' | 'info' | 'warn' } }

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
    devcontainer: {
      // The last 200 guard decisions, newest last; the project plugin's /guards reads it.
      guardLog: Shaped<GuardRow[]>
      // What was already shown this session, by name, so a reload repeats nothing.
      toasted: Record<string, boolean>
      'containers.state': Shaped<ContainersState | null>
      // The bands active now, for band priority (hooks/bands.ts); other plugins may read it.
      bands: Band[]
    }
    // The other band plugins' atoms, read-only here; each is absent when its plugin is not installed.
    // `workspaceLeader`: the project plugin draws the workspace line, so bands with a summary fold into it.
    project: { bands: Band[]; workspaceLeader: boolean }
    git: { bands: Band[] }
    repo: { bands: Band[] }
  }
}
