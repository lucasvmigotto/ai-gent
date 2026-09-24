---
description: Trigger — this request should load devcontainer:setup
tags: [trigger]
runs: 1
max_turns: 3
allowed_tools: [Skill, Read, Glob, Grep]
append_system_prompt: This is a skill-routing test. If a skill fits the request, load it, then stop without doing the work.
---

This repo has a Spring Boot API in api/ and a React app in web/, and no devcontainer yet. Set up the development containers.
