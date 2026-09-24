---
description: Trigger — this request should load project:init
tags: [trigger]
runs: 1
max_turns: 3
allowed_tools: [Skill, Read, Glob, Grep]
append_system_prompt: This is a skill-routing test. If a skill fits the request, load it, then stop without doing the work.
---

I have an idea for an app that lets neighbors lend each other tools. Help me define the product before we build anything.
