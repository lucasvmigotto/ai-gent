# Verification and pitfalls (§9 and the pitfalls table)

Part of `devcontainer:infra` (`../SKILL.md`): read it when verifying a stack, and when something behaves oddly.

## Verification — one real round trip per resource

A healthcheck only proves the process started, not that the app can use
it. From the app's own container, over the network alias:

- DB: migrations run; the app reads/writes one row.
- Cache: set + get a key.
- Queue/stream: publish + consume one message.
- Storage: write + read one object.
- SMTP: trigger one real app email (signup, password reset) and find it
  via Mailpit's API — confirm subject, recipient and rendered links.
- Mail API: one send reaches the sandbox/emulator/stub, and nothing
  reached a real inbox.
- Inbound mail: send to GreenMail, confirm the app ingests it.
- Identity: log in through the **browser** as each persona (§4.3 of `identity.md`), call a
  protected API endpoint with the resulting token and see it accepted;
  call it with the no-roles persona and see it **rejected**; call it with
  no token and see a 401. A dev IdP that makes everything pass is a
  bypass, not a simulation.
- Stubs: the app's real client hits the stub and parses the response.

Clean up any container/volume/image created only for the verification, per
`../../../references/containers.md` §9 — the stub images are as disposable as the
containers, and a stopped container still pins them.

## Pitfalls (observed firsthand — check these before assuming a fresh bug)

| Symptom | Cause | Fix |
|---|---|---|
| Compose resources named `<invoking-dir>_<resource>` instead of the intended project name | Compose prefixes resource names with the invoking directory when no explicit name is set | add a top-level `name: <project>` to the compose file; clean up anything already created under the stale prefix |
| A seed/init step intermittently fails right after `$CONTAINER_ENGINE compose up` | `depends_on` without a `condition` only waits for the target container to **start** | add a `healthcheck` to the target service and gate the seed step on `condition: service_healthy` |
| The app still reaches the real cloud endpoint even with the emulator container running | Cloud SDKs default to the real service; the emulator needs an explicit endpoint override (`AWS_ENDPOINT_URL`/path-style flag, an Azure emulator connection string) | set the override explicitly in the dev `.env`/`.env.example`, and confirm it with §9's round trip |
| Login works in the browser, but the API rejects the token with 401 "invalid issuer" | `iss` holds the browser-facing URL (`localhost`), the API is configured with the internal alias URL (or vice versa) | §4.4 of `identity.md` — split issuer and JWKS URLs, or pin the IdP's public hostname |
| API fails to start: can't fetch OIDC discovery | `issuer-uri`-style config fetches discovery at boot from a URL only the browser can reach, or before the IdP is healthy | use the alias for JWKS/discovery from the container; gate the API on the IdP's healthcheck |
| Authenticated, but every endpoint is 403 | Dev tokens carry roles under a different claim than prod (`realm_access.roles` vs. `roles`) | emit prod's exact claim shape (§4.3 of `identity.md`) instead of changing the app's mapping |
| IdP login page says "invalid redirect_uri" | dev redirect URI not registered exactly (port, path, trailing slash, proxy prefix) | add the exact URI to the realm export / mock config |
| App errors on SMTP connect: "AUTH not supported" or TLS handshake failure | Mailpit's defaults (no auth, no TLS) don't match what the client insists on | enable accept-any auth / give Mailpit a cert (§5.2 of `mail.md`) rather than editing the app's mail config |
| Emails "sent" but nothing in Mailpit | app points at `localhost:1025` from inside its container (that's its own loopback) | use the alias `mailpit:1025` |
