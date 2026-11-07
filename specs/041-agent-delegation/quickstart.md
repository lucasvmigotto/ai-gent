# Quickstart: delegating a bounded task

Hand a bounded task to another code agent with `ai-gent delegate`.

## What you need

- The delegate agent's CLI on `PATH` and logged in.
- A git worktree for the task.
- A brief file.

Confirm the setup before running:

```
ai-gent delegate detect --agent <agent>
ai-gent delegate ready --agent <agent>
ai-gent delegate preflight --agent <agent> --worktree <wt>
```

`preflight` needs `--worktree` and proves a run can succeed in it. `--agent` defaults to `codex`: pass the same `--agent` to every command below that takes it.

## The loop

1. Write the brief. `--task-file` is the task text you wrote; `brief` prints the finished brief (with the hard limits) on stdout, so save that to a new file and use it in step 2:

```
ai-gent delegate brief --repo <repo> --branch <branch> --spec <dir> --tasks <tasks> --task-file <task-text> --allow '<glob>' > <brief-file>
```

2. Run it in a worktree:

```
ai-gent delegate run --agent <agent> --track <track> --worktree <wt> --brief-file <brief-file> --tasks <tasks>
```

3. Watch for the completion event and acknowledge it. `watch` keeps running after a completion, so run it under a monitor or in a second terminal (or use `watch --once` to print what is waiting and exit), and acknowledge in your own terminal:

```
ai-gent delegate watch
ai-gent delegate ack <event-id>
```

4. Look at `status` and `result`:

```
ai-gent delegate status <track> --agent <agent>
ai-gent delegate result <track> --agent <agent>
```

5. Run the supervisor's check on the diff:

```
ai-gent delegate check --worktree <wt> --allow <glob> --track <track>
```

6. Get a review from a different agent:

```
ai-gent delegate review --builder-track <track> --reviewer <other-agent> --spec <spec-file> --allow <glob>
```

7. Before landing: run the project's own tests or checks yourself, outside the worker, and read every review finding: fix or reject each one with a reason, then check again. `review` succeeding only means a review was produced. Then land it, or take it back:

```
ai-gent delegate commit --worktree <wt> --allow <glob> -m "<Conventional Commit>"
```

To take the task back instead of landing it, stop the run with `ai-gent delegate stop <track>`.

## Rules

- The delegate never commits or pushes; the supervisor stages by explicit path and commits.
- Nothing the delegate reports is trusted without the supervisor's own checks.
- One agent never reviews its own work.

## Not covered

This page does not explain the per-project opt-in, task planning, source verification, the workers pane or the restart continuation. The details live in the contract: `specs/041-agent-delegation/contracts/adapter.md`.
