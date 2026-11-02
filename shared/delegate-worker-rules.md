# Delegated worker rules

Follow AGENTS.md and the task brief. Stay inside `allowed_paths`; do not
change other files, including through generated output or symlinks.
Run only the brief's targeted checks. Keep build output outside the worktree
and use the supplied temporary directory. You have no network; do not seek it.
Do not edit skills, plugins, hooks, shared references or agent instructions
unless the owner explicitly authorized that exact task and path.
Do not bump versions, tag releases or edit CHANGELOG files.
Never delete anything you did not create.
Never weaken, skip or remove tests or assertions to make checks pass.
Do not tick `tasks.md` items without evidence from the required checks.
Report blockers, denied operations, missing tools and environment failures
in your final message, never in docs or any repository file.
Report problems to the supervisor instead of bypassing limits or guards.

## Git

Leave changes in the working tree and stop; the supervisor stages and commits.
Do not commit, push, stage, merge, rebase, reset, clean or rewrite history.
Do not write `.git`, its index, refs, config or worktree metadata, through
git commands or any other tool. Use only read-only git inspection.
Never bypass hooks with `--no-verify` or `commit -n`.
Do not add commit-message markers, trailers, signatures or agent attribution
to commits, proposed messages, pull requests, issues or comments.

## Secrets

Never read, print, copy, log or write credential values or authentication
files, including your own agent's login files, tokens, keys and cookies.
Do not put secrets in commands, shell environments, repository files or reports.
Inspect names only; never use `pass show`, bare `pass <entry>`, clipboard or
QR options, secret searches, direct GPG decryption or secret-printing wrappers.
Do not add, edit, generate, move or delete password-store entries.
Only the authorized runner reads a secret; report credential needs to the
supervisor without asking for the value or trying another source.

## DB

Do not use database tools, including `dbrun`, direct clients, drivers,
container-wrapped clients, dumps, restores or migrations against a database.
Never read database credential files such as `connections.env`,
`connections.toml` or the ai-gent database config directory.
Database access belongs to the supervisor through `dbrun`, which classifies,
masks and audits access; writes require a proven local target.
Report any task or check that needs a database instead of bypassing the runner.

## Repo

Do not use network or platform tools to create, update, comment on, approve,
close or merge issues, work items or pull requests. Do not run `reporun --write`
or substitute a CLI or direct API call. The supervisor handles platform
changes, required approvals, CI checks and publishing without attribution.

## Containers

Do not use container tools, even through task recipes or test helpers.
Do not build, pull, start, exec into, stop or remove containers or resources.
Report container-dependent checks to the supervisor.
Never run any prune, remove-all operation or removal from an unfiltered list.
The supervisor scopes cleanup to owned IDs, its own prefix or project labels,
preserves standing environments and other projects, and respects the disk floor.
