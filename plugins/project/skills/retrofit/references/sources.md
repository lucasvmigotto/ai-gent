# Where to look, per ecosystem

Read at Process steps 3 to 5 with `../../../references/research.md` (the
method: labels, safety, offline). Query package names and versions only,
never repo paths, internal names or code. Budget (soft): about 5 searches
and 5 fetches per step; extended search only for a major hop.

| Need | Sources |
|---|---|
| End-of-life dates | endoflife.date, then the vendor's support policy page |
| CVEs for a package and version | OSV (osv.dev), GitHub Advisory Database, NVD, the vendor's security advisories |
| Exploited in the wild | CISA KEV catalog; EPSS score (first.org) |
| Upgrade path per major | The framework's official migration guide and release notes for each hop (Spring Boot, Angular, Django, Rails, .NET, Laravel) |
| Abandoned library | Registry page (last release), repository activity, deprecation notice, the maintainer's recommended successor |

By ecosystem: npm (npm registry page, `npm audit` advisories), PyPI (PyPI
page, PyPA advisory DB), Maven (Maven Central, GHSA), NuGet (nuget.org
deprecation and vulnerability flags), Go (pkg.go.dev, vuln.go.dev), Cargo
(RustSec), Composer (Packagist, FriendsOfPHP), RubyGems (ruby-advisory-db),
container images (the image's tag page, the distro's security tracker).

## Recording

- `docs/product/retrofit.md` gets a **Sources** section: `S<n>`, claim,
  URL, retrieved date; the EOL table gets an *as of <date>* column.
- Re-research EOL dates and advisories older than about 30 days.
- A page or PoC that says "exploited" is cited, never executed.
- No web tools: label the rows `[ASSUMPTION]`, add "verify before relying".
