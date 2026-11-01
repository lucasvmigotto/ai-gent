# Branches — the chain, names, sub-branches, a new repository

- **The chain**, highest first: **`main`/`master` > `homolog`/`staging`
  (when there is one) > `dev`/`develop` > work branches.** Work enters at
  `dev` and moves up one level at a time.
- **Branch off `dev`**, after `git fetch`. Getting `dev` ready (missing,
  only on the remote, behind or diverged from it) and the exception for a
  `dev` far behind the levels above are in `chain.md`. Both
  `dev` and `develop` exist: ask which one.
- **Names:** `<prefix>/[<id>-]short-description`, kebab-case —
  `feat/`, `fix/`, `hotfix/`, `release/`, `refactor/`, `perf/`, `docs/`,
  `test/`, `build/`, `ci/`, `chore/` (or `feature/`, `bugfix/` where the
  repository uses them). An id goes right after the prefix
  (`feat/123-user-auth`) only when it's confirmed — given by the user or
  recorded in a `tracker.md`. A repository's own convention wins.
- **A `hotfix/`** branches off the production branch and lands back in it
  **and** in every level below, so the fix isn't lost (each landing asked).
- **One context, one sub-branch**, off the parent's current tip, with a
  flat suffix (`feat/user-auth-db-schema` — `feat/user-auth/db-schema`
  can't coexist with `feat/user-auth`). Land it the moment it's done;
  the next one starts from the updated parent. Never chain sub-branches
  or save the landings for the end.
- **A protected trunk:** when the user works on an integration branch
  because the chain's branches are protected, that branch is the parent.
- **A new repository** starts the same way every time: `git init -b main`
  (or the user's production branch), a single-line `README.md` —
  `# <Project name>` — committed alone as `chore: initial commit` (the
  guard asks), then `dev` from it. Input documents the user handed in (a
  spec, requirements, references) are never committed unless they say
  so. **No `LICENSE` file unless the user names one.** An unlicensed
  repository is private and all rights reserved by default — never add
  an open-source license (or any other) on your own initiative; ask
  which one, if any, once it comes up — and mention open-sourcing it as
  a genuine option worth considering, GPLv3 being a common
  strong-copyleft pick, without pushing either way; the choice stays
  the user's.
