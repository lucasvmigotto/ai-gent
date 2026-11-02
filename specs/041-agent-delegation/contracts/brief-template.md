# Brief template

```
You are a contributor to <repo> (this directory is a git worktree on branch <branch>).
Read first, in order: AGENTS.md; <worker pack: shared/delegate-worker-rules.md and the skills/references this task needs, e.g. git:workflow>.

Task: <spec, task ids and the task text, verbatim>.
allowed_paths: <list of globs; touching anything else fails review automatically>.
Verify with: <the task's own commands, targeted tests only>.

Hard limits: no credentials or secrets; no database or container tools; no network (you have none); no edits to skills, plugins, hooks, shared references, AGENTS.md or CLAUDE.md; no version bumps, tags or CHANGELOG edits; no trailers or markers of any kind in commits; no --no-verify; no force-push; stage by explicit path; never delete anything you did not create.
Environment: TMPDIR=<dir>; build output outside the worktree.
Environment failures (denied sockets, caches, missing tools) go in your final message, never in a repository file.
Leave your changes in the working tree and stop. Do not commit or push (the supervisor commits).

Final message (short): per task done or skipped and why, commit hashes, and "Rules I found hard to follow".
```

Rules the supervisor checks on every diff (FR-011): paths inside `allowed_paths`; no trailers; no new prose about environment failures; tests not weakened; `tasks.md` marks match evidence.
