// What the project plugin's hooks module keeps in $.state.

// One active band, as the `bands` atom records it (hooks/band-priority.ts
// holds the same shape; this contract has to be self-contained).
export type Band = { id: string; plugin: string; priority: 'hold' | 'failure' | 'normal' | 'info'; since: number }

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

// Which hand-off nudges already toasted this session (hooks/handoff.ts): once
// each, so a hot reload does not repeat them.
export type HandoffToasted = {
  context: boolean
  compact: boolean
  // A cap by its name (`session_billable`, `week_billable`).
  caps: Record<string, boolean>
  // When `token_report.py budget` last ran, in $.clock.now() milliseconds, for the 5-minute throttle.
  lastBudgetAt: number | null
}

// The pipeline pane (hooks/pipeline-model.ts computes these from the files).

// One artifact of shared/pipeline.md's table, as the pane shows it.
export type ArtifactState = {
  path: string
  label: string
  // The skill that owns it, for the Next hint.
  stage: string
  isPresent: boolean
  // `git log -1 --format=%cs`, null when uncommitted or without git.
  commitDate: string | null
  // An upstream artifact was committed after this one.
  staleSince: { path: string; date: string } | null
  // Absent although a later stage already ran.
  isProblem: boolean
}

export type FeatureState = 'todo' | 'doing' | 'review' | 'done' | 'retired'

export type Feature = {
  number: string
  name: string
  priority: string
  depends: string[]
  // Frontend / backend / status / QA cells as written in specs/README.md.
  sides: Record<string, string>
  state: FeatureState
  // wt.py plan's split.
  bucket: 'ready' | 'blocked' | 'done'
  waiting: string[]
  // The Spec Kit files present in specs/NNN-*/.
  files: { spec: boolean; plan: boolean; tasks: boolean }
}

export type MarkerKind = 'gap' | 'clarification' | 'assumption'

export type Marker = { kind: MarkerKind; file: string; line: number; text: string }

export type Next = {
  rule: 1 | 2 | 3 | null
  stage: string
  reason: string
  ref?: string
}

export type Snapshot = {
  artifacts: ArtifactState[]
  features: Feature[]
  hasReadme: boolean
  markers: { counts: Record<MarkerKind, number>; items: Marker[]; isTruncated: boolean }
  next: Next
  hasGit: boolean
  computedAt: number
}

declare module 'claude-code' {
  interface PluginState {
    project: {
      checkpoint: Shaped<Checkpoint | null>
      isBandHidden: boolean
      handoffToasted: Shaped<HandoffToasted>
      // The bands active now, for band priority (hooks/bands.ts); other plugins read it.
      bands: Band[]
      pipelineSnapshot: Shaped<Snapshot | null>
      pipelineIsOpen: boolean
      // Sections the person expanded, by name.
      pipelineExpanded: Record<string, boolean>
    }
    // The git plugin's atom, read-only here; absent when it is not installed.
    git: { bands: Band[] }
  }
}
