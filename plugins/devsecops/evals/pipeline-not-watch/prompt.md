---
description: Trigger — designing a new pipeline loads devsecops:pipeline, not devsecops:watch
tags: [trigger]
runs: 1
max_turns: 3
allowed_tools: [Skill, Read, Glob, Grep]
append_system_prompt: This is a skill-routing test. If a skill fits the request, load it, then stop without doing the work.
---

We have no CI yet. Set up GitHub Actions for this repo: tests on pull requests and a release on main.
