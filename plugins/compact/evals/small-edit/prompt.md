---
description: Trigger — a small edit to one short file should not load compact:code
tags: [trigger]
runs: 1
max_turns: 3
allowed_tools: [Skill, Read, Glob, Grep]
append_system_prompt: This is a skill-routing test. If a skill fits the request, load it, then stop without doing the work.
---

In README.md, the second heading says "Instalation". Fix the typo.
