// What the project plugin's hooks module keeps in $.state.

// One active band, as the `bands` atom records it (hooks/band-priority.ts
// holds the same shape; this contract has to be self-contained).
export type Band = { id: string; plugin: string; priority: 'hold' | 'failure' | 'normal' | 'info'; since: number; summary?: { text: string; tone: 'ok' | 'info' | 'warn' } }

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
  // The reason specs/README.md's Applicability table gives for the artifact
  // being N/A here; absent when it applies (or exists).
  notApplicable?: string
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
  // The folder name under specs/ for this number (`011-explorer-ui`); absent when none exists.
  dir?: string
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

// The next-stage toast (hooks/nextstage.ts): the last recommendation seen, so a
// change is told once.
export type NextStageLast = Next | null

// One decision of a guard plugin's guard, verb only (the five plugins' own
// types declare the same shape; this contract has to be self-contained).
export type GuardRow = {
  at: number
  guard: 'git' | 'db' | 'secrets' | 'repo' | 'containers'
  decision: 'deny' | 'ask'
  entry: string
  tool: string
  verb: string | null
  outcome: 'asked' | null
}

// The workers pane (hooks/workers-model.ts parses these from
// .ai-gent/delegate/<track>/status.json and .ai-gent/delegation.log, spec 041).
export type WorkerState = 'queued' | 'running' | 'stalled' | 'done' | 'failed' | 'taken-back'

// Where a worker comes from: a subagent of this session ($.agent.list()), a headless session of another Claude
// profile (claude-profile[:name]) or any other code agent; the last two are status files.
export type WorkerKind = 'subagent' | 'profile' | 'agent'

export type WorkerStatus = {
  kind: WorkerKind
  // A subagent's description, only with the workersShowDescription option; null otherwise.
  note: string | null
  track: string
  agent: string
  role: 'builder' | 'reviewer' | 'researcher'
  tasks: string[]
  state: WorkerState
  // Epoch seconds, as the file has them.
  started: number
  updated: number
  events: number
  worktree: string
  // A rule id when rejected, never a diff or a brief.
  verdict: { result: 'pending' | 'accepted' | 'rejected'; reason: string | null }
  // Input plus output tokens the agent reported; null when it reported none.
  tokens: number | null
}

// One run's line of the delegation log.
export type WorkerLogLine = { ts: number; track: string; agent: string; tasks: string[]; wallSecs: number; tokens: number | null; result: WorkerState }

export type WorkersSnapshot = {
  workers: WorkerStatus[]
  log: WorkerLogLine[]
  // The worktree cap (max_parallel of .ai-gent/parallel.yaml, else 3).
  cap: number
  // Status files that were present but not well-formed.
  skipped: number
  // $.clock.now() milliseconds of the read that made it.
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
      // True while this plugin draws the workspace line (hooks/workspace.tsx); the other band plugins read it.
      workspaceLeader: boolean
      // Whether the workspace line's Details are open.
      workspaceExpanded: boolean
      // The model and effort of the last request (turn.step), drawn at the right of the Details row; null before the first one.
      sessionModel: { model: string; effort: string | null } | null
      // The context window's used percent as of the last turn or session start; null when the engine gave none.
      contextPercent: number | null
      // The account's rate-limit windows as of the last session.measure, turn or session start (percent used, one decimal; resetsAt is ISO 8601); empty when none was reported.
      rateLimits: { kind: 'five_hour' | 'seven_day' | 'spend_limit'; percent: number; resetsAt?: string }[]
      // The workers pane (spec 041): what the last read of the status files found.
      workersSnapshot: Shaped<WorkersSnapshot | null>
      workersIsOpen: boolean
      // Tracks whose detail rows the person opened.
      workersExpanded: Record<string, boolean>
      // The interval the workers poll keeps now, in milliseconds; 0 when stopped.
      workersPollMs: number
      pipelineSnapshot: Shaped<Snapshot | null>
      pipelineIsOpen: boolean
      // Sections the person expanded, by name.
      pipelineExpanded: Record<string, boolean>
      // The matrix filter, by readiness (`all` when never set).
      pipelineFilter: 'all' | 'ready' | 'blocked' | 'done'
      // When the poll last checked, in $.clock.now() milliseconds; 0 until the first check.
      pipelineCheckedAt: number
      // The interval the poll keeps now, in milliseconds; 0 when it is stopped or its last check failed.
      pipelinePollMs: number
      // The last `snapshot.next` seen; seeded at session start without a toast.
      nextStageLast: Shaped<NextStageLast>
      // `stage reason` keys already toasted this session.
      nextStageShown: string[]
      // `mtimeMs` of the watched artifacts at the last turn end, for writes made by other means.
      nextStageMtimes: Record<string, number>
    }
    // The other plugins' atoms, read-only here; each is absent when its plugin is not installed.
    // guardLog is each guard plugin's own ring (spec 024); /guards reads all five.
    git: { bands: Band[]; guardLog: Shaped<GuardRow[]> }
    repo: { bands: Band[]; guardLog: Shaped<GuardRow[]> }
    devcontainer: { bands: Band[]; guardLog: Shaped<GuardRow[]> }
    db: { guardLog: Shaped<GuardRow[]> }
    secrets: { guardLog: Shaped<GuardRow[]> }
  }
}
