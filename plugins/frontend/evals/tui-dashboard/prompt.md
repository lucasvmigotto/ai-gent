---
description: Trigger — this request should load frontend:tui
tags: [trigger]
runs: 1
max_turns: 3
allowed_tools: [Skill, Read, Glob, Grep]
append_system_prompt: This is a skill-routing test. If a skill fits the request, load it, then stop without doing the work.
---

Build a terminal dashboard, a TUI, that lists today's orders from our API and lets me cancel one from the keyboard.
