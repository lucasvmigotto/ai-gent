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
does for its file); `ai-gent autonomy grant` does that for you. The repository
root is the one of the command's directory, so a run inside a worktree reads
that worktree's own file.

## Who writes it

Only the user, with `ai-gent autonomy grant` in their own terminal (or `!` in
the prompt). It writes the file atomically in exactly the form above (mode
0600), adds it to `.git/info/exclude`, and has no default grant: the expiry
(`--for 30m|4h|1d` or `--until <local datetime>`; at most 30 days, not in the
past) and the actions (`--actions push,pr,merge,issues`) are always chosen.
`--branches` defaults to `develop,dev`, and a grant never covers `main`,
`master`, `homolog` or `staging` (the command refuses, exit 2). With no flags
on a terminal it asks: a checklist of actions, the branches, the expiry (also
shown in UTC), then a summary and a yes/no; nothing is written without the yes.
`--yes` only skips that final question. `grant` and `revoke` refuse (exit 2,
nothing written) unless stdin and stdout are real terminals and a controlling
terminal exists, so an agent, whose shell has pipes and no terminal, cannot run
them, with or without `--yes`. Only `show` runs anywhere. A grant that replaces
an active one prints a notice naming the old expiry, branches and actions, and a
branch is refused as protected whatever its spelling (`Main`, `origin/main`,
`refs/heads/main`).

```txt
ai-gent autonomy grant --for 4h --actions push,pr --yes
ai-gent autonomy show [--json]   # active grant, expiry in UTC and local time, time left
ai-gent autonomy revoke          # removes the file
```

The terminal requirement is the real defence. The `git` guard adds a text check
on top: it denies a command that has `ai-gent` and `autonomy` as words unless
`autonomy` is followed only by `show`, `status`, `--help` or `-h`, and any
`autonomy ... grant|revoke`, on the command as written and with quotes,
backslashes and `$(...)` stripped. A text check alone can never be complete,
because a shell can build any command line at run time; that is why the
terminal check exists. The guard also denies one that writes, moves or
removes the file (a redirect, `rm`, `mv`, `sed -i`, `tee`, …; reading it is
fine) and an `Edit`, `Write` or similar aimed at it. Never create or change it
yourself, never suggest an expiry you chose: ask the user for the scope and the
time, and give them the command to run.

### Actions

One vocabulary; the file always stores the first word, and the command accepts
the aliases.

| Action | Alias | On each platform | Lifted today |
| --- | --- | --- | --- |
| `push` | | `git push`, any remote | any remote |
| `pr` | `mr` | pull request on GitHub, Azure DevOps and Bitbucket; merge request on GitLab | GitHub only |
| `merge` | | merging that pull or merge request | GitHub only (`reporun cli github`) |
| `issues` | `workitems` | issues on GitHub and GitLab; Boards work items on Azure DevOps; Jira and Bitbucket issues | GitHub only (`reporun cli github`) |

A grant for an action on a platform whose rules do not exist is accepted, and
`grant` and `show` say it lifts nothing there. GitLab and Azure DevOps rules do
not exist yet.

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

1. The user asks for the run and says how far it may go; they run
   `ai-gent autonomy grant` themselves, you write nothing.
2. Check the grant exists before relying on it (`test -f .ai-gent/autonomy`, a
   read); without one, ask as the guards do.
3. Say in the handoff what was pushed, opened and merged under it, and when it
   expires.
