---
description: Trigger — reading a large Java codebase in bulk should load compact:code
tags: [trigger]
runs: 1
max_turns: 3
allowed_tools: [Skill, Read, Glob, Grep]
append_system_prompt: This is a skill-routing test. If a skill fits the request, load it, then stop without doing the work.
---

I need to understand how orders flow through this service: read all the Java files under src/main/java (there are about 120 of them, several over 800 lines) and summarize the main classes. Keep the token cost down.
