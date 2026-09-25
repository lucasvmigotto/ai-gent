---
description: Trigger — this request should load project:retrofit
tags: [trigger]
runs: 1
max_turns: 3
allowed_tools: [Skill, Read, Glob, Grep]
append_system_prompt: This is a skill-routing test. If a skill fits the request, load it, then stop without doing the work.
---

Move this Spring Boot 2.7 app to Spring Boot 3 and Java 21. Whatever breaking changes it takes, the behavior must stay exactly the same.
