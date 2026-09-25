---
description: Trigger — this request should load db:review
tags: [trigger]
runs: 1
max_turns: 3
allowed_tools: [Skill, Read, Glob, Grep]
append_system_prompt: This is a skill-routing test. If a skill fits the request, load it, then stop without doing the work.
---

Review how our application uses its database: ORM mappings against the real schema, migrations, missing indexes and N+1 queries.
