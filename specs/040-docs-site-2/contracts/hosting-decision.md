# Hosting decision: Cloudflare R2 or GitHub Pages

The question: the Install page tells people to run a script from `github.com/lucasvmigotto/ai-gent/releases`. Whoever controls the page can change that command. Where the page lives is therefore a trust decision, not only a cost one.

Evidence labels: [OBSERVED] from this repository, [KNOWN] general knowledge, [ASSUMPTION] not verified here (no web research was done on either host's current limits; verify before relying).

## Option A: stay on Cloudflare R2 (today)

- [OBSERVED] `https://docs.lucasvmigotto.me/ai-gent/`; `.github/workflows/docs-ci.yml` syncs to a shared docs-hub bucket under the prefix `/<repo-name>/` using the S3 API (`aws s3 sync|cp`) with `CLOUDFLARE_R2_ACCOUNT_ID` (a repo variable used as the key id) and `CLOUDFLARE_R2_ACCOUNT_SECRET` (a repo secret). A guard step fails the build if any sync is not prefix-scoped.
- [OBSERVED] Sibling sites share the bucket; the workflow comments say never sync the bucket root.
- Pros: already working; a custom domain on the owner's own zone; full control of cache headers per object (the deploy sets immutable assets and no-cache entry files); can serve any file layout; one hub for several projects.
- Cons: a long-lived access key in repo secrets (a leaked or exfiltrated key can overwrite the site, including the install command, without touching GitHub); a second trust root (Cloudflare account) beyond GitHub; deep links need one object per route or an edge rule because there is no single-page-app fallback by itself [ASSUMPTION]; deploys are skipped silently when credentials are missing (`steps.creds.outputs.present`), so a fork builds but does not publish.

## Option B: GitHub Pages

- [KNOWN] Pages can be published from a workflow with the official Pages actions using the job's OIDC token and `pages: write`/`id-token: write` permissions, with no stored key; a custom domain with HTTPS is supported; project sites are served under `/<repo>/`, which matches the current base path `/ai-gent/`.
- Pros: the page that tells people what to run lives under the same account and permission model as the release assets and `install.sh` (one trust root); no long-lived credential; deployment history and environment protection rules in the repository; works for forks; no bucket shared with other projects.
- Cons: [ASSUMPTION] no control of response headers (no custom `Cache-Control`, CSP or HSTS settings beyond GitHub's defaults); [ASSUMPTION] soft bandwidth and size limits; a 404 page instead of a route fallback (a `404.html` copy of the app can serve deep links but with a 404 status); migrating loses the shared hub with sibling docs; the custom domain `docs.lucasvmigotto.me` has to be re-pointed per project or the hub restructured.

## Mitigations that apply to either

1. Publish the SHA-256 of `install.sh` in the GitHub Release notes and in the repository README, so a changed page can be cross-checked on GitHub.
2. Put `--verify-attestation` (`gh attestation verify`) on the Install page as the recommended path for people who have `gh`.
3. Pin the Install page's example to a version (`VERSION=<tag>`) next to the `latest` form.
4. Deploy the site only after the release is published (FR-018).

## Recommendation

Move to **GitHub Pages deployed by OIDC** if the docs-hub bucket is not needed for other projects, because it removes the only long-lived deploy credential and puts the install instructions under the same trust root as the artifacts they describe. If the hub matters to the owner, **stay on R2**, apply the four mitigations, and rotate the R2 key on a schedule. Either way the decision is the owner's; this spec makes no workflow change before the sign-off below.

## Sign-off

- Chosen option:
- Date:
- Owner:
