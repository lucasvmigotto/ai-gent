# Secrets in pass

How ai-gent keeps credentials — database passwords, personal access
tokens — out of files, the conversation and the agent's reach. The store
is [pass](https://www.passwordstore.org/), the standard Unix password
manager: one GPG-encrypted file per entry under `~/.password-store`
(`PASSWORD_STORE_DIR` moves it). `secrets:pass` sets it up and adds
entries; this file is the contract every skill that uses a secret follows.
The runners share one implementation of it, `passstore.py` (in `shared/`,
linked next to each runner).

## Order of sources

1. **pass** — the first choice, always.
2. A source the tool owns and documents (a running dev container's own
   environment for `local:` database profiles — fixtures, not secrets).
3. **A plain-text file** (such as the db profiles file) — the last
   resort, for machines without pass. Tools that read one say so and point
   at the move into pass.

A secret is never typed on a command line, pasted into the conversation,
written into a repository, or put in the agent's shell environment.

## Entry names

`ai-gent/<area>/<name>` — lowercase, digits and hyphens, one entry per
secret. `AI_GENT_PASS_PREFIX` replaces `ai-gent` (for a store shared with
other setups).

| Entry | Holds | Read by |
|---|---|---|
| `ai-gent/db/<profile>` | a database profile's password; the profile name lowercased, `_` → `-` (`SHOP_DEV` → `ai-gent/db/shop-dev`) | `dbrun` only |
| `ai-gent/db/<family>` | the password of a family of profiles that share one **account** (a TOML family) — keyed by the account, never by matching values | `dbrun` only |
| `ai-gent/github/pat` | a GitHub fine-grained token | `reporun` (`repo:*`) |
| `ai-gent/azure-devops/pat` | an Azure DevOps PAT | `reporun` |
| `ai-gent/gitlab/pat` | a GitLab project or personal access token | `reporun` |
| `ai-gent/bitbucket/token` | a Bitbucket access token, or an Atlassian API token with an `email:` line | `reporun` |
| `ai-gent/jira/token` | a Jira API token (Cloud, with `email:` and `url:` lines) or PAT (Server/Data Center) | `reporun` |

A tool may also take an explicit reference, `pass:<entry>`, wherever its
configuration names a secret (`SHOP_DEV_PASSWORD=pass:work/db/shop`), so
an existing store layout keeps working.

**The first line is the secret** (pass's own convention); later lines are
notes. `key: value` notes a runner documents are read with the secret
(`email:` and `url:` for Atlassian, `org:` for Azure DevOps); anything
else — the scopes, the expiry date — is for you.

## Stores and mounts

The personal store is `PASSWORD_STORE_DIR` (default `~/.password-store`).
More stores are **mounted** next to it with
`AI_GENT_PASS_MOUNTS="name=path;name=path"` (lowercase names): an entry
whose first segment is a mount's name lives in that store —
`pass:team/db/shop` is `db/shop` in the store mounted as `team`. A mount
holds the same files as any pass store, so it can be used with plain pass
too (`PASSWORD_STORE_DIR=<path> pass ls`).

## Team stores

A team store is a pass store that is a **git repository**, encrypted to
every member's key and shared through a remote. Config files keep only
`pass:<entry>` references, so they can be shared too.

- **Recipients** are pass's own: `.gpg-id` lists one fingerprint per
  member, and a folder's own `.gpg-id` (the nearest one up the tree)
  narrows who can read it (`pass init -p db/core <fpr…>`).
- **Sync.** Before a read, the runners fetch and fast-forward a store
  that has a remote when its last fetch is older than
  `AI_GENT_PASS_FRESHNESS` seconds (default 300; `0` turns that off), and
  always before a write; a write is committed and pushed. Nothing is ever
  merged: a store that can't fast-forward is reported and the local copy
  is used — `.gpg` files are never auto-merged.
- **Membership** — joining, approval after an out-of-band fingerprint
  check, re-encryption, removal and the rotation it requires — is a
  procedure for people: `secrets:pass` walks through it, with the
  setup commands and a rotation checklist.
  Removing someone revokes nothing by itself: the history keeps old
  ciphertexts they can decrypt, so every secret they could read is
  rotated at its source.

## Without pass: Windows and WSL

pass is a bash script, not available on native Windows. The runners then
decrypt with **gpg** — `gpg --decrypt <store>/<entry>.gpg` — and keep the
first line: the files are the same, so no pass binary is needed. On
Windows that's [Gpg4win](https://www.gpg4win.org/) (its gpg-agent and
graphical pinentry). Writes encrypt to the recipients of the nearest
`.gpg-id`.

For non-interactive calls (an agent's shell), the passphrase must come
from gpg-agent's cache or a pinentry that can show itself: set the cache
in `gpg-agent.conf` (`default-cache-ttl 600`, `max-cache-ttl 7200`, in
seconds) and unlock once in your own terminal. On WSL without a graphical
pinentry, a read can't prompt: the runners stop within seconds and ask
you to unlock in your terminal (or set
`pinentry-program` to Windows' `pinentry.exe` from Gpg4win).

## Rules for tools that read a secret

- **Only the runner reads it** — `dbrun` for databases, `reporun` for
  code platforms and trackers — by running `pass show <entry>` (or
  `gpg --decrypt` without pass) itself, keeping the first line, and
  handing it to the one client or API call that needs it (stdin, a
  mode-600 file, or that process's environment; never argv, where `ps`
  shows it).
- **Decrypt late.** Listing, classifying or planning never decrypts;
  only connecting does. Whether an entry exists is a file check
  (`<store>/<entry>.gpg`), no passphrase needed.
- **Validate the name** before calling pass: no leading `-` or `/`, no
  `..` segment.
- **Never print, log or echo** the value; redact it from every error
  message (drivers and APIs quote what they received).
- **Locked key.** gpg-agent caches the passphrase for a while. When it
  can't prompt (no pinentry for a non-interactive shell), the read fails:
  stop, and ask the user to unlock it in their own terminal
  (`pass show <entry> >/dev/null`), then retry. Never ask for the
  passphrase, and never loop on the failure.
- **No pass and no gpg** — fall back to the documented last resort, and
  say pass is the better home.

## What the agent may do

- List names: `pass ls`, `pass find <word>`, the runner's own listing.
- Sync a team store: `pass git pull`, `fetch`, `status`, `log`, `diff`.
- Tell the user the exact command to add or rotate a secret —
  `pass insert ai-gent/<area>/<name>` (or `pass edit` to rotate), run in
  **their own terminal**: pass prompts without echo, so the value never
  enters the conversation.
- Move plain-text secrets into pass through the runner's migration
  command (`dbrun secrets migrate <profile>` or `--all`), which pipes the
  values without showing them.

Never: `pass show`, a bare `pass <entry>`, `pass -c`, `pass edit`,
`pass grep`, `pass generate`, `pass otp`, `pass insert`, `gpg` on the
store, or a runner such as `pass-run` handing a secret to a command that
can print it (`env`, a shell, `echo`). `pass git push` and other store
changes (`rm`, `mv`, `cp`, `init`) are the user's call. The `secrets`
guard hook enforces this in Claude Code, and the OpenCode guard plugin
does the same there; in other agents the rules are instructions only.

## Narrow and short-lived

A secret's real limit is its scope: a read-only database account, a
fine-grained token with the fewest permissions and the shortest expiry
that works. Note the expiry in the entry's later lines, and rotate with
`pass edit` (or `pass insert -f`) in the user's terminal.
