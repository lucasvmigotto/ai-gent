---
description: Trigger — storing a token loads secrets:pass
tags: [trigger]
runs: 1
max_turns: 3
allowed_tools: [Skill, Read, Glob, Grep]
append_system_prompt: This is a skill-routing test. If a skill fits the request, load it, then stop without doing the work.
---

I want ai-gent to use my GitHub token from pass instead of a .env file. Where should it go and how do I add it?
