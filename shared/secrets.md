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
| `ai-gent/forgejo/pat` | a Forgejo, Gitea or Codeberg access token | `reporun` |
| `ai-gent/bitbucket/token` | a Bitbucket access token, or an Atlassian API token with an `email:` line | `reporun` |
| `ai-gent/jira/token` | a Jira API token (Cloud, with `email:` and `url:` lines) or PAT (Server/Data Center) | `reporun` |

A tool may also take an explicit reference, `pass:<entry>`, wherever its
configuration names a secret (`SHOP_DEV_PASSWORD=pass:work/db/shop`), so
an existing store layout keeps working.

**The first line is the secret** (pass's own convention); later lines are
notes. `key: value` notes a runner documents are read with the secret
(`email:` and `url:` for Atlassian, `org:` for Azure DevOps); anything
else — the scopes, the expiry date — is for you.

## Rules for tools that read a secret

- **Only the runner reads it** — `dbrun` for databases, `reporun` for
  code platforms and trackers — by running `pass show <entry>` itself,
  keeping the first line, and handing it to the one client or API call
  that needs it (stdin, a mode-600 file, or that process's environment;
  never argv, where `ps` shows it).
- **Decrypt late.** Listing, classifying or planning never decrypts;
  only connecting does. Whether an entry exists is a file check
  (`<store>/<entry>.gpg`), no passphrase needed.
- **Validate the name** before calling pass: no leading `-` or `/`, no
  `..` segment.
- **Never print, log or echo** the value; redact it from every error
  message (drivers and APIs quote what they received).
- **Locked store.** gpg-agent caches the passphrase for a while. When it
  can't prompt (no pinentry for a non-interactive shell), the read fails:
  stop, and ask the user to unlock it in their own terminal
  (`pass show <entry> >/dev/null`), then retry. Never ask for the
  passphrase, and never loop on the failure.
- **No pass installed** — fall back to the documented last resort, and
  say pass is the better home.

## What the agent may do

- List names: `pass ls`, `pass find <word>`, the runner's own listing.
- Tell the user the exact command to add or rotate a secret —
  `pass insert ai-gent/<area>/<name>` (or `pass edit` to rotate), run in
  **their own terminal**: pass prompts without echo, so the value never
  enters the conversation.
- Move plain-text secrets into pass through the runner's migration
  command (`dbrun secrets migrate <profile>` or `--all`), which pipes the
  values without showing them.

Never (bar the user's own allowlist, below): `pass show`; never: a bare
`pass <entry>`, `pass -c`, `pass edit`,
`pass grep`, `pass generate`, `pass otp`, `pass insert`, `gpg` on the
store, or a runner such as `pass-run` handing a secret to a command that
can print it (`env`, a shell, `echo`). Store changes (`rm`, `mv`, `cp`,
`init`, `git`) are the user's call. The `secrets` guard hook enforces
this in Claude Code, and the OpenCode guard plugin does the same there;
in other agents the rules are instructions only.

### The personal allowlist

Some entries are the user's to hand over, such as a token the user wants
the agent to read. Only the user opens one, in a plain-text file the
`secrets` guard reads on every `pass show`: `$AI_GENT_PASS_ALLOW`, else
`${XDG_CONFIG_HOME:-~/.config}/ai-gent/pass-allow`.

```text
# one rule per line; blank lines and # comments are ignored
work/vpn
team/
```

- A rule ending in `/` allows everything under that prefix; any other rule
  is one exact entry. Rules that are empty, start with `/`, `-` or `~`, or
  have a `.` or `..` path part never allow anything.
- It opens only `pass show <entry>` with exactly one entry and **no flags**:
  never the bare `pass <entry>`, `-c`/`-q`, `edit`, `grep`, `generate`,
  `otp` or `insert`; never an entry under `ai-gent/db/` (only `dbrun`
  reads those, even if a rule names one); never a command that sets
  `PASSWORD_STORE_*`, `XDG_CONFIG_HOME` or `HOME`; never a name with shell
  metacharacters or substitutions; never when another `pass` command
  shares the line.
- A missing or unreadable file, or no matching rule, changes nothing: the
  guard denies as before and says where to list the entry.
- A Bash command that mentions the file (`pass-allow`, or the value of
  `AI_GENT_PASS_ALLOW`) is asked about, so the agent cannot widen its own
  reach through the shell. Write and Edit tool calls are outside the hook
  and fall under Claude Code's own permission prompts; don't edit the file
  unless the user asks.
- In OpenCode the guard plugin applies the same rules; the static
  `pass show*` deny rule `setup.sh` can add also applies there, so remove it
  to use the allowlist.

A request to debug or "compare" a rejected secret is still a read —
don't run `pass show` or `gpg -d` even to help: point at length/prefix
checks the user runs themselves, or testing the value against the thing
that rejected it (the runner's own test, or a curl with the secret piped
in on the user's side).

## Narrow and short-lived

A secret's real limit is its scope: a read-only database account, a
fine-grained token with the fewest permissions and the shortest expiry
that works. Note the expiry in the entry's later lines, and rotate with
`pass edit` (or `pass insert -f`) in the user's terminal.
