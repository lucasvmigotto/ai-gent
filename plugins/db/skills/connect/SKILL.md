---
name: connect
description: List, test and classify database connection profiles — which databases exist for this project, their engine and environment class (production, staging, development, proven local), and whether they connect — without ever printing credentials, and help the user add a profile safely. Use for "test the DB connection", "which databases can you reach", "add a database profile", "is this database local".
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
- If the file doesn't exist, create it with placeholder values
  (`<set by the user>`) and `chmod 600`, and tell the user to fill in the
  secret values themselves. Never write a real credential, never ask the
  user to paste one into the conversation, and never set `_CLASS=local`
  for something that isn't a container of this project.
- Suggest a read-only account for every non-local profile.
- After the user fills it in: `classify`, then `test`.

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
problem.
