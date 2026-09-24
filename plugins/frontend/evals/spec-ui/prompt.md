---
description: Trigger — this request should load frontend:spec
tags: [trigger]
runs: 1
max_turns: 3
allowed_tools: [Skill, Read, Glob, Grep]
append_system_prompt: This is a skill-routing test. If a skill fits the request, load it, then stop without doing the work.
---

The UX vision is written. Turn it into the frontend specs: design system and per-screen UI specs.
