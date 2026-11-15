// Reads the guard log the five guards append to (specs/024-guard-log, decided
// 2026-11-14): each guard (`ai-gent guard`, the shell fallback in guard.sh, the
// OpenCode plugin) writes one JSON row per deny or ask to
// <state>/ai-gent/guard-log/<session id>.jsonl; this only reads this session's
// file. A missing or unreadable file is no decisions; a line that is not a row
// is skipped.

import type { EngineInterface } from 'claude-code'

import type { GuardRow } from '../types'
import { grantLine } from './grant-model'
import { MAX_LIMIT, parseRows } from './guards'
import { mainRoot } from './repo'

const MAX_GRANT_CHARS = 4096

// The autonomy line, from the main checkout's grant file. Read only; a file that cannot be read is no grant.
export async function readGrantLine($: EngineInterface): Promise<string> {
  const root = await mainRoot($).catch(() => null)
  if (root === null) return grantLine(null, new Date())
  const text = await $.fs.read(`${root}/.ai-gent/autonomy`).then(t => String(t).slice(0, MAX_GRANT_CHARS)).catch(() => null)
  return grantLine(text, new Date())
}

const SESSION_ID = /^[A-Za-z0-9_-]{1,128}$/

// The log's directory, as the guards resolve it: AI_GENT_GUARD_LOG_DIR, else the state directory.
async function logDir($: EngineInterface): Promise<string | null> {
  const set = (value: string | undefined): string | null => (typeof value === 'string' && value !== '' ? value : null)
  const own = set(await $.env.get('AI_GENT_GUARD_LOG_DIR').catch(() => undefined))
  if (own !== null) return own
  const state = set(await $.env.get('XDG_STATE_HOME').catch(() => undefined))
  if (state !== null) return `${state}/ai-gent/guard-log`
  const home = set(await $.env.get('HOME').catch(() => undefined))
  if (home !== null) return `${home}/.local/state/ai-gent/guard-log`
  const local = set(await $.env.get('LOCALAPPDATA').catch(() => undefined))
  return local === null ? null : `${local}/ai-gent/state/guard-log`
}

// This session's rows, oldest first, at most the newest MAX_LIMIT.
export async function readGuards($: EngineInterface): Promise<GuardRow[]> {
  const id = await $.session.id().catch(() => '')
  const dir = await logDir($)
  if (dir === null || !SESSION_ID.test(id)) return []
  const text = await $.fs.read(`${dir}/${id}.jsonl`).then(t => String(t)).catch(() => '')
  const parsed = text.split('\n').filter(line => line.trim() !== '').map(line => {
    try {
      return JSON.parse(line) as unknown
    } catch {
      return null
    }
  })
  return parseRows(parsed).slice(-MAX_LIMIT)
}
