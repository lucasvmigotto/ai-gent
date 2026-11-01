---
description: Trigger — this request should load security:scope
tags: [trigger]
runs: 1
max_turns: 3
allowed_tools: [Skill, Read, Glob, Grep]
append_system_prompt: This is a skill-routing test. If a skill fits the request, load it, then stop without doing the work.
---

We're about to start a security assessment of our own staging environment. Set up the authorization scope and allowed techniques before anything else runs.
