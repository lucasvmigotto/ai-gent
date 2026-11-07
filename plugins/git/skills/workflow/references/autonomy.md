# The autonomy grant

A hands-off run (`SKILL.md`, *What each permission level allows*) is the user's
permission given beforehand, but the guard hooks can't see the conversation, so
on their own they would still ask on every push and every pull request. The
grant is how the user's permission reaches them: a small file only the user
writes, which lifts a fixed set of questions while it lasts.

## The file

`.ai-gent/autonomy` at the repository root, `key: value` lines:

```txt
expires: 2026-11-06T18:00:00Z
branches: develop dev
actions: push pr merge issues
```

- `expires` — UTC, in exactly that form. Required: no `expires`, an unreadable
  one or a time in the past means no grant, and every guard asks as usual.
- `branches` — the integration branches the grant covers (default
  `develop dev`).
- `actions` — which questions to lift: `push`, `pr`, `merge`, `issues`.

Keep it out of version control (`.git/info/exclude`, as `project:checkpoint`
does for its file). The repository root is the one of the command's directory,
so a run inside a worktree reads that worktree's own file.

## Who writes it

Only the user. The `git` guard denies an agent's command that writes, moves or
removes the file (a redirect, `rm`, `mv`, `sed -i`, `tee`, …; reading it is
fine) and an `Edit`, `Write` or similar aimed at it. Never create or change it
yourself, never suggest an expiry you chose: ask the user for the scope and the
time, and give them the file's text to put there (in their own terminal, or
with `!` in the prompt). Like the other guards, this catches the commands an
agent writes, not every possible obfuscation.

## What it lifts

| Action | The question that goes away | Still asked |
| --- | --- | --- |
| `push` | `git push` of a work branch or `develop`, from a work branch | a push naming `main`, `master`, `homolog` or `staging`; from one of those branches; `--all`, `--mirror`, `--tags`, `--delete`, `--prune`; a `+refspec` |
| `pr` | `gh pr create --base <a covered branch>` and the same through `reporun cli github` | a PR without an explicit covered `--base`, or into any other branch |
| `merge` | `reporun cli github --write -- pr merge …` | nothing the guard can see; **`reporun` refuses** a PR whose base is not covered, or that it can't read, so a merge into `main` is always the user's own step |
| `issues` | `reporun cli github --write -- issue create`, `comment`, `edit` | `issue close`, labels through the API, `reporun api`, `batch` and `chain` |

Never lifted, grant or not: `--no-verify`, agent attribution and a force push
without a lease (denied); promotion to `homolog`, `staging` or `main`, tags,
releases, history rewrites, `branch -D`, and the checkpoint holds gate of the
`project` mod. A local merge into `develop` was never asked about, so it needs
no grant.

## Where it applies

The Claude Code hooks, the OpenCode guard plugin (`opencode/guard/`) and the
Rust evaluator (`ai-gent guard`, `ai-gent repo`) read the same file and decide
the same, checked by the shared fixtures. OpenCode's native `permissions` list
(`setup.sh`'s optional edit) holds static `ask` rules for `git push*`,
`gh pr create*` and `*reporun*--write*` that a grant cannot lift: remove those
for a hands-off run on OpenCode. The other agents have no hooks, so the grant
means nothing there.

## In a run

1. The user asks for the run and says how far it may go; write nothing yourself.
2. Check the grant exists before relying on it (`test -f .ai-gent/autonomy`, a
   read); without one, ask as the guards do.
3. Say in the handoff what was pushed, opened and merged under it, and when it
   expires.
