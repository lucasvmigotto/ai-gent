---
name: issues
description: Find, read, create and update issues and work items — GitHub Issues, Azure Boards (WIQL, states, links), GitLab, Bitbucket and Jira (JQL, transitions) — through the platform's CLI or its REST API, with tokens from pass and every change asked first. Use for "find the issue about X", "what's assigned to me", "open a work item for this", "comment on PROJ-12", "move it to Done".
---

# Issues and work items

Read `../../references/runner.md` first: every call goes through
`reporun`, the CLI when it's installed and the REST API otherwise. Then
the tracker's file in `../../references/platforms/` (`github.md`,
`azure-devops.md`, `gitlab.md`, `bitbucket.md`, `jira.md`).

## Which tracker

1. `reporun detect` — the code platform from the remote, its CLI and
   token, and Jira when it's configured.
2. The tracker is the platform's own (GitHub Issues, Azure Boards, GitLab
   issues, Bitbucket issues) unless the ids say otherwise: `PROJ-123`
   keys mean Jira, `AB#123` means Azure Boards from GitHub. When both a
   platform tracker and Jira are set up and the request doesn't say,
   ask which one.
3. No token and no signed-in CLI: give the user the `pass insert -m
   ai-gent/<platform>/…` to run in their terminal (`secrets:pass`) — or,
   for a public GitHub repository, read without one.

## Read — freely

- **Find:** the platform's search — `gh issue list --search`, WIQL
  (`boards query`, `POST _apis/wit/wiql`), GitLab `search=`, BBQL `q=`,
  JQL. Start narrow (open, this project, recent) and widen only if
  nothing fits. Common asks: "assigned to me" (`@me`,
  `currentUser()`, `assignee=@me`), "in this sprint" (Azure
  `@CurrentIteration`, Jira `sprint in openSprints()`).
- **Read one:** with its comments and links (relations, parent, linked
  PRs) when the question needs them.
- **Report** as a short table — id, title, state, assignee, updated —
  and the link to each. Quote what the issue says; don't invent fields
  the platform didn't return.

## Write — only the change the user asked for

Creating, commenting, editing fields, labels, assignees, transitions,
links and closing are writes: `reporun … --write`, which the `repo` guard
confirms with the user. Before each one:

1. **Say exactly what will change** — the item, the field and the new
   value, or the full text of the comment or the new item.
2. **Use the project's conventions**: its issue templates
   (`.github/ISSUE_TEMPLATE/`, `.gitlab/issue_templates/`), the work item
   type and the area and iteration paths it uses, required fields (read
   one existing item of that type first), the labels that exist.
3. **Never on your own initiative**: no closing an issue because a commit
   mentions it, no comment summarizing work, no reassigning — unless
   asked for that specific action (`git:workflow`).
4. **Transitions** follow the workflow: read the allowed transitions
   (Jira `…/transitions`, the Azure work item type's states) instead of
   guessing a state name.
5. After the write, read the item back and report the link.

Rich text: Azure DevOps fields take HTML; Jira Cloud's v3 API takes
Atlassian Document Format — use `/rest/api/2` for plain text; GitHub,
GitLab and Bitbucket take Markdown.

## Linking to code

Issue references in commits and PRs belong to `git:workflow` (footers,
closing keywords, when closing happens); this skill finds and confirms
the ids it uses. Opening or reviewing the pull request itself is
`repo:pr`.

## Handoff

What was found or changed, with links; any write the user still has to
confirm; and the next step (a branch named after the item —
`git:workflow`, the PR — `repo:pr`).
