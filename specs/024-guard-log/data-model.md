# Data model — guard log

| Entity | Fields | Source |
|---|---|---|
| Guard row | at (ms), guard (`git` · `db` · `secrets` · `repo` · `containers`; `containers` is the `devcontainer` plugin's guard id), decision (`deny` · `ask`), entry (the table `match`, or `generic`), tool, verb (nullable), outcome (`asked`; `ran` or `declined` only if R2 allows) | a `classic.PreToolUse` result recognized by `decorate`, `e.tool`, `e.command`'s first words. No reason text: the shown reason is the table entry's fixed text |
| Guard log | rows[] (newest last, at most 200) | the `guardLog` atom |
| Summary | rows[] (newest N), frequent[] (entry, guard, decision, verbs with counts, count) | derived from the log; never stored |

## State atoms

| Key | Plugin | Type | Purpose |
|---|---|---|---|
| `guardLog` | each of `git`, `db`, `secrets`, `repo`, `devcontainer` (own atom each; `project` only reads) | `Shaped<GuardRow[]>` (tag `guard-log-1`) | the ring of rows |
| `guardsPaneIsOpen` | project | boolean | whether the pane is docked (named like `pipelineIsOpen` in `plugins/project/types/index.d.ts`) |

## Verb rule

`verb(command, tool)`: the first word of the command (a path is reduced to its last segment); for `git`, `gh`, `pass`, `podman`, `docker` also the second word. The first word is kept only if it matches `^[a-z][a-z0-9._-]{0,23}$` (lowercase start, at most 24 characters: a token such as `ghp_` plus 36 letters fails both and gives **no verb**); the second word only if it matches `^[a-z][a-z-]{0,23}$`, so an option (`git -C x push`) gives the verb `git`, a known limitation. A tool with no command (`Read`, `Edit`, …) has no verb. Tested with `shared/secret-cases.json`'s shapes as first word, argument and URL.

## `/guards` text

```
guards — 9 decisions this session (2 deny · 7 ask) · newest 1 (after /guards 1)
 14:02  ask   git      git push           git push publishes
Most frequent
 7×  git:workflow  git push publishes             ask    (git push ×7)
```
