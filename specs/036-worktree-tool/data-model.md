# Data Model: Worktree tool

| Entity | Fields | Where |
|---|---|---|
| Settings | `max_parallel` 3, `max_volumes` (default `max_parallel`), `volume_estimate_gb` 10, `disk_floor_gb` (optional) | `.ai-gent/parallel.yaml`, flat `key: number` lines |
| Port map | `{branch: offset}`, offsets multiples of 100 starting at 100, live branches only | `.worktrees/.ports.json` (sorted keys, indent 2, `\n` at end) |
| Baseline | `{"taken": "YYYY-MM-DDTHH:MM:SSZ", "external": [ids]}` indent 2 | `.worktrees/.containers-baseline.json` |
| Worktree row (list) | `branch` (`(detached)` if none), `path` (relative to root), `dirty` bool, `ahead` int or null, `behind` int or null | `list --json` |
| Feature row (plan) | `number`, `name`, `depends` [numbers], `sides` {frontend, backend, qa, status}, plus `todo` {side: status} and `waiting` [numbers] in ready/blocked | `plan --json` |
| Add result | `branch, base, path (absolute), compose_project, port_offset, labels{project, worktree}` | `add` stdout |
| Resources report | `project, disk{path, free_gb, floor_gb, below_floor}, images[], dangling_images[], volumes[{name, track, orphan}], containers[], external_new[{id,name}], baseline bool` | `resources --json` |

Names: `compose_project = <project>-<slug>`; `project = slug(root dir name)`; labels `me.lucasvmigotto.ai-gent.project=<project>`, `me.lucasvmigotto.ai-gent.worktree=<project>/<slug>`; compose label `com.docker.compose.project`.

Status words: `planned` 0, `in progress` 1, `partial` 1, `implemented` 2, `verified` 3 (prefix match after stripping `*_ `); anything else is not a side.

Local ignore entries written to `.git/info/exclude` under a `# agents:parallel` comment: `/.worktrees/`, `compose.worktree.yaml`.
