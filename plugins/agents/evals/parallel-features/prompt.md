---
description: Trigger — several independent features at once in worktrees loads agents:parallel, not agents:orchestrate
tags: [trigger]
runs: 1
max_turns: 3
allowed_tools: [Skill, Read, Glob, Grep]
append_system_prompt: This is a skill-routing test. If a skill fits the request, load it, then stop without doing the work.
---

Features 004 and 006 are both ready and do not depend on each other. Develop them at the same time, each on its own branch in a separate worktree, and merge them into dev when they are done.
