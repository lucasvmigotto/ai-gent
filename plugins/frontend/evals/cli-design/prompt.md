---
description: Trigger — this request should load frontend:tui
tags: [trigger]
runs: 1
max_turns: 3
allowed_tools: [Skill, Read, Glob, Grep]
append_system_prompt: This is a skill-routing test. If a skill fits the request, load it, then stop without doing the work.
---

Design and build the command-line tool for this service: subcommands, --json output and proper exit codes.
