---
description: Trigger — this request should load devsecops:watch
tags: [trigger]
runs: 1
max_turns: 3
allowed_tools: [Skill, Read, Glob, Grep]
append_system_prompt: This is a skill-routing test. If a skill fits the request, load it, then stop without doing the work.
---

I just pushed to main and the GitHub Actions release run went red. Can you find out why it failed and keep an eye on the deploy after it?
