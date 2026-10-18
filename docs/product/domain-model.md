Reconstructed by project:introspec on 2026-10-18 from 384c771

# Domain model (as implemented) — ai-gent docs site and runners

Status: Draft

Kept minimal on purpose: these are tools, not a business system. There is no server, no shared database and no personal data of users; the "domain" is the toolkit's own vocabulary and the safety rules the runners enforce. Names follow the brief's glossary and the code.

## Contexts

| Context | What it owns | Code |
|---|---|---|
| Documentation | Plugin, Skill, Version, Page, Flow step — read-only projections of the repository at build time | `docs/site/` (features 001-005) |
| Installation | Agent, Link, Generated entry, Targets state | `install.sh`, `setup.sh` (006) |
| Credentials | Entry (pass) | `shared/passstore.py` (007) |
| Database access | Profile, Classification, Statement class, Plan, Backup, Log entry | `dbrun.py` (008) |
| Platform access | Platform, Token entry, Batch, Chain | `reporun.py` (009) |
| Spec tracking | Feature, Story, Task, Item, Tracker record | `specsync.py` (010) |
| Parallel work | Track (worktree) | `wt.py` (011) |

Contexts meet only through files and process boundaries: Credentials is a library the two access contexts import; Spec tracking produces a batch that Platform access executes; Parallel work and Spec tracking both read `specs/README.md`, each with its own parser [OBSERVED: plugins/repo/scripts/specsync.py:141-160; plugins/agents/scripts/wt.py:87-107].

## Invariants (enforced in code)

1. A database is `local` only when proven on this run; anything else is treated as its declared class, and an unproven "local" as production [OBSERVED: dbrun.py:471-526].
2. Only a proven local user database is written, only through a confirmed, unmodified plan, after a backup where the engine supports one [OBSERVED: dbrun.py:1303-1324,1371-1382,1462-1478].
3. A statement is a read only when it matches the read rules; unknown is a write [OBSERVED: dbrun.py:660-693].
4. A platform change needs `--write`; unknown CLI calls are writes [OBSERVED: reporun.py:278-309,430-436] — with the read-verb exception found in `specs/009-repo-runner/spec.md` Edge Cases.
5. A token is sent only to its platform's API host [OBSERVED: reporun.py:485-487].
6. A credential value never appears in output, logs, plans or errors [OBSERVED: dbrun.py:838-854; reporun.py:140-145; passstore.py:20-21].
7. setup.sh never replaces what it didn't create without `--force`, and then backs it up [OBSERVED: setup.sh:310-340,399].
8. A skill id `<plugin>-<skill>` is unique and is its URL slug [OBSERVED: docs/site/tests/content.test.ts:13].

## Lifecycles

- **dbrun plan**: recorded → backed up → committed | rolled back → (restored) — `specs/008-db-runner/data-model.md`.
- **Tracker item**: todo → doing → review → done — `specs/010-spec-sync/data-model.md`.
- **Track**: added → (work, land on dev) → removed; removal refused while dirty or unlanded — `specs/011-worktrees/spec.md`.

## Personal data

None collected by the site (no analytics, no accounts) [OBSERVED: no tracking code in docs/site/src]. dbrun handles third-party personal data only transiently and masks it by column and pattern (names, emails, documents such as CPF/CNPJ, phones, cards, birth dates, addresses, IPs) [OBSERVED: dbrun.py:730-836]. Its command log records statements and row counts, not result rows [OBSERVED: dbrun.py:869-889].

## Events, aggregates, authorization

N/A — no event bus, no multi-user state and no authorization model beyond "the user is asked" (the guard hooks, outside this scope). The authorization matrix is therefore one row: the agent may read; every write needs the user's explicit yes (`--confirmed`, `--write`, the `[Y/n]` prompt).
