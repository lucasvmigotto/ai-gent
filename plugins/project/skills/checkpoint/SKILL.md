---
name: checkpoint
description: Keep a hand-off file for long or multi-repo work — .ai-gent/checkpoint.md, git-excluded, with the user's requests and standing rules verbatim, what is on hold, each repository's state and the next steps — rewritten at each milestone so another account, tool or session resumes exactly there. Use for "checkpoint your work", "I may switch accounts", "save where we are", "write a handoff".
---

# Checkpoint — a hand-off another agent can trust

A session can stop at any moment: a usage limit, a switch to another
account, a crash, a compaction that drops a detail. `project:recap`
reconstructs what happened from transcripts; the checkpoint records what
the transcripts lose — **the decisions still in force**. "Hold main until
X", "never mention Y on the page", "another session owns that repo" live
in one message among hundreds, and the next agent must not miss them.

```bash
python3 ../../scripts/checkpoint.py init     # skeleton + .git/info/exclude
python3 ../../scripts/checkpoint.py show     # the file, with staleness warnings
python3 ../../scripts/checkpoint.py stamp    # refresh "Updated" after a rewrite
```

(paths relative to this file). The file lives in the repository the
session runs in, at `.ai-gent/checkpoint.md`. `init` never overwrites it
and adds it to `.git/info/exclude` unless git already ignores it: it is
working material, never committed.

## When to write

Start it when the work will outlast the session — several steps, several
repositories, delegated agents — or as soon as the user mentions limits,
switching accounts or picking up later. Then rewrite it:

- after each milestone: a push, merge, promotion, tag or release, a
  delegated agent's report;
- when the user decides something or sets a rule;
- before starting anything long (a background agent, a migration, a
  watched pipeline), naming how to check on it;
- before the last message of a turn that leaves work open.

**Rewrite it whole**, then `stamp`. Never append a log: finished work
moves to `Done` (a few lines, newest first), obsolete state is deleted.
A checkpoint that grew by appending contradicts itself.

## What goes in

| Section | Content |
|---|---|
| `Asked` | The user's requests that are still open, verbatim, dated. |
| `Rules in force` | Constraints the user set, verbatim and dated: what to say or never say, who to ask, which conventions win. These outlive the session. |
| `Holds` | What must not be touched or must wait, and for what: "c4t main: hold until the 026 content is ready". |
| `State` | Per repository (path): branch, `HEAD` short hash, pushed or not, last tag, work in a worktree, a background agent or pipeline in flight and how to check it. |
| `Next` | Ordered steps; the first is actionable now. Say which need the user's go-ahead. |
| `Open questions` | What was asked and not answered yet. |
| `Done` | Short, newest first; enough to not redo it. |

- Quote the user. A paraphrase drifts; the next agent can't tell a rule
  from a guess.
- No secret values ever — names of pass entries, tokens or variables only.
- Facts with their evidence (a hash, a tag, a PR number), so the reader
  can verify instead of trust.
- Multi-repo work keeps **one** checkpoint, in the repository the session
  runs in; `State` lists the others by path.
- One hold per top-level `- ` bullet, and an empty section left empty (or
  `- none`). In Claude Code the `project` plugin's mod reads them: a band
  above the prompt shows them, and a push, merge, tag or release asks
  first while any is listed. Delete a hold the user lifted, so the band
  and the question stop.

## Reading one

`project:recap` reads the checkpoint before any transcript. A checkpoint
is the user's recorded intent, not an order to act: present `Next` and
the open questions, check `State` against the repositories (`show` warns
when commits are newer than the file or `HEAD` isn't in it), and ask
before resuming anything that `git:workflow` gates — a push, a merge, a
tag — since the go-ahead given in the old session doesn't carry over.
`Rules in force` and `Holds` do apply until the user lifts them.
