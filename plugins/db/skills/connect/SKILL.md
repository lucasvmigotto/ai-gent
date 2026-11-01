---
name: connect
description: List, test and classify database connection profiles — engine, environment class (production, staging, development, proven local) and whether they connect — without printing credentials, and add a profile safely. Use for "test the DB connection", "which databases can you reach", "add a database profile", "is this database local".
---

# Database connections

Read `../../references/safety.md`, `../../references/runner.md` and
`../../references/connections.md` first. Every action goes through
`dbrun`; never open the profiles file, `source` it, or call a database
client directly.

## List and classify

1. `dbrun profiles` — names, engines, declared classes and sources.
2. For the profile(s) the user means, `dbrun classify <PROFILE>` — the
   effective class with its evidence. Report it plainly: a profile
   declared `local` that fails the proof is **remote**, and a profile
   with no class is **production**.
3. `dbrun test <PROFILE> --reason "<why>"` — SUCCESS or FAILED with the
   reason (DNS, refused, timeout, authentication, TLS). Diagnose from the
   reason; never retry with credentials typed into the command.

Present a short table: profile · engine · class (and why) · reachable ·
read-only account (if the test could tell).

## Add a profile

- Explain the format (`connections.md`) and which keys the engine needs.
  Several profiles on one account (dev and QA databases behind one user,
  say) fit the TOML file best: one `[families.<account>]` with the shared
  host, user and `password = "pass:…"`, and a short `[profiles.<NAME>]`
  per database. An Oracle instance without a service name takes `SID`.
- Write only the non-secret keys (engine, class, host, port, database,
  user, project) — creating the file with `chmod 600` if it doesn't
  exist — and **no `_PASSWORD` line**: the password goes into pass.
  Never set `_CLASS=local` for something that isn't a container of this
  project.
- Give the user the command for their own terminal:
  `pass insert ai-gent/db/<profile>` (lowercase, `_` → `-`). If pass
  isn't set up, `secrets:pass` gets it ready; only without pass does the
  password go into the file, as the last resort, typed by the user.
- Never write a credential, and never ask the user to paste one into the
  conversation.
- Suggest a read-only account for every non-local profile.
- Then `dbrun profiles` (the password column shows the pass entry),
  `classify`, and `test`.

## Plain-text passwords

When `dbrun profiles` flags a password still in the file, offer
`dbrun secrets migrate <PROFILE>`: it moves the value into pass without
showing it and removes the line. If `test` then fails because the store
is locked, ask the user to unlock it in their terminal (the command
`dbrun` prints) and retry.

## Local databases

`local:<service>` profiles appear on their own when the project's
database containers are running (`devcontainer:infra`). If the user
expects one and it's missing, check with `$CONTAINER_ENGINE compose ps`
that the container is up and belongs to this repository's compose
project.

## Handoff

Which profiles are usable and in which class, what the user must fill
in, and the next skill: `db:inspect` for a schema discovery, `db:review`
for the application's database usage, `db:investigate` for a data
problem. If you started a throwaway datastore container to prove a
profile connects, remove it and its image per
`../../references/containers.md` §9.
