# Phase 7 — Ship in detail

- **Paths:** every CI step, cache key and the Docker build context use
  the site directory — `docs/site/` normally, `site/` for a hub-type repo
  (`working-directory: <site>`, `paths: [<site>/**]` triggers). Content
  read from `docs/product/` or `specs/` must also trigger a rebuild.
- **GitHub Actions, written here.** Unless the project already uses
  another platform, the skill writes the site's workflow itself —
  `.github/workflows/<site>.yml` with a `verify` job (checkout → setup Bun
  pinned → install → lint → typecheck → test → build → upload `dist/`) and
  a `deploy` job (`needs: verify`) that publishes on the default branch
  only. When a release workflow tags the default branch after the push,
  deploy on its completion instead (`workflow_run`), or the footer
  shows the previous version (`version-footer.md`).
  `devsecops:pipeline` still owns the supply-chain baseline (action
  pinning, SBOM, signing) and reviews/hardens this file; it does not invent
  its shape.
- **Register the deployment.** The publishing job must declare a GitHub
  Actions **environment** — a run that only syncs files is invisible in the
  repository's Deployments and has nowhere to attach protection rules:

  ```yaml
  deploy:
    needs: verify
    if: github.ref == 'refs/heads/main'
    environment:
      name: <repo>-docs        # e.g. rusteams-docs
      url: <public site URL>   # e.g. https://docs.lucasvmigotto.me/rusteams/
  ```

  `environment:` is what makes GitHub treat the run as a real Deployment:
  it lists under Deployments, keeps its own history, and becomes the home
  for environment-scoped variables/secrets and, later, required reviewers.
  A job merely *named* `deploy` without it is still just steps. One
  environment per site (`<repo>-docs`) so sibling repos don't share
  protection rules by accident; the job's display name stays descriptive
  (`Sync static site to …`).
- **R2 deploy steps:** the `deploy` job runs checkout → setup Bun (pinned) →
  install → lint → typecheck → test → build → artifact → S3 sync to
  Cloudflare R2.
  Immutable long-lived caching for hashed assets, `no-cache` for entry
  HTML, `llms.txt`, `llms-full.txt` and the `.md` pages, which are served
  as `text/markdown; charset=utf-8` (`.txt` as `text/plain;
  charset=utf-8`). Concurrency cancel-in-progress, least-privilege permissions,
  endpoint validation. Credentials via Variables/Secrets placeholders —
  **the names are `devsecops:pipeline`'s scheme, not a per-project
  spelling**: `CLOUDFLARE_R2_ACCOUNT_ID`, `CLOUDFLARE_R2_ACCOUNT_SECRET`,
  `CLOUDFLARE_R2_ENDPOINT_S3_CLIENT`, `CLOUDFLARE_R2_BUCKET_ID`.
- **Docker (optional):** per `../../../references/containers.md` — a Bun
  build stage with the sources bind-mounted and Bun's cache mounted, a
  hardened (or unprivileged) nginx runtime pinned by digest that copies
  only `dist/`, a deny-by-default `.dockerignore`, non-root user,
  security headers, gzip, and `types` for `.md` as `text/markdown`. Repeat security headers inside every
  `location` block — location-level `add_header` disables server-level
  inheritance. Hash routing needs no SPA fallback (`try_files
  $uri $uri/ =404`).
- **Final report:** architecture, reference deltas (reused / adapted /
  intentionally changed + why), accessibility mechanisms, i18n design,
  deployment, validation results, remaining work, future work.
