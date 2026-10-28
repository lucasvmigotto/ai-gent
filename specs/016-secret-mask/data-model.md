# Data model — secret mask

## Pattern (`shared/secret-patterns.json`)

| Field | Meaning |
|---|---|
| `kind` | `private-key`, `assignment`, `auth-header`, `github-token`, `api-key`, `aws-key`, `slack-token`, `url-credentials` |
| `source` | the regex, in the subset Python `re` and JS `RegExp` share |
| `flags` | `i`, `s` (no inline flags) |
| `replacement` | e.g. `\1\2<redacted>`, `<token>`, `<private key>`; group references translated per language |
| `entry` | optional pass entry hint (`ai-gent/github/pat`) from `shared/secrets.md`'s entry table |
| `toggle` | optional `userConfig` key that disables it (`maskAssignments` for `assignment`) |
| `skipIn` | optional places it is left out of: `skill` (a skill's written body keeps prose and examples whole) |

Initial content is exactly `SECRET_PATTERNS` in `plugins/project/scripts/sessions.py` at the time of building, split by kind (its token pattern holds GitHub, `sk-`, AWS and Slack shapes in one alternation; splitting it keeps the toast's kind precise without changing what matches).

## Mask result (per event, never stored)

| Field | Meaning |
|---|---|
| `text` | the masked text handed to `next` |
| `counts` | `{kind: n}` — what the toast says |
| `hints` | distinct `entry` values for the kinds hit |

## Where each event's text is

| Event | Field rewritten | Notes |
|---|---|---|
| `prompt.submit` | `text` | skipped when `command` is set; `isMeta` starting a turn takes no rewrite |
| `command.run` | `args` | before the command runs; its rows and history line keep the masked args |
| `skill.prompt` | `text` | the skill's rendered body |
