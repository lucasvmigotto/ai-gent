---
description: Trigger — this request should load devcontainer:proxy
tags: [trigger]
runs: 1
max_turns: 3
allowed_tools: [Skill, Read, Glob, Grep]
append_system_prompt: This is a skill-routing test. If a skill fits the request, load it, then stop without doing the work.
---

In production nginx serves our API under /api and the frontend under /app. Reproduce that path prefix setup locally so I can debug a cookie path bug.
