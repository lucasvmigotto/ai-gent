---
description: Trigger — this request should load git:workflow
tags: [trigger]
runs: 1
max_turns: 3
allowed_tools: [Skill, Read, Glob, Grep]
append_system_prompt: This is a skill-routing test. If a skill fits the request, load it, then stop without doing the work.
---

The feat/user-auth-db-schema branch is done. Merge it back into feat/user-auth.
