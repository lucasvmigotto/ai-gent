---
description: Trigger — this request should load project:status
tags: [trigger]
runs: 1
max_turns: 3
allowed_tools: [Skill, Read, Glob, Grep]
append_system_prompt: This is a skill-routing test. If a skill fits the request, load it, then stop without doing the work.
---

We've been working through the product pipeline on this repo for a while. Where are we, and which stage should I run next?
