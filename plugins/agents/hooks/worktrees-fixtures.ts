// Recorded outputs the worktree board's tests read (specs/022-worktree-board):
// `wt.py list --json` as the script prints it (a unittest in scripts/test_wt.py
// fails when its keys or types differ from WT_LIST, so a change to the script
// shows here), and `gh pr list --state all --limit 100 --json
// number,headRefName,state,isDraft,statusCheckRollup` as gh prints it: a
// failing check run, a draft with a pending check, a merged pull request with
// passing checks, an open one with a commit status and no checks at all. Nothing
// here came from the network at test time.

export const WT_LIST = `[
  {
    "branch": "feat/us1",
    "path": ".worktrees/feat-us1",
    "dirty": true,
    "ahead": 3,
    "behind": 0
  },
  {
    "branch": "feat/us2",
    "path": ".worktrees/feat-us2",
    "dirty": false,
    "ahead": 2,
    "behind": 1
  },
  {
    "branch": "feat/us3",
    "path": ".worktrees/feat-us3",
    "dirty": false,
    "ahead": 4,
    "behind": 0
  },
  {
    "branch": "feat/us4",
    "path": ".worktrees/feat-us4",
    "dirty": false,
    "ahead": 0,
    "behind": 2
  },
  {
    "branch": "(detached)",
    "path": ".worktrees/scratch",
    "dirty": false,
    "ahead": null,
    "behind": null
  }
]
`

export const WT_LIST_NO_BASE = `[
  {
    "branch": "feat/us1",
    "path": ".worktrees/feat-us1",
    "dirty": false,
    "ahead": null,
    "behind": null
  }
]
`

export const GH_PR_LIST = `[
  {
    "headRefName": "feat/us1",
    "isDraft": false,
    "number": 41,
    "state": "OPEN",
    "statusCheckRollup": [
      { "__typename": "CheckRun", "name": "test", "status": "COMPLETED", "conclusion": "SUCCESS", "workflowName": "ci" },
      { "__typename": "CheckRun", "name": "lint", "status": "COMPLETED", "conclusion": "FAILURE", "workflowName": "ci" }
    ]
  },
  {
    "headRefName": "feat/us2",
    "isDraft": true,
    "number": 42,
    "state": "OPEN",
    "statusCheckRollup": [
      { "__typename": "CheckRun", "name": "test", "status": "IN_PROGRESS", "conclusion": "", "workflowName": "ci" }
    ]
  },
  {
    "headRefName": "feat/us3",
    "isDraft": false,
    "number": 43,
    "state": "MERGED",
    "statusCheckRollup": [
      { "__typename": "CheckRun", "name": "test", "status": "COMPLETED", "conclusion": "SUCCESS", "workflowName": "ci" },
      { "__typename": "CheckRun", "name": "docs", "status": "COMPLETED", "conclusion": "SKIPPED", "workflowName": "ci" }
    ]
  },
  {
    "headRefName": "feat/old",
    "isDraft": false,
    "number": 30,
    "state": "CLOSED",
    "statusCheckRollup": []
  },
  {
    "headRefName": "feat/us1",
    "isDraft": false,
    "number": 12,
    "state": "CLOSED",
    "statusCheckRollup": [
      { "__typename": "StatusContext", "context": "ci/legacy", "state": "ERROR", "targetUrl": "" }
    ]
  }
]
`
