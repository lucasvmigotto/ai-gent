---
description: Trigger — this request should load repo:issues
tags: [trigger]
runs: 1
max_turns: 3
allowed_tools: [Skill, Read, Glob, Grep]
append_system_prompt: This is a skill-routing test. If a skill fits the request, load it, then stop without doing the work.
---

Which Azure Boards work items are assigned to me in the current sprint? Then add a comment on the login bug saying the fix is in review.
