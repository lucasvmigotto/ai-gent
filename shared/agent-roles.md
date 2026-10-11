# Agent roles

One table turns role names into models, for every coding agent. Specs, cards
and skills name **roles**, never models; this file (or a project override)
resolves them. Changing a model or adding an agent edits this file only.

## Roles

| Role | Does | Never does |
|---|---|---|
| `thinker` | specs, plans, cards, design decisions, the merge/no-merge call, the report to the user | bulk reading, mechanical edits, long implementations |
| `builder` | implements a card: code, tests, extraction, drafts, renders | merges, pushes, releases, scope changes |
| `reviewer` | checks a builder's diff against the card and a rubric, re-runs the acceptance commands, gap analysis | trusts the builder's report without running the checks |
| `scout` | searches, file maps, triage, "where is X" | edits, decisions |

The `reviewer` runs in a **fresh context** (a new subagent or session), never in
the builder's: a model grading its own work misses its own blind spots.

## Default mapping

| Role | Claude (default) | Codex / OpenAI | Other agents |
|---|---|---|---|
| `thinker` | Opus | the strongest reasoning model, high reasoning effort | strongest tier |
| `builder` | Sonnet | the fast coding model, medium effort | mid tier |
| `reviewer` | Opus, fresh context | strongest tier, separate session | strongest tier, separate session |
| `scout` | Haiku (or the Explore agent) | the small model | small tier |

Use the current generation of each family. Agents without subagents run the
roles one after another in separate sessions.

## Effort

A card may carry `Effort: low|high`, mapped to the agent's effort or reasoning
setting. `low` for mechanical work (renames, renders, rewording, applying a
known fix); `high` for design, tricky debugging and reviews.

## Project override

A project may override the mapping in `.ai-gent/orchestrate.yaml`:

```yaml
agent: claude            # claude | codex | opencode | <name>
roles:
  thinker:  {model: opus,   effort: high}
  builder:  {model: sonnet, effort: medium}
  reviewer: {model: opus,   effort: high, fresh_context: true}
  scout:    {model: haiku,  effort: low}
max_parallel: 4
```

## Marking tasks

Each task in `tasks.md` and each card carries:

```
Executor: builder · Reviewer: thinker · Effort: low
```

A task with no tag defaults to `Executor: builder`, `Reviewer: thinker`.
