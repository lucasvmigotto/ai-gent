---
description: Trigger — this request should load project:recap, not project:status
tags: [trigger]
runs: 1
max_turns: 3
allowed_tools: [Skill, Read, Glob, Grep]
append_system_prompt: This is a skill-routing test. If a skill fits the request, load it, then stop without doing the work.
---

I haven't touched this repo in about three weeks. Catch me up: where did we stop last time, and what has changed since?
