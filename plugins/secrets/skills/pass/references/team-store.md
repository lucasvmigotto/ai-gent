# Team stores — setup, membership, rotation

A team store is a pass store in a git repository, encrypted to every
member's GPG key. Read `../../../references/secrets.md` (*Stores and
mounts*, *Team stores*) for how the runners use it. Everything below is
done by people, in their own terminals: the agent explains the steps,
checks names and state (`pass ls`, `pass git status`, `pass git log`),
and never runs a command that writes, re-encrypts or pushes.

## Set it up once

```bash
git init --bare <remote>                       # or a private repository on your platform
git clone <remote> ~/team-store
PASSWORD_STORE_DIR=~/team-store pass init <fpr-1> <fpr-2>   # .gpg-id: one fingerprint per line
PASSWORD_STORE_DIR=~/team-store pass git init               # pass commits each change
```

Mount it next to the personal store, in each member's shell profile:

```bash
export AI_GENT_PASS_MOUNTS="team=$HOME/team-store"
```

Entries then read as `pass:team/db/<family>`. Narrow a folder to fewer
people with its own `.gpg-id`:
`PASSWORD_STORE_DIR=~/team-store pass init -p db/core <fpr-1>`.

## Join

1. The newcomer creates a key (`gpg --full-generate-key`), exports the
   public half and commits it under `.members/<name>.asc` with a line
   giving its fingerprint (`gpg --fingerprint <email>`), on a branch or
   a request — never a private key, never a passphrase.
2. An existing member **checks the fingerprint out of band** — a call,
   in person, a second channel — against the committed one. A key that
   only arrived through the repository proves nothing.
3. That member imports it (`gpg --import .members/<name>.asc`), adds the
   fingerprint and re-encrypts in one step:
   `PASSWORD_STORE_DIR=~/team-store pass init <all fingerprints>` (or
   `-p <folder>` for a narrowed folder). pass re-encrypts every entry and
   commits once, so a half-done state is never pushed.
4. They push (`pass git push`), and the newcomer pulls.

## Leave or remove

1. Remove the fingerprint and re-encrypt the same way
   (`pass init <remaining fingerprints>`), then push.
2. **Rotate.** Re-encryption protects only new reads: the git history
   still holds the old ciphertexts, which the removed key decrypts. Work
   through the checklist below for every entry that person could read.

### Rotation checklist

- [ ] List the entries they could read: every folder whose nearest
      `.gpg-id` included their fingerprint (`pass ls`, the `.gpg-id`
      files).
- [ ] For each: change the secret **at its source** — the database
      account's password, the token on the platform (revoke, then issue a
      new one with the same narrow scope).
- [ ] Store the new value (`pass edit` or `pass insert -f`), which
      commits it; push.
- [ ] Check the runners still connect (`dbrun test <PROFILE>`,
      `reporun detect`).
- [ ] Note the date and who was removed in the commit message or a
      `.members/` log — names only.

## Sync without surprises

- Pull before you change anything (`pass git pull`); push right after.
  The runners fetch on their own before reads (within
  `AI_GENT_PASS_FRESHNESS`) and refuse to write to a store that can't
  fast-forward.
- **Never merge `.gpg` files.** If a pull can't fast-forward, someone
  changed the store meanwhile: reset your copy to theirs and redo your
  own change on top (`pass git fetch`, then re-apply), never a merge that
  splices ciphertexts.
- The agent may run `pass git pull`, `fetch`, `status`, `log` and `diff`;
  `pass git push` and anything that rewrites the store stay with you.
