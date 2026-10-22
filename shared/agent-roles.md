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
| `scout` | searches, file maps, triage, "where is X", following CI runs and reporting them | edits, decisions, commits, pushes |

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

## Skills that run as a role

Some skills always run as a role, in a context of their own, instead of on
whatever model the session uses. Claude Code is the reference: the skill's
frontmatter says it (`model: haiku` for `scout`, `context: fork` for its own
context, `allowed-tools` for the read-only commands it may run unasked), and
the session gets back only its report.

| Skill | Role | Why |
|---|---|---|
| `devsecops:watch` | `scout`, forked | waiting costs nothing (the platform's watch runs in the background); reading results and classifying a failure is triage. A fix goes back to the stage that pushed, on its own model (pipeline rule 8) |

Committing, merging, pushing and fixing stay with the stage doing the
work: they're cheap commands whose cost is the judgement around them
(what goes in the commit, whether the change is ready, `git:workflow`'s
asks), which a fork without the conversation can't make.

How each agent gets the same:

| Agent | Small model for a skill | Own context | ai-gent does |
|---|---|---|---|
| **Claude Code** (reference) | `model: haiku` in SKILL.md | `context: fork` (background by default) | in the skill's frontmatter |
| opencode | `model: provider/model` on the command | `subtask: true` on the command | `setup.sh` writes `subtask: true`, and the model from `AI_GENT_OPENCODE_MODEL_SMALL` when set |
| Codex | `model`, `model_reasoning_effort` in a custom agent (`.codex/agents/<name>.toml`) | the agent runs as a subagent | nothing yet: add a `scout` agent with the small model and ask for it by name |
| Cursor | `model: fast` (or a model id) in a subagent (`.cursor/agents/<name>.md`) | subagents have their own context | nothing yet: a `scout` subagent with `model: fast` |
| Antigravity | `model: flash` (small) or `pro` (strong) in a subagent (`.agents/agents/<name>.md`); no mid-tier builder model | subagents have their own context | nothing yet: a `scout` subagent with `model: flash` |
| GitHub Copilot | `model` in a prompt file (`.github/prompts/*.prompt.md`) or a custom agent | a custom agent | nothing yet |
| Kiro | `model` in a custom agent (`.kiro/agents/`) | subagents | nothing yet |
| Amp | no per-skill model; its built-in Search subagent already runs small | built-in subagents | nothing to do |
| Windsurf | no per-workflow model | — | runs on the session's model |

Where an agent can't pin a model, the skill still works: it runs on the
session's model, in the session's context, and its report says the same
things.

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
