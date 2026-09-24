---
description: Trigger — this request should load devsecops:iac
tags: [trigger]
runs: 1
max_turns: 3
allowed_tools: [Skill, Read, Glob, Grep]
append_system_prompt: This is a skill-routing test. If a skill fits the request, load it, then stop without doing the work.
---

The architecture picked AWS with Postgres on RDS and ECS services. Write the Terraform for staging and production.
