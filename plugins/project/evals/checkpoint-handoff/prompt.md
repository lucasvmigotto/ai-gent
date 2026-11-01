---
description: Trigger — this request should load project:checkpoint, not project:recap
tags: [trigger]
runs: 1
max_turns: 3
allowed_tools: [Skill, Read, Glob, Grep]
append_system_prompt: This is a skill-routing test. If a skill fits the request, load it, then stop without doing the work.
---

I'm close to my weekly limit and may have to continue this on my other account. Save where we are, with everything I decided so far, so whoever picks it up doesn't redo or undo anything.
