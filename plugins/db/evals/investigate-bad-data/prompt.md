---
description: Trigger — this request should load db:investigate
tags: [trigger]
runs: 1
max_turns: 3
allowed_tools: [Skill, Read, Glob, Grep]
append_system_prompt: This is a skill-routing test. If a skill fits the request, load it, then stop without doing the work.
---

Since last week some orders show a total that doesn't match the sum of their items. Find out why those records are wrong.
