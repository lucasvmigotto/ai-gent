# README files

How the README files a stage writes or updates read — the project's
`README.md` and any folder README (`tests/load/README.md`,
`infra/README.md`). The README is the front door: what the project is,
how to run it, where the rest lives. Details go to `docs/`, the docs site
(`project:docs`) or the specs, and the README links to them.

## Sections, in this order

Only the ones that apply:

1. `# <Project name>` and one paragraph: what it is and who it's for.
2. **Quick start** — the shortest path from clone to running, as
   commands.
3. **Usage** — the main things people do with it.
4. **Development** — prerequisites, the container-first tasks
   (`devcontainer:setup`), how to run the tests, the branch and commit
   rules (`git:workflow`).
5. **Configuration** — environment variables and settings by **name**,
   never a value.
6. **Links** — docs site, specs, API contract, changelog.
7. **License.**

Flows and architecture fit a Mermaid diagram (GitHub renders it); a
table beats a paragraph for anything with columns.

## Callouts — GitHub alerts, by default

Anything a reader must not miss goes in a GitHub alert, never in bold
prose or an emoji:

| Alert | Use it for | Example |
|---|---|---|
| `> [!NOTE]` | context worth knowing even when skimming | "The API and the web client run as separate containers." |
| `> [!TIP]` | a better or faster way to do something | "`task test -- --watch` reruns only what changed." |
| `> [!IMPORTANT]` | what's required to succeed: a prerequisite, a key decision | "Node 22 or later; earlier versions fail the build." |
| `> [!WARNING]` | a risk that needs attention: a breaking change, a security concern | "2.0 renames every environment variable; update `.env` first." |
| `> [!CAUTION]` | what an action can destroy: irreversible or destructive | "`task reset-database` drops the local database and its data." |

```markdown
> [!WARNING]
> 2.0 renames every environment variable; update `.env` before upgrading.
```

- The marker sits alone on the blockquote's first line, the text on the
  next `>` lines.
- **Sparingly**: at most one per section, never two in a row, a sentence
  or two each. An alert that's everywhere stops being read.
- Pick by consequence, not by tone: a destructive command is `CAUTION`
  even when phrased politely; a nice-to-know is `NOTE`, not `IMPORTANT`.
- GitHub and GitLab render them as alerts; other platforms (Azure
  DevOps, Bitbucket) show a plain blockquote with the label, which still
  reads correctly.

## Keep it true

A README that contradicts the code is worse than none. When a stage
changes how the project runs, builds, tests or is configured, it updates
the README in the same change — and never claims a feature the specs
still mark Planned.
