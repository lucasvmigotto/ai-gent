# Research — guard UX

Each item has a question, what was checked, and a decision. Items marked *to verify* need a run on Claude Code 2.1.312 before building; the decision says what happens either way.

## R1 — Can a mod read the guard's decision without deciding itself?

**Checked**: the 2.1.312 types declare `classic.PreToolUse` as a hookable event whose `next(e)` resolves to `PreToolUseResult` (`{ allow } | { ask: string } | { deny: string } | {}`, plus `updatedInput`, `additionalContext`), and the classic chain as "[managed settings hooks, ...hooks modules, the other settings hooks as core]". The guards are plugin command hooks (`plugins/<name>/hooks/hooks.json`, `PreToolUse`, matcher `Bash`; the db guard also `Read|Grep|Glob|Edit|Write|MultiEdit|NotebookEdit`). Each guard prints `permissionDecision` and a reason prefixed with its name.

**Decision**: hook `classic.PreToolUse`, call `next(e)` first, and decorate the result. *To verify*: that plugin command hooks are part of "the other settings hooks" beneath modules (one `--debug` run with `git push` and a mod that logs `next`'s result). **Fallback** if they are not: the mod runs its own plugin's `hooks/guard.sh` through `$.process.run` with the call's envelope as JSON on stdin and reads only the printed decision — the guard still decides, at the cost of running twice. Tests stub the guard either way (`claude plugin test` has no settings hooks beneath unless the test adds a `classic.PreToolUse` hook).

## R2 — Is a dialog "Allow" from a mod safe?

**Checked**: the types say an `allow` from `classic.PreToolUse` "lets the call run without a permission prompt". Unclear from the types alone whether settings `permissions.deny` rules and managed policy still apply after it (`tool.check` is a separate verdict).

**Decision**: the dialog stays off by default (FR-004). *To verify*: with a settings rule `deny: ["Bash(git push:*)"]`, does a mod's `allow` still get refused? If yes, the dialog may be offered as an opt-in; if no, the mod returns `ask` with better text only, and the dialog is dropped.

## R3 — Should the mod import `opencode/guard/rules.mjs`?

**Checked**:

- `rules.mjs` is plain ES module code with no Node imports (only `opencode/guard/index.ts` imports `node:child_process`); its exports (`gitGuard`, `dbGuard`, `dbCredsShell`, `dbCredsPath`, `dbCredsTool`, `secretsGuard`, `repoGuard`, `containersGuard`, `evaluate`) are pure. So it would load in a hooks module.
- `gitGuard` takes `branchOf` as a synchronous function; a mod would read the branch first with `$.process.run(["git", "rev-parse", "--abbrev-ref", "HEAD"])` and pass `() => branch`.
- Its input is the raw command; the shell guards get JSON-escaped text. The parity test already bridges that difference.
- A hooks module imports only files of its own plugin; `rules.mjs` sits in `opencode/guard/`, outside `plugins/`, so it would have to be symlinked into each of five plugins (and survive packaging, which drops links).

**Decision**: **do not import it.** The mod needs the guard's *result*, which R1 gives for free (or the fallback, from `guard.sh` itself). Deciding in the mod from `rules.mjs` would create a third decision path inside Claude Code that runs alongside `guard.sh`; any lag between them (a guard change landing before its port) would show the person one thing and enforce another. The parity test keeps `rules.mjs` equal to `guard.sh` on its vectors, not on every command.

**What importing would mean for parity tests, if ever revisited** (say, to preview a decision before the call): `rules.test.mjs` would stay the oracle test unchanged (guard.sh ↔ rules.mjs); the mod would need a test that its import resolves to the same file (the symlink), and the mod's use would have to be advisory only, never a decision. No new vector set would be needed, but R3's drift risk would remain.

## R4 — Shared code across five modules

**Checked**: one hooks module per plugin; a module imports files of its plugin with `import` declarations (no `import()`). Five plugins need the same explanation table and decoration logic.

**Decision**: one source file, `shared/guard-ux.ts`, linked by relative symlink into each plugin into each plugin's `hooks/` (decided 2026-10-28; CLAUDE.md amended). *To verify*: the engine follows a symlinked import inside a plugin loaded through `setup.sh`'s symlinked plugin directory. Fallback: a small generator in `scripts/` that copies the file, with `scripts/consistency.py` checking the copies are identical.
