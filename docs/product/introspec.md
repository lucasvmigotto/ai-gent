Reconstructed by project:introspec on 2026-10-18 from 384c771

# Introspection report — ai-gent docs site and runners

Status: Draft

## Scope and boundaries

- **In scope** (agreed with the requester): the documentation site `docs/site/` and the runner scripts `plugins/db/scripts/dbrun.py`, `plugins/repo/scripts/reporun.py`, `plugins/repo/scripts/specsync.py`, `plugins/agents/scripts/wt.py`, `shared/passstore.py`, `setup.sh`, `install.sh`.
- **Out of scope**: the skills' Markdown (`plugins/*/skills/**/SKILL.md` and references), which is already its own specification; `scripts/check.sh`, `scripts/release.py`, `scripts/update-check.sh`, `scripts/opencode-config.py`, the guard hooks and `opencode/guard/` (listed below as entry points, not reconstructed).
- **What ran**: only `scripts/check.sh --quick` (the repository's own checks, which run the runners' unit tests), read-only `git` and `gh run list`, two one-line Python probes of runner functions, and the reconstructed specs fed to `specsync` and `wt.py`. No database: none is involved in this scope (dbrun was read, not run against an engine). Nothing was installed: `bun install` needs the network, so the site's own tests were not run locally — their results come from CI.
- **No HTTP API**: `contracts/openapi.yaml` is N/A; contracts are per-feature `contracts/cli.md` and `routes.md`.
- **Spec Kit not bootstrapped**: `specify init` would add `.specify/` and `.claude/skills/speckit-*` to the toolkit's own repository, changing what every Claude Code session here loads; that is a decision for the maintainer. The features follow the installed Spec Kit 1.2.2 templates (`spec-template.md`, `plan-template.md`, read from the local `specify-cli` package) without the bootstrap. Consequently there is no constitution, and every plan's Constitution Check reads N/A.
- **Confirmation round**: not run — this reconstruction ran as a delegated agent with no user to ask. Every Inferred and Assumed item is listed under *Open items* for that round.

## Inventory

| Area | Contents | Evidence |
|---|---|---|
| Languages (repo) | 297 Markdown, 21 Python, 21 JSON, 17 shell, 12 TS, 10 TSX, 7 MJS files | `git ls-files` by extension |
| Docs site | Bun 1.x, React 19.3.0, TypeScript 7.0.2, Vite 8.3.2, Tailwind 4.3.3, Biome 2.5.15, Vitest 5.0.3; lockfile `bun.lock` | `docs/site/package.json` |
| Runners | Python 3.9+ stdlib only; drivers via `uv` at run time | module docstrings |
| Installer | POSIX sh + bash ≥ 4 | `install.sh:1`, `setup.sh:1,71` |
| CI/CD | `check.yml` (PR, called by release), `release.yml` (push main), `docs-ci.yml` (docs paths, release workflow_run) | `.github/workflows/` |
| Containers | none for the project itself; dbrun uses `ghcr.io/astral-sh/uv:python3.12-bookworm-slim`, Docs CI `selenium/standalone-chrome` | `dbrun.py:44`, `docs-ci.yml:135` |
| IaC | none | — |
| License | GPL v3+ | `LICENSE`, footer test |
| SBOM | `docs/product/sbom.cdx.json` — 337 npm components from `bun.lock` (CycloneDX 1.5). syft and cdxgen were not installed, so it was generated from the lockfile by a one-off reader; the Python runners have no third-party dependencies to list | this commit |

### Entry points

