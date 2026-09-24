---
description: Trigger — this request should load backend:domain
tags: [trigger]
runs: 1
max_turns: 3
allowed_tools: [Skill, Read, Glob, Grep]
append_system_prompt: This is a skill-routing test. If a skill fits the request, load it, then stop without doing the work.
---

Model the domain for this product: entities, invariants, lifecycles and business rules.
