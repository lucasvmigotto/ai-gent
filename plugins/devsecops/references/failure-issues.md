# Failure issues — a red run that nobody is fixing becomes an issue

Two sources open them, both deduplicated so one failure is one issue:

- **The agent**, through `devsecops:watch`'s report: a red run the loop
  won't fix — flaky, runner or outage, already red on the default branch,
  or still red after three fix rounds (pipeline rule 8) — is handed back
  with `Hand back: issue`, and the stage opens or updates it through
  `repo:issues`.
- **The pipelines themselves**, for runs no agent is watching: scheduled
  runs and runs on `main`/release branches get a last job that runs only
  on failure and opens or updates the issue.

## The issue

- **Title:** `CI failing: <workflow> on <branch>` — stable, so the next
  failure finds it.
- **Label:** `ci-failure` (created once if the project agrees; otherwise
  match by title).
- **Body:** the run link, the commit, the failed job and step, the cause
  class and the few log lines that show it (never a whole log, never a
  secret's value).
- **Dedupe:** search open issues with the label or title first; when one
  exists, add a comment with the new run instead of opening another.
  Close it (or let the fix PR close it with `Closes #N`) once the
  workflow is green again.

## In the pipeline

**GitHub Actions** — a final job with `if: failure()` and
`permissions: issues: write`:

```yaml
  report-failure:
    needs: [build, test]                 # the jobs whose failure counts
    if: failure() && (github.event_name == 'schedule' || github.ref == 'refs/heads/main')
    runs-on: ubuntu-latest
    permissions: { issues: write }
    steps:
      - env:
          GH_TOKEN: ${{ github.token }}
          GH_REPO: ${{ github.repository }}
          TITLE: "CI failing: ${{ github.workflow }} on ${{ github.ref_name }}"
          RUN: ${{ github.server_url }}/${{ github.repository }}/actions/runs/${{ github.run_id }}
        run: |
          n=$(gh issue list --state open --search "\"$TITLE\" in:title" --json number -q '.[0].number')
          if [ -n "$n" ]; then gh issue comment "$n" --body "Failed again: $RUN"
          else gh issue create --title "$TITLE" --label ci-failure --body "Run: $RUN (commit ${{ github.sha }})"; fi
```

**GitLab CI** — a job with `when: on_failure` in the last stage, calling
the issues API with a project access token held in a masked variable
(the job token can't create issues). **Azure Pipelines** — a final job
with `condition: failed()` running `az boards work-item create` (or
adding a comment to the open one) with `System.AccessToken` and the
build service granted work-item rights. **Bitbucket** — an
`after-script` that checks `$BITBUCKET_EXIT_CODE` and calls the issues
API (or Jira's) with a repository variable. **Jenkins** — a
`post { failure { … } }` block.

Mark failures for scheduled and main/release runs only: a PR's red
checks belong to its author, who sees them on the PR.
