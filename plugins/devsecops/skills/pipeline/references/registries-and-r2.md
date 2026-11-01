# Registry logins and the Cloudflare R2 scheme

Part of `devsecops:pipeline`'s Render step; `containers.md` is `../../../references/containers.md`.

- **Registry logins** follow `containers.md` §3 and §8: `dhi.io` with the
  `DOCKER_HUB_PAT` secret; pushes to `ghcr.io` as the repository owner
  with the job's `GITHUB_TOKEN` (`packages: write` on that job only); to
  `docker.io` with `DOCKER_HUB_USERNAME` (default: the owner) and
  `DOCKER_HUB_PAT`. Image names lowercase; build with BuildKit (or
  buildah) cache mounts and a registry cache.
- **Cloudflare R2 configuration uses one shared scheme**, so a deploy reads
  the same way in every project:

  | Name | Kind | Holds |
  |---|---|---|
  | `CLOUDFLARE_R2_ACCOUNT_ID` | **variable** | the access-key **id** (fed to `AWS_ACCESS_KEY_ID`) |
  | `CLOUDFLARE_R2_ACCOUNT_SECRET` | **secret** | the secret half |
  | `CLOUDFLARE_R2_ENDPOINT_S3_CLIENT` | **variable** | the **S3-API** endpoint (`--endpoint-url`) |
  | `CLOUDFLARE_R2_BUCKET_ID` | **variable** | the bucket |

  Only the secret is a secret. The account id, endpoint and bucket are
  identifiers, not credentials: keeping them as variables makes them
  visible, reviewable and diffable, and keeps the secret store to the one
  value that needs it. A deploy that puts them in secrets anyway still
  works but hides configuration the next person has to debug blind.
  Some S3-compatible stores differ; keep the `CLOUDFLARE_R2_*` names when
  the provider is R2 and adapt the prefix for another, rather than
  inventing a per-project spelling.

  Pass the values through `env:` — never interpolate `${{ secrets.… }}`
  straight into a command path — and validate them in a step that fails
  loudly, naming the missing variable and where it is set. A deploy that
  silently syncs to the wrong bucket is worse than one that refuses to
  start.
