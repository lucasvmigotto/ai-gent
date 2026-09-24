# Linking issues and work items — only when asked

Part of `git:workflow` (`../SKILL.md`); its non-negotiable rules apply here too.

Do this only when the user asks ("this closes #12", "link the issue", "find the related issue"). Never add issue references on your own initiative.

## 1. Find the candidate ids, cheapest first

1. **Ids the user gave** in the request.
2. **The branch name — check it, then ask.** Extract likely ids from the current branch (and the context branch being merged):
   - forms that are usually ids: a leading number segment (`feat/123-login` → `123`), `#123`, `gh-123`/`issue-123`, Jira-style keys (`PROJ-123`), Azure Boards `AB#123`;
   - forms that usually aren't: numbers inside words or versions (`oauth2`, `v2`, `http2`, `k8s`, `s3`), dates (`2026-09`), port-like or long numbers.
   Then **ask**: *"The branch `feat/123-login` has `123` — is that the issue/work item this closes?"* Use it only after the user confirms.
3. **Earlier commits on the branch** that already reference an id (`git log <parent>..HEAD --grep='#[0-9]'`).
4. **Look it up on the remote**, if the user wants a search:
   - detect the platform from `git remote get-url origin` — `github.com` → GitHub, `gitlab` → GitLab, `dev.azure.com`/`visualstudio.com` → Azure DevOps, `bitbucket.org` → Bitbucket, another host → possibly Forgejo/Gitea (confirm);
   - use its CLI if installed and authenticated (`gh issue list --search`, `glab issue list --search`, `tea issues`, `az boards query`), otherwise its REST API with `curl` **only** with a token already present in the environment (`GH_TOKEN`/`GITHUB_TOKEN`, `GITLAB_TOKEN`, …) — public repositories may not need one;
   - **never ask the user to paste a token into the chat**; with no access, ask the user for the id instead.
5. Show the candidates (id, title, state) and ask which ones this change **closes** and which it only **relates to**.

## 2. Write the reference

- As a **footer** of the commit that completes the work — not in the subject:
  ```
  fix(auth): refresh token before expiry

  Closes #123
  Refs #98
  ```
- Closing keywords by platform:
  - **GitHub, GitLab, Forgejo/Gitea, Bitbucket Cloud:** `Closes #N` / `Fixes #N` / `Resolves #N`; cross-repo `owner/repo#N` (GitHub/Gitea), `group/project#N` (GitLab).
  - **Azure DevOps:** `AB#N` links a work item from GitHub-hosted code; in Azure Repos, `#N` mentions link work items, and closing on completion is a PR setting — check how the organization has it configured.
  - **Jira:** the issue key (`PROJ-123`) links; transitions (`PROJ-123 #done`) work only if smart commits are enabled.
  - `Refs #N` (or just `#N`) links without closing.
- **When it closes:** closing keywords act only when the commit or PR reaches the repository's **default branch**. Merging into `dev` or a feature branch links the issue but won't close it — say so whenever the target isn't the default branch.
- If the user asks for a PR, put the same `Closes #N` lines in the PR description (the most reliable place for closing on merge).
- Never close, comment on, label, assign or otherwise change an issue through a CLI or API unless the user asks for that specific action.
