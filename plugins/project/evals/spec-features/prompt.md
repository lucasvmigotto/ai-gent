---
description: Trigger — this request should load project:spec
tags: [trigger]
runs: 1
max_turns: 3
allowed_tools: [Skill, Read, Glob, Grep]
append_system_prompt: This is a skill-routing test. If a skill fits the request, load it, then stop without doing the work.
---

docs/product/brief.md is done. Break the project into Spec Kit features with plans and tasks.
