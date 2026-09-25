# Retrofit tooling

Used by `project:retrofit` (`../SKILL.md`). Run everything inside the
project's container so the versions reported are the ones that ship.

## Scanning

| Purpose | Tools |
|---|---|
| Dependency CVEs, any ecosystem | `osv-scanner scan -r .`, `trivy fs .`, `grype dir:.` |
| Image CVEs | `trivy image <image>`, `grype <image>`, `docker scout cves` |
| From an SBOM | `grype sbom:docs/product/sbom.cdx.json`, `osv-scanner scan --sbom=…` |
| Exploitability | CISA KEV catalog, EPSS scores (FIRST API), GitHub advisories |
| End of life | `https://endoflife.date/api/<product>.json` (runtime, framework, database, OS) |

## Reachability — is the vulnerable code used?

| Ecosystem | Tool or method |
|---|---|
| Go | `govulncheck ./...` (call-graph based) |
| Rust | `cargo audit` + checking the affected function's call sites |
| JavaScript / TypeScript | `npm audit` / `pnpm audit` / `bun audit` for the list; trace imports of the vulnerable module and function (`grep`, `madge`); dev-only dependencies rarely reach production |
| Java / Kotlin | OWASP Dependency-Check or `trivy` for the list; `jdeps` and call-site search for the vulnerable class; check whether the vulnerable feature is enabled in config |
| Python | `pip-audit`; import and call-site search; optional extras that aren't installed are unreachable |
| .NET | `dotnet list package --vulnerable --include-transitive`; call-site search |
| PHP | `composer audit`; call-site search |

Record the evidence (`file:line` of the call, or its absence) next to each
verdict.

## Outdated and upgrade paths

| Ecosystem | See what's outdated | Upgrade helpers |
|---|---|---|
| npm / pnpm / Bun | `npm outdated`, `pnpm outdated`, `bun outdated`; `npx npm-check-updates` | framework codemods (`npx @next/codemod`, `ng update`, `npx @angular/cli update`), `npx @vue/cli upgrade` |
| Maven / Gradle | `mvn versions:display-dependency-updates`, `./gradlew dependencyUpdates` (ben-manes plugin) | **OpenRewrite** recipes (`UpgradeSpringBoot_3_x`, `UpgradeToJava21`, `JavaxMigrationToJakarta`) |
| Python | `pip list --outdated`, `uv pip list --outdated`, `poetry show --outdated` | `pyupgrade`, `django-upgrade`, `bump-pydantic`, `ruff --select UP` |
| .NET | `dotnet list package --outdated` | .NET Upgrade Assistant |
| PHP | `composer outdated` | **Rector** sets (PHP version, Symfony, Laravel via Shift) |
| Ruby | `bundle outdated` | `rails app:update`, `next_rails` dual boot |
| Go | `go list -u -m all` | `go get -u`, `go fix`, `gofmt -r` |
| Rust | `cargo outdated` | `cargo update`, `cargo fix --edition` |
| Containers | registry tags vs. the pinned digest | Renovate/Dependabot for digests; rebuild on the new base |

## Characterization tests

- **HTTP:** record responses from the running baseline (Playwright's
  `request` API, `pytest` with a recorded fixture, REST Assured) and
  assert on status, body shape and key values; mask volatile fields (ids,
  timestamps) instead of dropping the assertion.
- **Jobs and batch:** run with fixed inputs and a fixed clock; snapshot
  the rows written and files produced.
- **Mail and messaging:** assert on messages captured by Mailpit or read
  from the local broker.
- **Approval / golden-master:** ApprovalTests, `syrupy`, Jest snapshots,
  Verify (.NET) — reviewed once by a human, then frozen.
