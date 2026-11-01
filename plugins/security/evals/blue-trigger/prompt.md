---
description: Trigger — this request should load security:blue
tags: [trigger]
runs: 1
max_turns: 3
allowed_tools: [Skill, Read, Glob, Grep]
append_system_prompt: This is a skill-routing test. If a skill fits the request, load it, then stop without doing the work.
---

What would actually catch this attack? Set up detection and alerting for the login brute-force issue we found.
