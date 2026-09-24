---
description: Trigger — this request should load devcontainer:infra
tags: [trigger]
runs: 1
max_turns: 3
allowed_tools: [Skill, Read, Glob, Grep]
append_system_prompt: This is a skill-routing test. If a skill fits the request, load it, then stop without doing the work.
---

For local development I need a Postgres database, a fake SMTP server to catch outgoing emails, and a local OIDC login provider.