| Entry point | Kind | Feature |
|---|---|---|
| `#/`, `#/use/skills`, `#/use/skills/:id`, `#/*` | screens | 002 |
| `#/use/install`, `#/use/flow`, `#/contribute`, `#/reference`, `#/changelog` | screens | 003 |
| `dist/llms*.txt`, `dist/docs/<locale>/**.md` | build output | 004 |
| `bun run gen/dev/build/test/lint/typecheck/e2e/og` | CLI | 001, 004, 005 |
| Docs CI jobs `verify`, `e2e`, `deploy` | CI jobs | 005 |
| `install.sh`, `setup.sh` | CLI | 006 |
| `passstore` (`prefix`, `store`, `check`, `has`, `read`, `write`) | library | 007 |
| `dbrun profiles/classify/test/query/plan/apply/restore/script/session/secrets/log/_exec` | CLI | 008 |
| `reporun detect/cli/api/batch/chain` | CLI | 009 |
| `specsync plan/record/features` | CLI | 010 |
| `wt.py plan/add/list/remove` | CLI | 011 |
| `scripts/check.sh`, `release.py`, `update-check.sh`, `ai-gent-update.sh`, `opencode-config.py`, `test-*.sh`, `test-dbrun-engines.py` | internal CLIs | out of scope |
| `plugins/{git,db,secrets,repo}/hooks/guard.sh`, `opencode/guard/` | hooks | out of scope |
| `plugins/project/scripts/repo_state.py`, `plugins/compact/` scripts | skill helpers | out of scope |

### Tables and collections

None: no database in scope. File-based state is in `docs/product/architecture.md` § Data stores, each owned by one feature.

## Runs

| Command | Result |
|---|---|
| `scripts/check.sh --quick` (2026-10-18, on 384c771 and on every commit of this branch) | all checks passed; "reporun, specsync (Ran 35 tests)", "dbrun (Ran 51 tests)", "agents wt.py (Ran 4 tests)"; installer and update-check tests skipped by `--quick` |
| `gh run list --workflow release.yml -L 3` | run 39066868067 on 0d34208 (the last code commit before 384c771) success — it runs `check.yml`, i.e. the full `check.sh` including `scripts/test-install.sh` |
| `gh run list --workflow docs-ci.yml -L 5` | all success; run 39067000179 (dev, 384c771) includes `verify` (lint, typecheck, unit tests, build, assertions) and `e2e` (Chrome smoke) |
| `python3 -I -c 'import reporun; reporun.cli_is_write("gh", …)'` | `["label","create","status"]` → False, `["issue","edit","12","view"]` → False (writes classified as reads) |
| `python3 -I plugins/repo/scripts/specsync.py features` on the new `specs/` | 11 features parsed, all `review` (Implemented), not synced |
| `python3 -I plugins/agents/scripts/wt.py plan` from this branch's worktree | failed: "no specs/README.md" — it reads the main checkout |
| `scripts/test-dbrun-engines.py`, `bun run test`, `bun run e2e` | not run (containers or network; results taken from CI where CI runs them) |

## Drift against existing docs

`docs/product/brief.md` and `docs/product/ux-vision.md` existed before this run and were **not rewritten**. Their drift:

### brief.md

