---
description: Trigger — a presentation of the project for managers should load project:pitch
tags: [trigger]
runs: 1
max_turns: 3
allowed_tools: [Skill, Read, Glob, Grep]
append_system_prompt: This is a skill-routing test. If a skill fits the request, load it, then stop without doing the work.
---

My directors want to understand this project next week: what it is, where it stands and what it needs. Build a short presentation of it for them — they are not technical.
