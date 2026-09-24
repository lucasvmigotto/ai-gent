---
description: Trigger — this request should load qa:strategy
tags: [trigger]
runs: 1
max_turns: 3
allowed_tools: [Skill, Read, Glob, Grep]
append_system_prompt: This is a skill-routing test. If a skill fits the request, load it, then stop without doing the work.
---

What should we test for this project, at which layer, and what are the quality gates?
