---
name: proxy
description: Simulate a production path-based reverse proxy (nginx, Traefik, an ingress, an API gateway) in front of a project's local/devcontainer backend and frontend — using the project's real path prefixes, websocket upgrade headers so frontend HMR survives the hop, and cookie-path rewrites — shipped as an opt-in compose stack. Use when the user asks to "simulate the reverse proxy", "reproduce the prod path prefix locally", "test basename/CORS/cookie path issues", or when devcontainer:setup reaches its reverse-proxy phase. Skip when production has no path-based proxy.
---

# Simulating the production reverse proxy

**Only if production has one.** If production doesn't route through a
path-based reverse proxy, stop — adding a proxy layer that doesn't exist in
prod is a new thing to debug with no payoff.

Assumes the shared Docker network and module aliases (`api`, `web`) from
`devcontainer:setup`. If the project uses a dev IdP from
`devcontainer:infra`, the proxy often is also the fix for its issuer-URL
trap — see that skill's §4.4.

## Rules

- Read the **real** production path prefixes from the project's own
  config (its `.env.example`, existing proxy/ingress config, deployment
  manifests) — never invent a generic shortcut path. The entire value of
  this simulation is catching basename/CORS/cookie-path bugs that only
  appear when the path structure matches prod exactly; a simplified path
  defeats that purpose. A sibling project using a different, flatter
  convention doesn't mean this one should — match *this* project's own
  prod, not a sibling's.
- Use the **same proxy software as prod** when it's free (nginx, Traefik,
  Caddy, HAProxy — tier 1 in `devcontainer:infra` terms). For a managed
  gateway/ingress with no local image, use nginx and document which
  gateway features (auth offload, rate limits, header injection) are not
  simulated.
- One `location`-equivalent block per upstream: the API path(s) proxy to
  the backend module's network alias; everything else proxies to the
  frontend dev server, with the HTTP version bump and `Upgrade`/
  `Connection` headers HMR's websocket needs to survive the hop. Without
  them HMR silently degrades to full page reloads.
- Mirror prod's prefix-stripping behavior exactly — whether prod forwards
  `/app/api/users` as-is or strips it to `/users` decides the backend's
  context path; getting this wrong is exactly the bug class being tested.
- Forward the headers prod forwards (`X-Forwarded-Proto`/`-Host`/
  `-Prefix`), so frameworks generate correct absolute URLs and redirects.
- Rewrite cookie paths if the backend sets cookies scoped to a path that
  changes once fronted by the proxy.
- If a dev IdP is in play, register the proxied URL as a redirect URI.
- Ship this as its own compose file + config, run opt-in via a separate
  script — most edit-and-reload work doesn't need the proxy in the loop;
  only the moments actually checking cross-origin/path assumptions do.

## Minimal nginx shape

```nginx
# Prefixes copied from <source of truth, e.g. deploy/ingress.yaml> — keep in sync.
server {
  listen 80;

  location /<app-prefix>/api/ {
    proxy_pass http://api:8080/<backend-context-path>/;   # match prod's stripping
    proxy_set_header Host $host;
    proxy_set_header X-Forwarded-Proto $scheme;
    proxy_set_header X-Forwarded-Prefix /<app-prefix>/api;
    proxy_cookie_path /<backend-context-path>/ /<app-prefix>/api/;
  }

  location /<app-prefix>/ {
    proxy_pass http://web:5173;
    proxy_http_version 1.1;
    proxy_set_header Upgrade $http_upgrade;
    proxy_set_header Connection "upgrade";
    proxy_set_header Host $host;
  }
}
```

The frontend dev server must know its public base path (Vite `base`,
Angular `--base-href`/`--serve-path`, Next `basePath`) and may need its HMR
client pointed at the proxy's host/port — check the dev server's docs for
the setting name.

## Verification

- Repeat the app's smoke test through the proxy with the **real** backend
  behind it, not a stub — the proxy config is exactly the part a stub
  can't validate.
- Deep-link reload a client-side route under the prefix (catches missing
  history-API fallback / wrong basename).
- Log in, confirm the session cookie's `Path` matches the proxied prefix
  and survives a reload.
- Edit frontend source and confirm an HMR update through the proxy URL,
  not a full reload.
- Stop the proxy stack afterwards if it was only started to verify.
