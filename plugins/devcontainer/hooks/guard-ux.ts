// How the guards' decisions are presented in Claude Code (specs/015-guard-ux).
//
// Each guard.sh decides; this only explains. A plugin's hooks module calls
// `decorate` on what `next(e)` returned at `classic.PreToolUse`, and gets back
// the same decision with more words around it: a deny keeps the guard's exact
// reason and gains one line of context for the model plus a notice for the
// person; an ask keeps the guard's reason verbatim under a fuller question.
// It never allows, never turns a deny into an ask or the reverse, and acts only
// on a reason that carries its own guard's marker.
//
// The engine refuses an import through a symlink that leaves a plugin, so this
// file is COPIED into plugins/{git,db,secrets,repo,devcontainer}/hooks/guard-ux.ts;
// `python3 scripts/consistency.py --sync-shared-code` rewrites the copies and
// scripts/consistency.py fails if they differ or if a guard reason has no entry.

export type GuardId = 'git' | 'db' | 'secrets' | 'repo' | 'containers'

// What a classic.PreToolUse result can carry (the part decorate touches).
export type Decision = {
  allow?: true
  ask?: string
  deny?: string
  additionalContext?: string[]
  [other: string]: unknown
}

export type Entry = {
  // A fragment of the guard's reason that stays the same when a variable in it changes.
  match: string
  // One sentence for the person: what the command would do.
  say: string
  // A deny: what to do instead.
  instead?: string
  // An ask: when allowing is right.
  allowIf?: string
}

export type Guard = {
  // Every reason of the guard carries it (guard.sh's printf).
  marker: string
  // The skill that explains the rule.
  skill: string
  entries: readonly Entry[]
}