- [UPSTREAM GAP: the Summary calls the site "bilingual"; the decision log of 2026-10-05 reversed that to English only, and the code ships `LOCALES = ["en-US"]` (`docs/site/src/i18n.tsx:9`). The brief contradicts itself.]
- [UPSTREAM GAP: counts are stale — "9 plugins, 36 skills", "Claude Code and opencode", "just reached 1.7.0". At 384c771: 12 plugins (agents, repo and secrets added), 44 plugin skills, nine agents supported, release 2.8.0.]
- [UPSTREAM GAP: Capabilities say "client-side search over titles and content"; search covers command, id, name, plugin and description, not the body (`docs/site/src/content.ts:73`).]
- [UPSTREAM GAP: "Shared reference — the README's conceptual sections as pages"; the Reference page links to the source files on GitHub instead (`docs/site/src/pages.tsx:351-358`).]
- [UPSTREAM GAP: journey 1 promises install troubleshooting "drawn from setup.sh behavior" and journey 2 "maps each failure class to its fix"; neither exists. The copy key `install.troubleshooting` is defined but never rendered (`i18n.tsx:214`).]
- [UPSTREAM GAP: the candidate feature map lacks the Flow page (`#/use/flow`) and the Changelog page, both shipped.]
- [UPSTREAM GAP: "project:spec is N/A here … the host repo has no specs/" — this run adds a reconstructed `specs/`; the brief should say whether that changes the plan.]
- [UPSTREAM GAP: the glossary's Guard hook enforces "git or db rules"; there are four guards now (git, db, secrets, repo).]
- [UPSTREAM GAP: the brief covers only the docs site; the runners and installer (features 006-011) have no brief section. Purpose and audiences for them are Assumed below.]
- The 200 KB landing budget is still an unmeasured assumption; nothing checks bundle size.

### ux-vision.md

- [UPSTREAM GAP: the IA lists `/reference/pipeline`, `/reference/containers`, `/reference/databases`, `/reference/guards` pages; the code has one `#/reference` list of links, plus `#/use/flow`, which the IA lacks.]
- [UPSTREAM GAP: the navigation model has a section sidebar in Use and Contribute and the theme toggle in the top bar; the code has no sidebar and puts the theme switch in the footer (`docs/site/src/components.tsx:415`).]
- [UPSTREAM GAP: the install page's per-tool tabs and troubleshooting state are two stacked code blocks with no troubleshooting.]
- [UPSTREAM GAP: `Esc` closes menu and search (Accessibility targets); no `Escape` handler exists in `docs/site/src`.]
- [UPSTREAM GAP: the empty search state should name the closest plugins and link the index; it shows two static sentences.]
- [UPSTREAM GAP: the skill page wireframe leads with `/qa-load`; the code leads with `/qa:load` (Claude Code), then the other agents' form.]
- [UPSTREAM GAP: "36 skills · 9 plugins" in copy examples; the live counter is generated, so only the vision's examples are stale.]
- The search-index loading skeleton and "index fetch failed" state don't apply: the index is bundled (N/A, not a gap).

## Contradictions (code vs. tests vs. docs)

- `docs/site/vite.config.ts` says "see .env.example"; it exists and agrees (no contradiction).
- The HTML and Markdown renderings of the same pages differ: the Markdown Reference lists 6 contracts, the HTML 9; the Markdown Install and Contribute pages drop sections (`specs/004-docs-site-llm-output/spec.md`).
- `install.sh`'s header lists `--target, --skip-prompt, --dry-run, --force, --uninstall`; `setup.sh` also takes `--yes`, `--no-config-edits`, `--verbose` (incomplete, not wrong).

## Dead code and unreachable features

- Copy key `install.troubleshooting` — defined, never rendered.
- Personal skills under `skills/` (`skill-docs`) are installed by `setup.sh` but never shown on the site (`generate-content.mjs:49` reads only `plugins/`) [ASSUMPTION: deliberate].

## Risks spotted

| # | Risk | Severity | Evidence |
|---|---|---|---|
| R1 | reporun classifies a CLI write as a read when a positional argument equals a read verb (`gh label create status`, `gh issue edit 12 view`), so it runs without `--write` and without the guard's question | high (a remote change without consent) | Runs table; `plugins/repo/scripts/reporun.py:309` |
| R2 | Docs CI actions pinned by tag (`actions/checkout@v4`, `oven-sh/setup-bun@v2`, `actions/upload-artifact@v4`) and Bun by major, in the job holding R2 credentials; the other workflows pin by SHA | medium | `.github/workflows/docs-ci.yml:36,40,98,112,114,195,199` |
| R3 | dbrun's client image is a tag without a digest and its drivers are unpinned; a compromised upstream would receive database credentials on stdin | medium | `dbrun.py:36-44,1081-1127` |
| R4 | Deploy deletes old hashed assets before uploading the new `index.html`; readers mid-deploy may hit 404s | low | `docs-ci.yml:231-248` |
| R5 | A deploy without R2 credentials is a green run that published nothing | low | `docs-ci.yml:210-218` |
| R6 | The skip link `href="#main"` under `HashRouter` likely navigates to the not-found route | low (a11y) | `docs/site/src/components.tsx:376` |
| R7 | `wt.py add` port offsets reuse a live track's offset after a removal | low | `wt.py:158` |
| R8 | `setup.sh` needs bash ≥ 4; `install.sh` only checks that bash exists | low (Linux) / high if macOS is supported | `setup.sh:71`, `install.sh:39` |
| R9 | No tests for the LLM output, or for the install, contribute, reference and changelog pages | medium (agents read the LLM output) | `docs/site/tests/` |
| R10 | `scripts/test-dbrun-engines.py`, the only end-to-end check of dbrun's engine-specific SQL, is manual and outside CI | medium | `CLAUDE.md` |

No secrets, connection strings or data rows were copied into any artifact; configuration is named by variable only.

## Feature statuses and evidence

| # | Feature | Status | Evidence |
|---|---|---|---|
| 001 | docs-site-shell | Implemented | unit, component and axe tests; Docs CI green |
| 002 | docs-site-skill-reference | Implemented | as above + e2e smoke for skill page and not-found |
| 003 | docs-site-guides | Implemented | flow page tested and smoked; the other pages untested; troubleshooting Planned |
| 004 | docs-site-llm-output | Implemented, no tests | build step runs in CI |
| 005 | docs-site-delivery | Implemented | Docs CI runs green |
| 006 | installer | Implemented | `test-install.sh` (70 checks) passes in CI; interactive selector untested |
| 007 | pass-store | Implemented | through the dbrun and reporun suites |
| 008 | db-runner | Implemented | 51 unit tests pass; engine suite not run |
| 009 | repo-runner | Implemented | unit tests pass; R1 found |
| 010 | spec-sync | Implemented | unit tests pass; parses these specs |
| 011 | worktrees | Implemented | unit tests pass |

Nothing is Verified. Confidence mix: most behavior claims are Observed with file:line; about a dozen Inferred items and eight Assumed items, listed below.

## Open items

### Inferred (confirm with the named check)

1. Skip link under hash routing renders not-found (`specs/001`) — keyboard test in a browser.
2. Install verify commands `claude plugin details project@skills-dir`, `opencode plugin list` work (`specs/003`) — run them in a real install.
3. Static pages' Markdown drifts by construction (`specs/004`) — render both from one description.
4. Mid-deploy 404 window (`specs/005`) — load the site during a deploy.
5. macOS bash 3.2 fails at `setup.sh:71` (`specs/006`) — run on macOS.
6. `setup.sh --target` with no value exits silently (`specs/006`).
7. `--uninstall --target codex` removes the entries of every shared-directory agent (`specs/006`).
8. `has()` true but `read()` failing on a missing gpg key; `pass` doesn't echo values on stderr (`specs/007`).
9. dbrun plan hash is integrity, not authenticity; `script`'s WHERE check passes on a subquery's WHERE (`specs/008`).
10. wt.py port-offset reuse; git ≥ 2.31 required (`specs/011`).

### Assumed (only the user can confirm)

1. Purpose and audiences of the runners and installer: the toolkit's users running agents on their own Linux machines; maintained by one person.
2. Linux is the only supported platform.
3. Personal skills are deliberately absent from the site.
4. Linking reference entries to GitHub, instead of rendering them, is a deliberate simplification.
5. The Markdown twins are meant to match the HTML pages (the gaps are drift).
6. The R2 credentials are configured (the deploy log wasn't read).
7. `scripts/test-dbrun-engines.py` passes on all five engines.
8. The shared-directory prune on uninstall is intended.

### Needs clarification

- [NEEDS CLARIFICATION: keep and build the install troubleshooting section (brief journey 1), or drop the dead `install.troubleshooting` key?] (`specs/003`)
- [NEEDS CLARIFICATION: bootstrap Spec Kit (`.specify/`, constitution, `.claude/skills/speckit-*`) in the toolkit repository, or keep these reconstructed specs template-shaped only?]

## Coverage checklist

- [x] every entry point is in a feature or listed as internal/out of scope
- [x] every table and collection — N/A, no database; file stores mapped to owners
- [x] every integration has a label and the config keys that select it (`architecture.md`)
- [x] every status field has a state machine (dbrun plan, tracker item); authorization matrix — one row, the user's yes (`domain-model.md`)
- [x] every feature status is backed by evidence; nothing Verified
- [x] SBOM generated (from the lockfile); stack and versions in `architecture.md`
- [x] drift against existing docs reported, not silently fixed
- [x] no secrets, connection strings or data rows in any artifact
- [ ] the confirmation round ran — not possible in a delegated run; every Inferred/Assumed item is listed above

## Handoff

Reconstructed: `specs/` (11 features + index), `docs/product/architecture.md`, `docs/product/domain-model.md`, this report and the SBOM. Riskiest finding: R1. Next: confirm the open items with the maintainer, fix R1 through the `repo` plugin's normal change flow, then `project:architecture` (review mode) for R2/R3, or `qa:strategy` for the untested pages and the LLM output.
