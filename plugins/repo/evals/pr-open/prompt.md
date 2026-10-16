---
description: Trigger — opening a PR loads repo:pr, not repo:issues
tags: [trigger]
runs: 1
max_turns: 3
allowed_tools: [Skill, Read, Glob, Grep]
append_system_prompt: This is a skill-routing test. If a skill fits the request, load it, then stop without doing the work.
---

My branch feat/42-export-csv is pushed. Open a pull request into dev using our template and link issue 42.
