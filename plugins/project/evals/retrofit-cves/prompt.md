---
description: Trigger — this request should load project:retrofit
tags: [trigger]
runs: 1
max_turns: 3
allowed_tools: [Skill, Read, Glob, Grep]
append_system_prompt: This is a skill-routing test. If a skill fits the request, load it, then stop without doing the work.
---

Update our dependencies and fix the CVEs in this service without changing how it behaves.
