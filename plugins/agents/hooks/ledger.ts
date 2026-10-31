// Reading the orchestration ledger with $.fs, shared by the token status (tokens.ts)
// and the landing toast (landing.ts). Read-only; the parsing is in token-model.ts.

import type { EngineInterface } from 'claude-code'

import { parseLedger } from './token-model'
import type { LedgerCard } from './token-model'

export const LEDGER = '.ai-gent/orchestrate-status.md'

export async function exists($: EngineInterface, path: string): Promise<boolean> {
  return $.fs.exists(path).catch(() => false)
}

export async function readCards($: EngineInterface, cwd: string): Promise<LedgerCard[]> {
  if (!(await exists($, `${cwd}/${LEDGER}`))) return []
  return parseLedger(String(await $.fs.read(`${cwd}/${LEDGER}`).catch(() => '')))
}
