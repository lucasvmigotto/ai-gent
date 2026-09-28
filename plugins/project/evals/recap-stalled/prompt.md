---
description: Trigger — this request should load project:recap, not project:status
tags: [trigger]
runs: 1
max_turns: 3
allowed_tools: [Skill, Read, Glob, Grep]
append_system_prompt: This is a skill-routing test. If a skill fits the request, load it, then stop without doing the work.
---

My last session ran out of tokens in the middle of the work and I edited a couple of files myself afterwards. Pick up where you left off.
