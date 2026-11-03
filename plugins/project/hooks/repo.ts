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

// The main checkout, where `.ai-gent/` lives: from inside a linked worktree `--show-toplevel` is the
// worktree, so the git common directory (the main checkout's `.git`) is used instead. Cached per session
// directory, because that directory moves between the main checkout and its worktrees.
const mainByCwd = new Map<string, string>()

export async function mainRoot($: EngineInterface): Promise<string> {
  const cwd = await $.session.cwd()
  const known = mainByCwd.get(cwd)
  if (known !== undefined) return known
  const common = await $.process
    .run(['git', 'rev-parse', '--path-format=absolute', '--git-common-dir'], { timeoutMs: 5000 })
    .catch(() => undefined)
  const dir = common?.exitCode === 0 ? common.stdout.trim() : ''
  const root = dir.endsWith('/.git') ? dir.slice(0, -5) : await repoRoot($)
  mainByCwd.set(cwd, root)
  return root
}
