---
description: Trigger — this request should load db:connect
tags: [trigger]
runs: 1
max_turns: 3
allowed_tools: [Skill, Read, Glob, Grep]
append_system_prompt: This is a skill-routing test. If a skill fits the request, load it, then stop without doing the work.
---

Test whether you can connect to our staging database, and tell me whether it counts as local or remote.
