---
name: pass
description: Keep ai-gent's credentials in pass — check pass and gpg-agent are ready, list entries by name, give the exact pass insert for a database password or a GitHub, Azure DevOps, GitLab or Bitbucket token under ai-gent/<area>/<name>, and move plain-text secrets into pass — never showing a value. Use for "store this token in pass", "set up pass for ai-gent", "where do my credentials live".
---

# Secrets in pass

Read `../../references/secrets.md` first: entry names, the order of
sources, and what the agent may and may not run. The agent handles
**names**; values stay between the user's terminal, pass and the runner
that needs them.

## Is pass ready?

Check, without decrypting anything:

1. `command -v pass gpg` — both installed. If not, name the package for
   the system (`pass` on Debian/Ubuntu, Fedora, Arch, Alpine, Homebrew)
   and stop; the user installs it.
2. The store exists: `pass ls >/dev/null` succeeds, or
   `${PASSWORD_STORE_DIR:-~/.password-store}/.gpg-id` exists. If not,
   the user creates a key (`gpg --full-generate-key`) and runs
   `pass init <key-id>` themselves — both are theirs to run, in their
   own terminal.
3. `pass ls ai-gent` — which ai-gent entries exist (names only). For the
   current repository, `reporun detect` (`repo:issues`, `repo:pr`) says
   which platform token it needs and whether it's there.

Report what's there and what's missing. Never check a value by reading
it; a runner's own test (`dbrun test <PROFILE>`) proves an entry works.

## Add or rotate a secret

1. Pick the entry from the table in `secrets.md` (`ai-gent/db/<profile>`,
   `ai-gent/github/pat`, …).
2. Give the user the command to run **in their own terminal** — pass
   prompts without echo, so nothing reaches the conversation:

   ```bash
   pass insert ai-gent/github/pat        # add
   pass insert -f ai-gent/github/pat     # replace (rotate)
   pass insert -m ai-gent/github/pat     # first line the token; notes after (scopes, expiry)
   ```

3. Suggest the narrowest scope and the shortest expiry that work: a
   read-only database account; a fine-grained token limited to the
   repositories and permissions the task needs.
4. Afterwards, `pass ls ai-gent` to confirm the name, then the runner's
   test (for a database, `dbrun test <PROFILE>`).

Never run `pass insert` yourself, ask for the value in the chat, or
accept one pasted there: if the user pastes a secret, tell them it is now
in the conversation's history and should be rotated.

## Move plain-text secrets into pass

- **Database profiles:** `dbrun profiles` flags profiles whose password
  is still in the profiles file; `dbrun secrets migrate <PROFILE>` moves
  each one into `ai-gent/db/<profile>` without showing it and removes the
  line from the file (`db:connect`).
- **Tokens elsewhere** (a `.env`, a CLI's own config): tell the user
  which file holds it and the `pass insert` to run; removing it from the
  old place is theirs to do once the new one works.

## Locked store

If a runner reports it couldn't read an entry because the store is
locked, ask the user to unlock it in their terminal —
`pass show <entry> >/dev/null` (the value goes nowhere) — and retry.
gpg-agent then keeps it unlocked for its cache time. Never ask for the
passphrase.

## Handoff

Which entries exist, which are still missing (with the exact
`pass insert` for each), and the runner test that proves them — then back
to the skill that needed the secret (`db:connect`, `db:inspect`, …).
