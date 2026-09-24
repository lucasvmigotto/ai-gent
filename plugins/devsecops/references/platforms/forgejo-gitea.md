# Forgejo / Gitea Actions

GitHub Actions–compatible CI built into Forgejo and Gitea, executed by
`forgejo-runner` / Gitea `act_runner` (both derived from `nektos/act`).
Start from `github-actions.md` — syntax, attack surface and hardening
carry over — and apply the differences below. Compatibility varies by
version: check the instance's version and its documentation before using
any feature beyond the basics.

## Layout

- Workflows in `.forgejo/workflows/` (Forgejo) or `.gitea/workflows/`
  (Gitea; `.github/workflows/` is also read by Gitea).
- `runs-on:` matches **runner labels**, which map to container images
  configured on the runner (e.g. `docker://node:22-bookworm`), not to
  GitHub-hosted runner names.
- `uses: actions/checkout@<ref>` resolves against the instance's
  **default actions URL** (often `https://code.forgejo.org` or
  `https://github.com`). Use a full URL (`uses: https://code.forgejo.org/actions/checkout@<sha>`)
  to make the source explicit, and pin by SHA as on GitHub.

## Differences to plan around

- Feature parity with GitHub is partial: verify support for
  `concurrency:`, environments and approvals, reusable workflows, OIDC
  `id-token` issuance, artifacts v4 actions, caching and service
  containers on the target version. Where one is missing, say so in
  `docs/product/delivery.md` and use the workaround (e.g. a vault for
  credentials when OIDC isn't available, branch protection + a manual
  job for approvals).
- Secrets: repository/organization/user secrets; no environment-scoped
  secrets on older versions — separate deploy credentials by repository
  or by vault policy instead.
- Many GitHub Marketplace actions assume GitHub APIs (`GITHUB_TOKEN`
  scopes, GitHub-hosted tool caches); prefer plain `run:` steps or
  actions known to work on the instance.

## Attack surface (in addition to GitHub's list)

1. **Runners with the host Docker socket** mounted or in host mode —
   a job gets host root. Use rootless/Docker-in-Docker or per-job VMs for
   untrusted code.
2. **Runners registered instance-wide** serving repositories of
   different trust; register per organization/repository.
3. **Implicit action sources** — a short `uses:` resolving to an instance
   the attacker controls or to an unpinned mirror.
4. **Fork PRs** — confirm how the instance treats secrets and approval
   for first-time contributors.

## Lint and verify

- `actionlint` works for most syntax (ignore GitHub-only checks);
  `zizmor` catches the shared injection patterns. Exercise workflows on a
  branch (push only with the user's go-ahead).
