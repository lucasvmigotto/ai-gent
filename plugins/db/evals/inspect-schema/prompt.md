---
description: Trigger — this request should load db:inspect
tags: [trigger]
runs: 1
max_turns: 3
allowed_tools: [Skill, Read, Glob, Grep]
append_system_prompt: This is a skill-routing test. If a skill fits the request, load it, then stop without doing the work.
---

Map this database's schema for me: tables, keys, indexes, triggers and anything that looks like a leftover backup table.
