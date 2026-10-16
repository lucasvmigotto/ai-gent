---
description: Trigger — a Jira transition loads repo:issues
tags: [trigger]
runs: 1
max_turns: 3
allowed_tools: [Skill, Read, Glob, Grep]
append_system_prompt: This is a skill-routing test. If a skill fits the request, load it, then stop without doing the work.
---

Move PROJ-142 to In Progress in Jira and assign it to me.
