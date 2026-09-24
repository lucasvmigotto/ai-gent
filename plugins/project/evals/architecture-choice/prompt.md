---
description: Trigger — this request should load project:architecture
tags: [trigger]
runs: 1
max_turns: 3
allowed_tools: [Skill, Read, Glob, Grep]
append_system_prompt: This is a skill-routing test. If a skill fits the request, load it, then stop without doing the work.
---

We expect about 50k daily users and have a two-person team. Which cloud, database and API style should this system use?
