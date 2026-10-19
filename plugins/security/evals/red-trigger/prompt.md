---
description: Trigger — this request should load security:red
tags: [trigger]
runs: 1
max_turns: 3
allowed_tools: [Skill, Read, Glob, Grep]
append_system_prompt: This is a skill-routing test. If a skill fits the request, load it, then stop without doing the work.
---

Review our app's authentication and authorization code for exploitable security weaknesses — broken access control, injection — and report ranked findings with fixes.
