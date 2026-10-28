// The repository the session runs in, found once; shared by the project
// plugin's features (the checkpoint band, the pipeline pane).

import type { EngineInterface } from 'claude-code'

let root: string | null = null

export async function repoRoot($: EngineInterface): Promise<string> {
  if (root !== null) return root
  const cwd = await $.session.cwd()
  const top = await $.process
    .run(['git', 'rev-parse', '--show-toplevel'], { timeoutMs: 5000 })
    .catch(() => undefined)
  root = top?.exitCode === 0 && top.stdout.trim() ? top.stdout.trim() : cwd
  return root
}
