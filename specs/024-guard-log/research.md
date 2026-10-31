# Research — guard log

Items marked *to verify* need a run on Claude Code 2.1.312 (the pin 012 sets) before building; the decision says what happens either way. Where this file says 2.1.312 it means the CI pin; the mod API types read on 2026-10-31 were 2.1.316's, and a probe of the events used here passed `claude plugin validate` on 2.1.312.

## R1 — One command reading five plugins' records (decided; one run to verify)

**Checked**: each plugin keeps its own `$.state` atoms declared in its `types/index.d.ts` (`plugins/git/types/index.d.ts` declares `git.hygiene` and `git.toasted`; `plugins/devcontainer/types/index.d.ts` declares `devcontainer.toasted`; `db`, `secrets` and `repo` have no `types/` yet). A plain `/<name>` command has no plugin prefix (017), so five plugins registering `/guards` would collide. The kit's `$.state.get` takes `{ plugin, key }` as the owner's contract declares them in `PluginState`, only the owner writes, and the engine lays a dependency's contract into the dependent's `.claude-plugin/types/<plugin>/index.d.ts`. On 2.1.312, `claude plugin validate` accepted a `dependencies` entry naming a plugin that was not installed, with no error or warning (probe, 2026-10-31).

**Decision (the maintainer, 2026-10-31)**: one command, owned by `project`, which lists `git`, `db`, `secrets`, `repo` and `devcontainer` under `dependencies` and reads each `guardLog` atom with `$.state.get`. The shared-atom and per-plugin-command fallbacks are dropped. *To verify* (probe in the build's first task, on the pin): what `$.state.get` returns or throws for an atom whose plugin is not loaded; `project` must catch it per plugin and print `not reporting: <plugins>`. If the engine instead refuses to load `project` when a dependency is absent, the dependencies become a hard requirement and the decision is revisited with the maintainer; the transcript script (User Story 3) needs no engine state and remains the other route.

## R2 — Can an ask's outcome be known?

**Checked**: at `classic.PreToolUse`, `next(e)` returns `{ ask: … }` and the person's answer comes later, in the engine's own permission prompt (015, decision: "asks keep the engine's own permission prompt"). A completed call reaches `tool.call` hooks after `next` (the git module's `tool.call` hook runs its refresh after `await next(e)`).

**Decision**: a row says `asked`. *To verify*: whether `classic.PreToolUse` and `tool.call` events share a call id, or the same tool and verb in order, so a later `tool.call` can mark the row `ran`. If an id exists, the row gains `ran` or `declined`; otherwise no outcome is claimed (never inferred from timing).

## R3 — Is the verb enough to see an over-strict guard?

**Checked**: the guards' reasons are keyed by `Entry.match` (`shared/guard-ux.ts`), so counting by entry and verb answers "which rule, for which command"; but the raw reasons are **not** fixed sentences: `plugins/git/hooks/guard.sh:83-92` interpolates `$branch`, `plugins/db/hooks/guard.sh:94-102` interpolates `$tool`, `plugins/secrets/hooks/guard.sh:80-83` interpolates `$sub`. So a row stores the entry `match` (and nothing at all for a `generic` reason), and `/guards` shows the entry's own fixed text. The command's arguments add little and carry the risk in FR-003.

**Decision**: verb only, under the tightened rule in data-model.md (a first word that could be a token yields no verb). If a case shows the verb isn't enough (say, `git push` on which branch), the branch name is the next candidate, never the command.
