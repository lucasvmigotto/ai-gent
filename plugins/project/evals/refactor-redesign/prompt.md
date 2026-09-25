---
description: Trigger — this request should load project:refactor
tags: [trigger]
runs: 1
max_turns: 3
allowed_tools: [Skill, Read, Glob, Grep]
append_system_prompt: This is a skill-routing test. If a skill fits the request, load it, then stop without doing the work.
---

Plan a major refactor of this codebase. Keep the core business rules, but rethink the workflows that are clumsy and the parts of the design that slow us down.
