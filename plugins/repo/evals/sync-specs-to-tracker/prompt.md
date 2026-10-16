---
description: Trigger — mirroring the specs into the tracker loads repo:sync, not repo:issues
tags: [trigger]
runs: 1
max_turns: 3
allowed_tools: [Skill, Read, Glob, Grep]
append_system_prompt: This is a skill-routing test. If a skill fits the request, load it, then stop without doing the work.
---

We have the Spec Kit specs for features 001 to 004. Create the Azure Boards work items for them, one per user story under each feature, and keep them in step as we build.
