// Status files and log lines the workers tests read, as the Rust core writes them
// (rust/crates/ai-gent-core/src/delegate/status.rs; contracts/status.md).

import type { AgentInfo } from 'claude-code'

import type { WorkerState } from '../types'

export const NOW = 1_700_000_600_000

export function statusJson(over: Record<string, unknown> = {}): string {
  return JSON.stringify({
    track: 'feat-core',
    agent: 'codex',
    role: 'builder',
    tasks: ['T010', 'T011'],
    state: 'running',
    started: 1_700_000_000,
    updated: 1_700_000_590,
    events: 12,
    worktree: '.worktrees/feat-core',
    verdict: { result: 'pending' },
    tokens: null,
    ...over,
  })
}

export const track = (name: string, state: WorkerState, over: Record<string, unknown> = {}): string =>
  statusJson({ track: name, state, worktree: `.worktrees/${name}`, ...over })

// One of each state.
export const ALL_STATES: Record<string, string> = {
  'feat-core': track('feat-core', 'running'),
  'feat-ui': track('feat-ui', 'stalled', { agent: 'opencode:flint-40b-free', tasks: ['T020'], updated: 1_700_000_100, events: 3 }),
  'feat-docs': track('feat-docs', 'queued', { role: 'researcher', tasks: ['T040'], started: 1_700_000_590, events: 0 }),
  'feat-api': track('feat-api', 'failed', { agent: 'claude-subagent', role: 'reviewer', tasks: ['T030'], updated: 1_700_000_300, verdict: { result: 'rejected', reason: 'out-of-scope-path' } }),
  'feat-db': track('feat-db', 'done', { agent: 'gemini', tasks: ['T001'], updated: 1_700_000_400, verdict: { result: 'accepted' }, tokens: { input_tokens: 40_000, cached_input_tokens: 1000, output_tokens: 2500, reasoning_output_tokens: 10 } }),
  'feat-old': track('feat-old', 'taken-back', { tasks: ['T002'], updated: 1_700_000_200 }),
}

export const LOG = [
  JSON.stringify({ ts: 1_700_000_400, track: 'feat-db', agent: 'gemini', tasks: ['T001'], wall_secs: 400, tokens: { input_tokens: 40_000, output_tokens: 2500 }, result: 'done', tags: ['delegate'] }),
  'not json',
  JSON.stringify({ ts: 1_700_000_200, track: 'feat-old', agent: 'codex', tasks: ['T002'], wall_secs: 90, tokens: null, result: 'taken-back', tags: ['delegate'] }),
].join('\n')

// Kinds 2 and 3: a headless session of another Claude profile and another code agent, as status files.
export const PROFILE_AND_CODEX: Record<string, string> = {
  'feat-prof': track('feat-prof', 'running', { agent: 'claude-profile:work', tasks: ['T060'], updated: 1_700_000_595 }),
  'feat-cx': track('feat-cx', 'running', { agent: 'codex', tasks: ['T061'], updated: 1_700_000_590 }),
}

// Kind 1: what `$.agent.list()` gives. The descriptions are prompts on purpose (never shown by default).
export const AGENTS: AgentInfo[] = [
  { id: 'a1b2c3d4e5f6a7b8', description: 'PROMPT-SECRET: read the vault', type: 'general-purpose', status: 'running' },
  { id: 'e1', description: 'survey \u001b[31mthe repo\u0007', type: 'Explore', status: 'pending' },
  { id: 'r2', description: 'review it', type: 'code-reviewer', status: 'completed' },
  { id: 'f3', description: 'x', type: 'general-purpose', status: 'failed' },
]
