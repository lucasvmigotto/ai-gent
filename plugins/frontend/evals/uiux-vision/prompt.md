---
description: Trigger — this request should load frontend:uiux
tags: [trigger]
runs: 1
max_turns: 3
allowed_tools: [Skill, Read, Glob, Grep]
append_system_prompt: This is a skill-routing test. If a skill fits the request, load it, then stop without doing the work.
---

Define how this app should look and read: layout, flows, visual direction and the tone of its copy.
