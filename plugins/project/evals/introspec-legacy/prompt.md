---
description: Trigger — this request should load project:introspec
tags: [trigger]
runs: 1
max_turns: 3
allowed_tools: [Skill, Read, Glob, Grep]
append_system_prompt: This is a skill-routing test. If a skill fits the request, load it, then stop without doing the work.
---

This is a legacy Django app with no documentation. Reverse-engineer a full specification of what it does, including its data model and integrations.