export const GUARDS: Record<GuardId, Guard> = {
  git: {
    marker: 'git:workflow guard:',
    skill: 'git:workflow',
    entries: [
      { match: 'hooks must not be bypassed', say: 'Hooks are never bypassed (--no-verify, -n).', instead: 'fix what the hook reports, then commit again.' },
      { match: 'commits, issues, pull requests and comments carry no sign-off', say: 'Commits, issues, pull requests and comments carry no mention of the agent.', instead: 'remove the Co-Authored-By or "generated with" line and run the command again.' },
      { match: 'force pushes use --force-with-lease', say: 'A plain force push can overwrite other people\'s commits.', instead: 'use --force-with-lease, and only on the user\'s explicit order.' },
      { match: 'git push publishes', say: 'This publishes commits to the remote.', allowIf: 'Allow only if you asked for this push.' },
      { match: 'opening a PR publishes the branch', say: 'This opens a pull request, which publishes the branch.', allowIf: 'Allow only if you asked for it.' },
      { match: 'git branch -D deletes unmerged work', say: 'This deletes a branch that may hold unmerged work.', allowIf: 'Allow only if you asked for it; the workflow uses git branch -d.' },
      { match: 'rewrites history or discards work', say: 'This rewrites history or discards work.', allowIf: 'Allow only if you asked for it.' },
      { match: 'the workflow works on a branch', say: 'This commits or merges on a long-lived branch.', allowIf: 'Allow only if you asked for it.' },
      { match: 'promotion up the branch chain', say: 'This commits or merges on a branch higher in the chain.', allowIf: 'Allow only if you asked to promote.' },
      { match: 'commits directly on', say: 'This commits straight onto the integration branch.', allowIf: 'Work goes on a branch merged into it; allow only if you meant this (concluding a merge is fine).' },
    ],
  },
  db: {
    marker: 'db guard:',
    skill: 'db:connect',
    entries: [
      { match: 'db credentials file', say: 'Only dbrun reads database credential files.', instead: 'use dbrun profiles, classify or test.' },
      { match: 'restores into a database', say: 'A direct restore changes a database.', instead: 'local restores go through dbrun restore; anything else is a script for a person to run.' },
      { match: 'with SQL that writes', say: 'A direct client call would write to a database.', instead: 'use dbrun (plan and apply on a proven local database, a script for anything else).' },
      { match: 'bypasses dbrun', say: 'This calls a database client directly: no classification, masking or command log.', allowIf: 'Allow only if you want this direct call.' },
    ],
  },
  secrets: {
    marker: 'secrets guard:',
    skill: 'secrets:pass',
    entries: [
      { match: 'decrypting the password store directly', say: 'Decrypting the store would show secret values.', instead: 'the runner that needs a secret reads it.' },
      { match: 'ai-gent/db/ entries are read only by dbrun', say: 'Database passwords are read only by dbrun.', instead: 'use dbrun test or query.' },
      { match: 'hands the secret to a command that can print it', say: 'This would hand a secret to a command that can print it.', instead: 'run the tool that needs it after --.' },
      { match: 'pass -c/-q exposes a secret value', say: 'This would expose a secret value.', instead: 'the runner that needs it reads it; the agent never does.' },
      { match: 'the agent never writes a credential', say: 'The agent never types a credential.', instead: 'ask the user to run `pass insert <entry>` in their own terminal.' },
      { match: 'changes the password store', say: 'This changes the password store.', allowIf: 'The user decides.' },
      { match: 'that shows a secret value from pass', say: 'This would show a secret value.', instead: 'the agent sees names only (pass ls).' },
    ],
  },
  repo: {
    marker: 'repo guard:',
    skill: 'repo:pr',
    entries: [
      { match: 'work items, pull requests and comments carry no sign-off', say: 'Issues, work items, pull requests and comments carry no mention of the agent.', instead: 'remove the Co-Authored-By or "generated with" line and run the command again.' },
      { match: 'reporun --write changes an issue', say: 'This changes an issue, work item or pull request on the platform.', allowIf: 'Allow only if you asked for exactly this change.' },
    ],
  },
  containers: {
    marker: 'containers guard:',
    skill: 'devcontainer:workflow',
    entries: [
      { match: 'prune removes other projects', say: 'A prune removes other projects\' images, volumes and builds.', instead: 'remove by ID, or by this project\'s label (containers.md §9a).' },
      { match: 'removing everything takes other projects', say: 'Removing everything takes other projects\' resources.', instead: 'remove by ID or by this project\'s label (containers.md §9).' },
      { match: 'removes whatever an unfiltered listing returns', say: 'That removes whatever an unfiltered listing returns.', instead: 'filter the listing by this project\'s label (containers.md §9a).' },
    ],
  },
}

export type Decorated = {
  result: Decision
  // What to show the person (a toast), or null.
  notice: string | null
}

// The same decision with more words around it. A result that is not this
// guard's, or is neither a deny nor an ask, comes back untouched.
export function decorate(id: GuardId, result: Decision): Decorated {
  const guard = GUARDS[id]
  const isDeny = typeof result.deny === 'string'
  const reason = isDeny ? result.deny : result.ask
  if (typeof reason !== 'string' || result.allow !== undefined) return { result, notice: null }
  const at = reason.indexOf(guard.marker)
  if (at < 0) return { result, notice: null }

  const text = reason.slice(at + guard.marker.length).trim()
  const entry = guard.entries.find(candidate => text.includes(candidate.match))

  if (isDeny) {
    const notice = `${guard.marker.slice(0, -1)} refused this. ${entry ? `${entry.say}${entry.instead ? ` Instead: ${entry.instead}` : ''}` : text} (${guard.skill})`
    const line = entry?.instead ? `${guard.marker.slice(0, -1)}: instead, ${entry.instead}` : undefined
    return { result: line ? { ...result, additionalContext: [...(result.additionalContext ?? []), line] } : result, notice }
  }

  if (!entry) return { result, notice: null }
  return { result: { ...result, ask: `${entry.say}${entry.allowIf ? ` ${entry.allowIf}` : ''}\n\n${reason}` }, notice: null }
}
