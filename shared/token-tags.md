# Token tags

Every card carries tags, assigned by the planner at planning time and written
on the card's route line in `cards.md` and on its first task line:
`· Tags: extend, backend, protocol, rust, tokio, known-pattern, integration, M`.

Exactly one value per facet, except `stack`, which takes any number (or none).
`lang` is required. Estimates match cards by shared tags
(`token-tracking.md`), so everyone uses this one vocabulary. It lives in
`shared/token-tags.json`; to extend it, add the value there and a row here
(`scripts/consistency.py` checks both, and that no value is in two facets).

## `kind`

What the work is.

| Value | Meaning |
|---|---|
| `new-module` | a new module, crate, package or screen written from scratch |
| `extend` | adds behavior to existing code |
| `fix` | repairs a defect or a review finding |
| `refactor` | restructures without changing behavior |
| `test-only` | adds or repairs tests and nothing else |
| `docs` | documentation or specs only |
| `ci` | pipeline, release or tooling configuration |

## `layer`

Which part of the product.

| Value | Meaning |
|---|---|
| `backend` | services, daemons, libraries, storage, protocols |
| `frontend` | web or desktop UI |
| `tui` | a terminal interface |
| `cli` | a command-line interface |
| `infra` | containers, environments, IaC, packaging |
| `qa` | test suites, journeys, load, charters |

## `surface`

What it touches.

| Value | Meaning |
|---|---|
| `protocol` | a wire format, IPC or API contract |
| `storage` | persistence, schema, migrations |
| `session-display` | what the user sees of a live session: rendering, terminals, windows |
| `ui-snapshots` | screens covered by snapshot or visual tests |
| `workflow` | pipelines, release flow, orchestration, automation |
| `other` | none of the above |

## `lang`

The main language written (required). Weighted with kind, surface and novelty in matching.

| Value | Meaning |
|---|---|
| `rust` | Rust |
| `typescript` | TypeScript |
| `javascript` | JavaScript |
| `python` | Python |
| `go` | Go |
| `java` | Java |
| `kotlin` | Kotlin |
| `csharp` | C# |
| `php` | PHP |
| `cpp` | C and C++ |
| `ruby` | Ruby |
| `sh` | shell scripts |
| `sql` | SQL |
| `markdown` | prose only (docs, specs) |

## `stack`

Frameworks and crates involved (optional, any number), normalized to the list below; aliases fold package names into one value (`react-dom` becomes `react`). Scored by overlap, never excluding a record.

| Value | Meaning |
|---|---|
| `tokio` | rust stack |
| `ratatui` | rust stack |
| `clap` | rust stack |
| `serde` | rust stack |
| `zbus` | rust stack |
| `x11rb` | rust stack |
| `wayland-client` | rust stack |
| `rusqlite` | rust stack |
| `axum` | rust stack |
| `react` | typescript stack |
| `vite` | typescript stack |
| `tailwind` | typescript stack |
| `vitest` | typescript stack |
| `playwright` | typescript stack |
| `express` | javascript stack |
| `gjs` | javascript stack |
| `fastapi` | python stack |
| `django` | python stack |
| `flask` | python stack |
| `pytest` | python stack |
| `spring` | java stack |
| `gin` | go stack |
| `cobra` | go stack |

Aliases: `react-dom` → `react`, `tailwindcss` → `tailwind`, `@playwright/test` → `playwright`, `playwright-core` → `playwright`, `wayland-client-rs` → `wayland-client`, `tokio-util` → `tokio`, `tokio-stream` → `tokio`, `serde_json` → `serde`, `serde_derive` → `serde`, `clap_derive` → `clap`, `@vitejs/plugin-react` → `vite`, `spring-boot` → `spring`, `spring-boot-starter-web` → `spring`, `github.com/gin-gonic/gin` → `gin`, `github.com/spf13/cobra` → `cobra`.

## `novelty`

How new the work is to the repo.

| Value | Meaning |
|---|---|
| `known-pattern` | the repo already does this the same way |
| `new-to-repo` | familiar technique, first use in this repo |
| `new-tech` | a library or technique new to the team |

## `verification`

What decides it is done.

| Value | Meaning |
|---|---|
| `unit` | unit tests decide done |
| `integration` | integration or contract tests decide done |
| `e2e` | end-to-end journeys decide done |
| `manual` | needs a person or a real device |

## `size`

Planned files or lines touched.

| Value | Meaning |
|---|---|
| `S` | about 1-3 files or under 150 lines touched |
| `M` | about 4-10 files or 150-600 lines |
| `L` | more than 10 files or 600 lines |
