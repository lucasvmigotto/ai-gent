---
description: Trigger — asking to hand bounded tasks to Codex loads agents:delegate, not agents:parallel or agents:orchestrate
tags: [trigger]
runs: 1
max_turns: 3
allowed_tools: [Skill, Read, Glob, Grep]
append_system_prompt: This is a skill-routing test. If a skill fits the request, load it, then stop without doing the work.
---

I have Codex installed and I'm close to my usage limit. Hand the fixture-writing and the survey draft tasks to Codex in their own worktrees, and you review what it sends back before anything lands.
